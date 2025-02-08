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
use dbus::nonblock::stdintf::org_freedesktop_dbus::Properties;
use dbus::nonblock::{LocalConnection, Proxy};
use dbus_tokio::connection;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{self as tokio_io, AsyncWrite, AsyncWriteExt};
use tokio::runtime::Builder;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::Mutex as tokio_Mutex;

mod data;
mod io;
mod time;

use crate::data::battery::BatteryStatus;
use crate::data::{StatusbarChangeCause, StatusbarData};
use crate::io::StatusbarIOContext;
use crate::time::{wait_till_next_minute, wait_till_time_change};

// TODO: add a place to put realtime clock change detection.

async fn listen_to_upower(
    sys_conn: Arc<LocalConnection>,
    data: Arc<Mutex<StatusbarData>>,
) -> Result<(), Box<dyn Error>> {
    let upower_proxy = Proxy::new(
        "org.freedesktop.UPower",
        "/org/freedesktop/UPower/devices/DisplayDevice",
        Duration::from_secs(5),
        sys_conn,
    );

    // Get the starting percentage.
    let start_pct = upower_proxy
        .get::<f64>("org.freedesktop.UPower.Device", "Percentage")
        .await?;

    // Set the start percentage.
    {
        let mut dat = data.lock().unwrap();
        dat.update_battery(BatteryStatus {
            percentage: start_pct,
        });
    }

    // TODO: remove
    println!("starting battery: {}%", start_pct);

    // TODO: add match
    // TODO: update data
    // TODO: schedule refresh

    Ok(())
}

async fn listen_for_tzchange(
    sys_conn: Arc<LocalConnection>,
    data: Arc<Mutex<StatusbarData>>,
) -> Result<(), Box<dyn Error>> {
    let timedate_proxy = Proxy::new(
        "org.freedesktop.timedate1",
        "/org/freedesktop/timedate1",
        Duration::from_secs(5),
        sys_conn,
    );

    // Get the starting TZ.
    let start_tz = timedate_proxy
        .get::<String>("org.freedesktop.timedate1", "Timezone")
        .await?
        .parse::<Tz>()?;

    // Set the starting TZ.
    {
        let mut dat = data.lock().unwrap();
        dat.update_timezone(start_tz);
    }

    // TODO: remove
    println!("starting TZ: {}", start_tz);

    // TODO: add match for TZ change
    // TODO: update data
    // TODO: schedule refresh

    Ok(())
}

async fn fire_on_next_minute<SBO, DO>(
    changeQ: Sender<StatusbarChangeCause>,
    ioCtx: Arc<tokio_Mutex<StatusbarIOContext<SBO, DO>>>,
) -> Result<(), Box<dyn Error>>
where
    SBO: AsyncWrite + Unpin,
    DO: AsyncWrite + Unpin,
{
    // TODO this should return Result<!, ...>

    loop {
        wait_till_next_minute(ioCtx.clone()).await?;
        changeQ.send(StatusbarChangeCause::NextMinute).await?;
    }
}

async fn fire_on_clock_change(changeQ: Sender<StatusbarChangeCause>) -> Result<(), Box<dyn Error>> {
    // TODO this should return Result<!, ...>

    loop {
        wait_till_time_change().await?;
        changeQ.send(StatusbarChangeCause::ClockAdjust).await?;
    }
}

// TODO: async for listening for time change
// This involves creating a timerfd with TFD_TIMER_CANCEL_ON_SET set,
// then waiting on it. This will involve tokio::io::AsyncRead, or
// something like that.

async fn update_statusbar<SBO, DO>(
    data: Arc<Mutex<StatusbarData>>,
    mut changeQ: Receiver<StatusbarChangeCause>,
    ioCtx: Arc<tokio_Mutex<StatusbarIOContext<SBO, DO>>>,
) -> Result<(), Box<dyn Error>>
where
    SBO: AsyncWrite + Unpin,
    DO: AsyncWrite + Unpin,
{
    // TODO this should return Result<!, ...>
    // TODO: get time
    // TODO: calculate top of next minute
    // TODO: sleep until next minute

    loop {
        // TODO: grab lock on StatusbarData
        let dat = data.lock().unwrap();

        let newStat = format!("{}\n", dat);

        {
            let output = &mut ioCtx.lock().await.statusbarOutput;

            output.write_all(newStat.as_bytes()).await?;
            output.flush().await?;
        }

        match changeQ.recv().await {
            None => return Ok(()),
            Some(StatusbarChangeCause::NextMinute) => {},
            Some(StatusbarChangeCause::ClockAdjust) => {},
            Some(StatusbarChangeCause::TzChange(mbd)) => {},
            Some(StatusbarChangeCause::BatteryChange(mbd)) => {},
        }
    }
    // TODO: print out status
}

async fn setup_system_connection<SBO, DO>(
    sys_conn: Arc<LocalConnection>,
    data: Arc<Mutex<StatusbarData>>,
    ioCtx: Arc<tokio_Mutex<StatusbarIOContext<SBO, DO>>>,
) -> Result<(), Box<dyn Error>>
where
    SBO: AsyncWrite + Unpin,
    DO: AsyncWrite + Unpin,
{
    // Get a proxy to the bus.
    let bus_proxy = Proxy::new(
        "org.freedesktop.DBus",
        "/",
        Duration::from_secs(5),
        sys_conn,
    );

    // Get what activatable names there are.
    let (sys_act_names,): (Vec<String>,) = bus_proxy
        .method_call("org.freedesktop.DBus", "ListActivatableNames", ())
        .await?;

    {
        let output = &mut ioCtx.lock().await.debugOutput;

        // Print all the names.
        for name in sys_act_names {
            output.write_all(format!("{}\n", name).as_bytes()).await?;
        }

        output.flush().await?;
    }

    // TODO: make connections to UPower & al. and add matches to subscribe to signals.

    Ok(())
}

async fn task_setup() -> Result<(), Box<dyn Error>> {
    let local_tasks = tokio::task::LocalSet::new();

    let ioCtx = Arc::new(tokio_Mutex::new(StatusbarIOContext::from((
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

    // TODO: proper lifetime management
    let sb_dat = Arc::new(Mutex::new(StatusbarData::new()));

    // Set up all the listening stuff for the system connection.
    let _sys_connect = local_tasks.spawn_local(setup_system_connection(
        sys_conn.clone(),
        sb_dat.clone(),
        ioCtx.clone(),
    ));

    let (tx, mut rx) = channel(32);

    let _upow_connect = local_tasks.spawn_local(listen_to_upower(sys_conn.clone(), sb_dat.clone()));
    let _tz_connect = local_tasks.spawn_local(listen_for_tzchange(sys_conn, sb_dat.clone()));

    let _tick_minute = local_tasks.spawn_local(fire_on_next_minute(tx.clone(), ioCtx.clone()));
    let _listen_adj = local_tasks.spawn_local(fire_on_clock_change(tx));

    let _update_stat = local_tasks.spawn_local(update_statusbar(sb_dat, rx, ioCtx));

    // TODO: set up the statusbar printer?

    // Wait for our tasks to finish.
    local_tasks.await;

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
