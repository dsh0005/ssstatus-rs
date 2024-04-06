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

use dbus_tokio::connection;
use dbus::nonblock::{LocalConnection, Proxy};
use std::sync::Arc;
use std::error::Error;
use std::time::Duration;

async fn setup_system_connection(sys_conn: Arc<LocalConnection>) -> Result<(), Box<dyn Error>> {
    // Get a proxy to the bus.
    let bus_proxy = Proxy::new("org.freedesktop.DBus", "/", Duration::from_secs(5), sys_conn);

    // Get what activatable names there are.
    let (sys_act_names,): (Vec<String>,) = bus_proxy.method_call("org.freedesktop.DBus", "ListActivatableNames", ()).await?;

    // Print all the names.
    for name in sys_act_names { println!("{}", name); }

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

    // Set up all the listening stuff for the system connection.
    let _sys_connect = local_tasks.spawn_local(setup_system_connection(sys_conn));

    // Wait for our tasks to finish.
    local_tasks.await;

    Ok(())
}
