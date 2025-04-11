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
use std::rc::Rc;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio::sync::mpsc::Receiver;

use crate::data::StatusbarChangeCause::{self, BatteryChange, TzChange};
use crate::data::StatusbarData;
use crate::io::StatusbarIOContext;
use crate::time::ShortenedDTD;

mod json;

use json::{EscapeJSONString, EscapePolicy::MinimalEscaping};

async fn print_header(io_ctx: &Rc<Mutex<StatusbarIOContext<'_>>>) -> Result<(), Box<dyn Error>> {
    let header = String::from("{ \"version\": 1 }\n");

    let output = &mut io_ctx.lock().await.statusbar_output;

    output.write_all(header.as_bytes()).await?;

    Ok(())
}

async fn print_body_begin(
    io_ctx: &Rc<Mutex<StatusbarIOContext<'_>>>,
) -> Result<(), Box<dyn Error>> {
    let body_begin = String::from("[\n");

    let output = &mut io_ctx.lock().await.statusbar_output;

    output.write_all(body_begin.as_bytes()).await?;

    Ok(())
}

async fn print_status_line(
    data: &StatusbarData,
    io_ctx: &Rc<Mutex<StatusbarIOContext<'_>>>,
) -> Result<(), Box<dyn Error>> {
    let line = "  [\n\
        \x20   {\n\
        \x20     \"full_text\": \""
        .chars()
        .chain(EscapeJSONString::new_from_str(
            &data.battery().to_string(),
            MinimalEscaping(),
        ))
        .chain(
            "\",\n\
        \x20     \"min_width\": \""
                .chars(),
        )
        .chain(EscapeJSONString::new_from_str("000%", MinimalEscaping()))
        .chain(
            "\"\n\
        \x20   },\n\
        \x20   {\n\
        \x20     \"full_text\": \""
                .chars(),
        )
        .chain(EscapeJSONString::new_from_str(
            &data.time().to_string(),
            MinimalEscaping(),
        ))
        .chain(
            "\",\n\
        \x20     \"short_text\": \""
                .chars(),
        )
        .chain(EscapeJSONString::new_from_str(
            &ShortenedDTD(data.time()).to_string(),
            MinimalEscaping(),
        ))
        .chain(
            "\",\n\
        \x20     \"min_width\": \""
                .chars(),
        )
        .chain(EscapeJSONString::new_from_str("00:00", MinimalEscaping()))
        .chain(
            "\"\n\
        \x20   }\n\
        \x20 ],\n"
                .chars(),
        )
        .collect::<String>();

    let output = &mut io_ctx.lock().await.statusbar_output;

    output.write_all(line.as_bytes()).await?;
    output.flush().await?;

    Ok(())
}

async fn print_infinite_body(
    mut change_q: Receiver<StatusbarChangeCause>,
    io_ctx: Rc<Mutex<StatusbarIOContext<'_>>>,
) -> Result<(), Box<dyn Error>> {
    print_body_begin(&io_ctx).await?;

    let mut data = StatusbarData::new();
    let mut buf = Vec::with_capacity(4);

    loop {
        print_status_line(&data, &io_ctx).await?;

        loop {
            match change_q.recv_many(&mut buf, 128).await {
                0 => {
                    // No more, Senders must be shut down. I guess it's time
                    // to close up.
                    return Ok(());
                }
                _ => {
                    // Process a bunch of our messages before rerendering.
                    for msg in buf.drain(..) {
                        match msg {
                            TzChange(tz_change) => {
                                data.update_timezone_maybedata(tz_change);
                            }
                            BatteryChange(bat_change) => {
                                data.update_battery_maybedata(bat_change);
                            }
                            _ => {}
                        }
                    }
                }
            }

            // See if there's more messages that we should process
            // before printing out the new status line.
            if change_q.is_empty() {
                break;
            }
        }
    }
}

pub async fn run_statusbar_updater(
    change_q: Receiver<StatusbarChangeCause>,
    io_ctx: Rc<Mutex<StatusbarIOContext<'_>>>,
) -> Result<(), Box<dyn Error>> {
    print_header(&io_ctx).await?;

    print_infinite_body(change_q, io_ctx).await?;

    Ok(())
}

#[cfg(test)]
mod jsontests;
