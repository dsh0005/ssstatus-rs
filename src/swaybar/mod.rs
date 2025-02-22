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

async fn print_header<SBO, DO>(
    io_ctx: &Arc<Mutex<StatusbarIOContext<SBO, DO>>>,
) -> Result<(), Box<dyn Error>>
where
    SBO: AsyncWrite + Unpin,
    DO: AsyncWrite + Unpin,
{
    let header = String::from("{ \"version\": 1 }\n");

    let output = &mut io_ctx.lock().await.statusbar_output;

    output.write_all(header.as_bytes()).await?;

    Ok(())
}

async fn print_body_begin<SBO, DO>(
    io_ctx: &Arc<Mutex<StatusbarIOContext<SBO, DO>>>,
) -> Result<(), Box<dyn Error>>
where
    SBO: AsyncWrite + Unpin,
    DO: AsyncWrite + Unpin,
{
    let body_begin = String::from("[\n");

    let output = &mut io_ctx.lock().await.statusbar_output;

    output.write_all(body_begin.as_bytes()).await?;

    Ok(())
}

async fn print_status_line<SBO, DO>(
    data: &StatusbarData,
    io_ctx: &Arc<Mutex<StatusbarIOContext<SBO, DO>>>,
) -> Result<(), Box<dyn Error>>
where
    SBO: AsyncWrite + Unpin,
    DO: AsyncWrite + Unpin,
{
    let line = format!(
        "\t[\n\t\t{{\n\t\t\t\"full_text\": \"{}\"\n\t\t}}\n\t],\n",
        data
    );

    let output = &mut io_ctx.lock().await.statusbar_output;

    output.write_all(line.as_bytes()).await?;
    output.flush().await?;

    Ok(())
}

async fn print_infinite_body<SBO, DO>(
    mut change_q: Receiver<StatusbarChangeCause>,
    io_ctx: Arc<Mutex<StatusbarIOContext<SBO, DO>>>,
) -> Result<(), Box<dyn Error>>
where
    SBO: AsyncWrite + Unpin,
    DO: AsyncWrite + Unpin,
{
    print_body_begin(&io_ctx).await?;

    let mut data = StatusbarData::new();

    loop {
        print_status_line(&data, &io_ctx).await?;

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

pub async fn run_statusbar_updater<SBO, DO>(
    change_q: Receiver<StatusbarChangeCause>,
    io_ctx: Arc<Mutex<StatusbarIOContext<SBO, DO>>>,
) -> Result<(), Box<dyn Error>>
where
    SBO: AsyncWrite + Unpin,
    DO: AsyncWrite + Unpin,
{
    print_header(&io_ctx).await?;

    print_infinite_body(change_q, io_ctx).await?;

    Ok(())
}
