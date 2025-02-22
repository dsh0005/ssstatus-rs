// SPDX-License-Identifier: AGPL-3.0-only

/* Silly Simple Status(bar) widget
 * Copyright (C) 2025 Douglas Storm Hill
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

use std::error::Error;
use std::sync::Arc;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::sync::mpsc::Receiver;

use crate::data::StatusbarChangeCause::{self, BatteryChange, TzChange};
use crate::data::StatusbarData;
use crate::io::StatusbarIOContext;

pub async fn run_statusbar_updater<SBO, DO>(
    mut change_q: Receiver<StatusbarChangeCause>,
    io_ctx: Arc<Mutex<StatusbarIOContext<SBO, DO>>>,
) -> Result<(), Box<dyn Error>>
where
    SBO: AsyncWrite + Unpin,
    DO: AsyncWrite + Unpin,
{
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
