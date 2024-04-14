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

use dbus::nonblock::{LocalConnection, Proxy};
use dbus_tokio::connection;
use std::error::Error;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod data;
mod time;

use crate::data::StatusbarData;

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

    // TODO: add match for TZ change
    // TODO: update data
    // TODO: schedule refresh

    Ok(())
}

// TODO: async for listening for time change
// This involves creating a timerfd with TFD_TIMER_CANCEL_ON_SET set,
// then waiting on it. This will involve tokio::io::AsyncRead, or
// something like that.

async fn update_statusbar(data: Arc<Mutex<StatusbarData>>) -> Result<(), Box<dyn Error>> {
    // TODO: get time
    // TODO: calculate top of next minute
    // TODO: sleep until next minute

    // TODO: grab lock on StatusbarData
    let dat = data.lock().unwrap();

    let stdout = io::stdout();
    let mut locked = stdout.lock();

    // TODO: print out status

    Ok(())
}

async fn setup_system_connection(
    sys_conn: Arc<LocalConnection>,
    data: Arc<Mutex<StatusbarData>>,
) -> Result<(), Box<dyn Error>> {
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

    // Print all the names.
    for name in sys_act_names {
        println!("{}", name);
    }

    // TODO: make connections to UPower & al. and add matches to subscribe to signals.

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn main() -> Result<(), Box<dyn Error>> {
    let local_tasks = tokio::task::LocalSet::new();

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
    let _sys_connect = local_tasks.spawn_local(setup_system_connection(sys_conn, sb_dat));

    // TODO: set up the statusbar printer?

    // Wait for our tasks to finish.
    local_tasks.await;

    Ok(())
}
