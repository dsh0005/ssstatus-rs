// SPDX-License-Identifier: AGPL-3.0-only

/* Silly Simple Status(bar) widget
 * Copyright (C) 2024, 2025 Douglas Storm Hill
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, version 3 of the License.
 *
 * This program is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public
 * License along with this program.
 * If not, see <https://www.gnu.org/licenses/>.
 */

use chrono_tz::Tz;
use dbus::arg::RefArg;
use dbus::message::{Message, SignalArgs};
use dbus::nonblock::stdintf::org_freedesktop_dbus::{
    Properties, PropertiesPropertiesChanged as PropChange,
};
use dbus::nonblock::{LocalConnection, MsgMatch, Proxy};
use dbus::strings::{Interface, Member};
use dbus_tokio::connection;
use std::convert::Infallible;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{self as tokio_io, AsyncWrite};
use tokio::runtime::Builder;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{Sender, channel};
use tokio::task::spawn_local;

mod data;
mod io;
mod swaybar;
mod time;

use crate::data::battery::BatteryStatus;
use crate::data::{MaybeData, StatusbarChangeCause};
use crate::io::StatusbarIOContext;
use crate::swaybar::run_statusbar_updater;
use crate::time::{ClockTickCallbacks, tick_every_minute};

async fn wrangle_lifetimes_update(
    change_q: Sender<StatusbarChangeCause>,
    data: StatusbarChangeCause,
) -> Result<(), Box<dyn Error>> {
    Ok(change_q.send(data).await?)
}

async fn listen_to_upower(
    sys_conn: Arc<LocalConnection>,
    change_q: Sender<StatusbarChangeCause>,
) -> Result<MsgMatch, Box<dyn Error>> {
    let rule = PropChange::match_rule(
        None,
        Some(&"/org/freedesktop/UPower/devices/DisplayDevice".into()),
    )
    .static_clone();

    let upower_proxy = Proxy::new(
        "org.freedesktop.UPower",
        rule.path.clone().unwrap(),
        Duration::from_secs(5),
        sys_conn.clone(),
    );

    let iface = match Interface::new("org.freedesktop.UPower.Device") {
        Ok(interface) => interface,
        Err(_description) => {
            unreachable!("This hardcoded name is the correct one so it must be okay.")
        }
    };

    let percent_member = match Member::new("Percentage") {
        Ok(member) => member,
        Err(_description) => {
            unreachable!("This hardcoded name is the correct one so it must be okay.")
        }
    };

    // TODO: go introspect and make sure that Percentage is marked emits-change.

    let cloned_change_q = change_q.clone();

    let mtch = sys_conn.add_match(rule).await?.cb(move |_mesg: Message, change: PropChange| {
        if change.interface_name == "org.freedesktop.UPower.Device" {
            let maybe_new_pct = {
                if let Some(new_value) = change.changed_properties.get("Percentage") {
                    Some(new_value.as_f64().expect("Percentage is documented as \"double\""))
                } else if change.invalidated_properties.contains(&String::from("Percentage")) {
                    unimplemented!("Firing off a message here would reenter dbus and I haven't thought that out yet.");
                } else {
                    None
                }
            };

            if let Some(new_pct) = maybe_new_pct {
                let got_bat_when = Instant::now();
                spawn_local(wrangle_lifetimes_update(cloned_change_q.clone(), StatusbarChangeCause::BatteryChange(MaybeData(Ok(Some((got_bat_when, BatteryStatus::from(new_pct))))))));
            }
        }

        true
    });

    // Get the starting percentage.
    let start_pct = upower_proxy.get::<f64>(&iface, &percent_member).await?;
    let got_bat_when = Instant::now();

    change_q
        .send(StatusbarChangeCause::BatteryChange(MaybeData(Ok(Some((
            got_bat_when,
            BatteryStatus::from(start_pct),
        ))))))
        .await?;

    Ok(mtch)
}

