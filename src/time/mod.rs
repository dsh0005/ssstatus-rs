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

use chrono::{DateTime, TimeZone};
use chrono_tz::Tz;
use std::error::Error;
use std::fmt;

pub struct DateTimeData<Tz: TimeZone>(
    pub Result<Option<DateTime<Tz>>, Box<dyn Error + Send + Sync>>,
);

impl fmt::Display for DateTimeData<Tz> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.0 {
            Ok(opt) => match opt {
                Some(date_time) => write!(f, "{}", date_time.format("%Y-%m-%d %H:%M")),
                None => write!(f, "none"),
            },
            Err(e) => write!(f, "{}", e),
        }
    }
}

pub struct ShortenedDTD<Tz: TimeZone>(pub DateTimeData<Tz>);

impl fmt::Display for ShortenedDTD<Tz> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.0.0 {
            Ok(Some(date_time)) => write!(f, "{}", date_time.format("%H:%M")),
            _ => write!(f, "none"),
        }
    }
}

use crate::StatusbarIOContext;
use chrono::{DurationRound, Local, TimeDelta};
use nix::errno::Errno::{self, EAGAIN, ECANCELED};
use nix::sys::time::TimeSpec;
use nix::sys::timerfd::{ClockId, Expiration, TimerFd, TimerFlags, TimerSetTimeFlags};
use std::convert::Infallible;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsFd;
use std::sync::Arc;
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncWrite, AsyncWriteExt, Interest, Ready};
use tokio::sync::Mutex;

fn get_next_minute_absolute_timespec() -> Result<TimeSpec, Box<dyn Error>> {
    let start = Local::now();
    let half_minute = TimeDelta::seconds(30);
    let minute = TimeDelta::minutes(1);
    let next_minute = (start + half_minute).duration_round(minute)?;

    let next_minute_timespec = TimeSpec::new(
        next_minute.timestamp(),
        next_minute.timestamp_subsec_nanos().into(),
    );

    Ok(next_minute_timespec)
}

pub trait ClockTickCallbacks {
    async fn changed_minute(&self) -> Result<(), Box<dyn Error>>;
    async fn minute_maybe_lost(&self) -> Result<(), Box<dyn Error>>;
    async fn adjustment_happened(&self) -> Result<(), Box<dyn Error>>;
}

pub async fn tick_every_minute<SBO, DO>(
    io_ctx: Arc<Mutex<StatusbarIOContext<SBO, DO>>>,
    clock_tick_callbacks: &impl ClockTickCallbacks,
) -> Result<Infallible, Box<dyn Error>>
where
    SBO: AsyncWrite + Unpin,
    DO: AsyncWrite + Unpin,
{
    let next_tick = get_next_minute_absolute_timespec()?;
    let tick_period = TimeSpec::new(60, 0);

    let listen_flags =
        TimerSetTimeFlags::TFD_TIMER_ABSTIME | TimerSetTimeFlags::TFD_TIMER_CANCEL_ON_SET;

    let wait_for_minute = Expiration::IntervalDelayed(next_tick, tick_period);

    let tfd = TimerFd::new(
        ClockId::CLOCK_REALTIME,
        TimerFlags::TFD_NONBLOCK | TimerFlags::TFD_CLOEXEC,
    )?;
    tfd.set(wait_for_minute, listen_flags)?;

    let copy_tfd = File::from(tfd.as_fd().try_clone_to_owned().unwrap());

    let tok_afd = AsyncFd::with_interest(copy_tfd, Interest::READABLE | Interest::ERROR)?;

    // We just set the timer, so we'll catch changes from now on,
    // but we might have missed one earlier.
    clock_tick_callbacks.minute_maybe_lost().await?;

    loop {
        match tok_afd.readable().await {
            Ok(mut guard) => {
                let mut buf: [u8; 8] = [0; 8];
                let read_res = tok_afd.get_ref().read(&mut buf);

                match read_res {
                    Ok(_) => {
                        if cfg!(feature = "debug_sleep") {
                            let output = &mut io_ctx.lock().await.debug_output;
                            output
                                .write_all("Got Ok(()) from timerfd\n".as_bytes())
                                .await?;
                            output.flush().await?;
                        }

                        // We hit the next minute.
                        clock_tick_callbacks.changed_minute().await?;

                        guard.retain_ready();
                    }
                    Err(err) => match err.raw_os_error().map(Errno::from_raw) {
                        Some(ECANCELED) => {
                            if cfg!(feature = "debug_sleep") {
                                let output = &mut io_ctx.lock().await.debug_output;
                                output
                                    .write_all("Got ECANCELED from timerfd\n".as_bytes())
                                    .await?;
                                output.flush().await?;
                            }

                            // The clock got changed, so the timer got canceled.
                            // Set it to the next minute, _then_ fire the callback.

                            let next_tick = get_next_minute_absolute_timespec()?;
                            let wait_for_minute =
                                Expiration::IntervalDelayed(next_tick, tick_period);

                            tfd.set(wait_for_minute, listen_flags)?;

                            clock_tick_callbacks.adjustment_happened().await?;

                            guard.clear_ready_matching(Ready::ERROR);
                        }
                        Some(EAGAIN) => {
                            if cfg!(feature = "debug_sleep") {
                                let output = &mut io_ctx.lock().await.debug_output;
                                output
                                    .write_all("Got EAGAIN from timerfd\n".as_bytes())
                                    .await?;
                                output.flush().await?;
                            }

                            guard.clear_ready_matching(Ready::READABLE);

                            // Circle back around.
                        }
                        Some(eno) => {
                            if cfg!(feature = "debug_sleep") {
                                let output = &mut io_ctx.lock().await.debug_output;
                                output
                                    .write_all(
                                        format!("Got error {:?} from timerfd\n", eno).as_bytes(),
                                    )
                                    .await?;
                                output.flush().await?;
                            }

                            guard.clear_ready_matching(Ready::ERROR);
                            return Err(eno.into());
                        }
                        None => {
                            if cfg!(feature = "debug_sleep") {
                                let output = &mut io_ctx.lock().await.debug_output;
                                output
                                    .write_all(
                                        format!(
                                            "Got rust-exclusive error {:?} from timerfd\n",
                                            err
                                        )
                                        .as_bytes(),
                                    )
                                    .await?;
                                output.flush().await?;
                            }

                            return Err(Box::new(err));
                        }
                    },
                }
            }
            Err(e) => match Errno::try_from(e) {
                Err(unconverted) => {
                    // TODO: log? panic?
                    return Err(Box::new(unconverted));
                }
                Ok(eno) => match eno {
                    ECANCELED => {
                        // TODO: is this unreachable?

                        // The clock got changed, so the timer got canceled.
                        // Set it to the next minute, _then_ fire the callback.

                        let next_tick = get_next_minute_absolute_timespec()?;
                        let wait_for_minute = Expiration::IntervalDelayed(next_tick, tick_period);

                        tfd.set(wait_for_minute, listen_flags)?;

                        clock_tick_callbacks.adjustment_happened().await?;
                    }
                    eno => {
                        // TODO: log? panic?
                        return Err(eno.into());
                    }
                },
            },
        }
    }
}
