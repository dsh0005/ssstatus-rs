// SPDX-License-Identifier: AGPL-3.0-only

/* Silly Simple Status(bar) widget
 * Copyright (C) 2024 Douglas Storm Hill
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
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{self as tokio_io, AsyncWrite, AsyncWriteExt};
use tokio::runtime::Builder;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::Mutex;
use tokio::task::spawn_local;

mod data;
mod io;
mod time;

use crate::data::battery::BatteryStatus;
use crate::data::StatusbarChangeCause::{self, BatteryChange, TzChange};
use crate::data::{MaybeData, StatusbarData};
use crate::io::StatusbarIOContext;
use crate::time::{wait_till_next_minute, wait_till_time_change, ClockChangedCallback};

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
            unreachable!()
        }
    };

    let percent_member = match Member::new("Percentage") {
        Ok(member) => member,
        Err(_description) => {
            unreachable!()
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
                    panic!("help I can't reasonably do async in a sync callback, it'd reenter dbus!");
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
) -> Result<(), Box<dyn Error>> {
    let timedate_proxy = Proxy::new(
        "org.freedesktop.timedate1",
        "/org/freedesktop/timedate1",
        Duration::from_secs(5),
        sys_conn,
    );

    // Get the starting TZ.
    let start_tz_str = timedate_proxy
        .get::<String>("org.freedesktop.timedate1", "Timezone")
        .await?;
    let got_tz_when = Instant::now();
    let start_tz = start_tz_str.parse::<Tz>()?;

    change_q
        .send(StatusbarChangeCause::TzChange(MaybeData(Ok(Some((
            got_tz_when,
            start_tz,
        ))))))
        .await?;

    // TODO: add match for TZ change
    // TODO: update data
    // TODO: schedule refresh

    Ok(())
}

async fn fire_on_next_minute<SBO, DO>(
    change_q: Sender<StatusbarChangeCause>,
    io_ctx: Arc<Mutex<StatusbarIOContext<SBO, DO>>>,
) -> Result<(), Box<dyn Error>>
where
    SBO: AsyncWrite + Unpin,
    DO: AsyncWrite + Unpin,
{
    // TODO this should return Result<!, ...>

    loop {
        wait_till_next_minute(io_ctx.clone()).await?;
        change_q.send(StatusbarChangeCause::NextMinute).await?;
    }
}

struct TreatPossibleChangesConservatively<'a> {
    change_q: &'a Sender<StatusbarChangeCause>,
}

impl ClockChangedCallback for TreatPossibleChangesConservatively<'_> {
    async fn clock_change_maybe_lost(&self) -> Result<(), Box<dyn Error>> {
        self.change_q
            .send(StatusbarChangeCause::ClockAdjust)
            .await?;
        Ok(())
    }
}

async fn fire_on_clock_change(
    change_q: Sender<StatusbarChangeCause>,
) -> Result<(), Box<dyn Error>> {
    // TODO this should return Result<!, ...>

    let cb = TreatPossibleChangesConservatively {
        change_q: &change_q,
    };

    loop {
        wait_till_time_change(&cb).await?;
        change_q.send(StatusbarChangeCause::ClockAdjust).await?;
    }
}

async fn update_statusbar<SBO, DO>(
    mut change_q: Receiver<StatusbarChangeCause>,
    io_ctx: Arc<Mutex<StatusbarIOContext<SBO, DO>>>,
) -> Result<(), Box<dyn Error>>
where
    SBO: AsyncWrite + Unpin,
    DO: AsyncWrite + Unpin,
{
    // TODO this should return Result<!, ...>

    let mut data = StatusbarData::new();

    loop {
        let new_stat = format!("{}\n", data);

        {
            let output = &mut io_ctx.lock().await.statusbar_output;

            output.write_all(new_stat.as_bytes()).await?;
            output.flush().await?;
        }

        match change_q.recv().await {
            Some(TzChange(tz_change)) => {
                data.update_timezone_maybedata(tz_change);
            }
            Some(BatteryChange(bat_change)) => {
                data.update_battery_maybedata(bat_change);
            }
            Some(_) => {}
            None => {
                return Ok(());
            }
        }
    }
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
    let _tz_connect = local_tasks.spawn_local(listen_for_tzchange(sys_conn.clone(), tx.clone()));

    let _tick_minute = local_tasks.spawn_local(fire_on_next_minute(tx.clone(), io_ctx.clone()));
    let _listen_adj = local_tasks.spawn_local(fire_on_clock_change(tx));

    let _update_stat = local_tasks.spawn_local(update_statusbar(rx, io_ctx));

    let upow_unlisten_match = local_tasks.run_until(upow_connect).await??;

    // Wait for our tasks to finish.
    local_tasks.await;

    sys_conn.remove_match(upow_unlisten_match.token()).await?;

    Ok(())
}

pub fn main() -> Result<(), Box<dyn Error>> {
    Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .unwrap()
        .block_on(task_setup())
}