async fn listen_for_tzchange(
    sys_conn: Arc<LocalConnection>,
    change_q: Sender<StatusbarChangeCause>,
) -> Result<MsgMatch, Box<dyn Error>> {
    let rule =
        PropChange::match_rule(None, Some(&"/org/freedesktop/timedate1".into())).static_clone();

    let timedate_proxy = Proxy::new(
        "org.freedesktop.timedate1",
        rule.path.clone().unwrap(),
        Duration::from_secs(5),
        sys_conn.clone(),
    );

    let iface = match Interface::new("org.freedesktop.timedate1") {
        Ok(interface) => interface,
        Err(_description) => {
            unreachable!("This hardcoded name is the correct one so it must be okay.")
        }
    };

    let tz_name_member = match Member::new("Timezone") {
        Ok(member) => member,
        Err(_description) => {
            unreachable!("This hardcoded name is the correct one so it must be okay.")
        }
    };

    // TODO: go introspect and make sure that Timezone is marked emits-change.

    let cloned_change_q = change_q.clone();

    let mtch = sys_conn.add_match(rule).await?.cb(move |_mesg: Message, change: PropChange| {
        if change.interface_name == "org.freedesktop.timedate1" {
            let maybe_new_tz = {
                if let Some(new_tz_str) = change.changed_properties.get("Timezone") {
                    Some(new_tz_str.as_str().expect("Timezone is documented as a string").parse::<Tz>().expect("expected to recognize timezone name"))
                } else if change.invalidated_properties.contains(&String::from("Timezone")) {
                    unimplemented!("Firing off a message here would reenter dbus and I haven't thought that out yet.");
                } else {
                    None
                }
            };

            if let Some(new_tz) = maybe_new_tz {
                let got_tz_when = Instant::now();
                spawn_local(wrangle_lifetimes_update(cloned_change_q.clone(), StatusbarChangeCause::TzChange(MaybeData(Ok(Some((got_tz_when, new_tz)))))));
            }
        }

        true
    });

    // Get the starting TZ.
    let start_tz_str = timedate_proxy
        .get::<String>(&iface, &tz_name_member)
        .await?;
    let got_tz_when = Instant::now();
    let start_tz = start_tz_str.parse::<Tz>()?;

    change_q
        .send(StatusbarChangeCause::TzChange(MaybeData(Ok(Some((
            got_tz_when,
            start_tz,
        ))))))
        .await?;

    Ok(mtch)
}

struct TreatPossibleChangesConservatively<'a> {
    change_q: &'a Sender<StatusbarChangeCause>,
}

impl ClockTickCallbacks for TreatPossibleChangesConservatively<'_> {
    async fn changed_minute(&self) -> Result<(), Box<dyn Error>> {
        self.change_q.send(StatusbarChangeCause::NextMinute).await?;
        Ok(())
    }
    async fn minute_maybe_lost(&self) -> Result<(), Box<dyn Error>> {
        self.change_q.send(StatusbarChangeCause::NextMinute).await?;
        Ok(())
    }
    async fn adjustment_happened(&self) -> Result<(), Box<dyn Error>> {
        self.change_q
            .send(StatusbarChangeCause::ClockAdjust)
            .await?;
        Ok(())
    }
}

async fn fire_on_next_minute<SBO, DO>(
    change_q: Sender<StatusbarChangeCause>,
    io_ctx: Arc<Mutex<StatusbarIOContext<SBO, DO>>>,
) -> Result<Infallible, Box<dyn Error>>
where
    SBO: AsyncWrite + Unpin,
    DO: AsyncWrite + Unpin,
{
    // TODO this should return Result<!, ...>

    let cb = TreatPossibleChangesConservatively {
        change_q: &change_q,
    };

    tick_every_minute(io_ctx, &cb).await
}

async fn task_setup() -> Result<(), Box<dyn Error>> {
    let local_tasks = tokio::task::LocalSet::new();

    let io_ctx = Arc::new(Mutex::new(StatusbarIOContext::from((
        tokio_io::stdout(),
        tokio_io::stderr(),
    ))));

    // Connect to the system bus, since we want time, battery, &c. info.
    let (sys_resource, sys_conn) = connection::new_system_local()?;

    // TODO: Do we want to connect to the session bus?

    // Start our resource tracker task, to see if we lose connection.
    let _system_handle = local_tasks.spawn_local(async {
        let err = sys_resource.await;
        panic!("Lost connection to system D-Bus: {}", err);
    });

    // Make the channel, with a totally arbitrary depth.
    let (tx, rx) = channel(32);

    let upow_connect = local_tasks.spawn_local(listen_to_upower(sys_conn.clone(), tx.clone()));
    let tz_connect = local_tasks.spawn_local(listen_for_tzchange(sys_conn.clone(), tx.clone()));

    let _tick_minute = local_tasks.spawn_local(fire_on_next_minute(tx.clone(), io_ctx.clone()));

    let _update_stat = local_tasks.spawn_local(run_statusbar_updater(rx, io_ctx));

    let upow_unlisten_match = local_tasks.run_until(upow_connect).await??;
    let tz_unlisten_match = local_tasks.run_until(tz_connect).await??;

    // Wait for our tasks to finish.
    local_tasks.await;

    sys_conn.remove_match(upow_unlisten_match.token()).await?;
    sys_conn.remove_match(tz_unlisten_match.token()).await?;

    Ok(())
}

use nix::sys::prctl::set_timerslack;

pub fn main() -> Result<(), Box<dyn Error>> {
    // Set a vague guess at a decent slack.
    set_timerslack(7_500_000u64)?;

    Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .thread_keep_alive(Duration::from_secs(70))
        .build()
        .unwrap()
        .block_on(task_setup())
}
