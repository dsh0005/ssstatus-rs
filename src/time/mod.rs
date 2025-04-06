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
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::sync::Arc;
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncWrite, AsyncWriteExt, Interest};
use tokio::sync::Mutex;
use tokio::time::sleep;

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

    let borrow_tfd = tfd.as_fd();

    let tok_afd = AsyncFd::with_interest(borrow_tfd, Interest::READABLE | Interest::ERROR)?;

    // We just set the timer, so we'll catch changes from now on,
    // but we might have missed one earlier.
    clock_tick_callbacks.minute_maybe_lost().await?;

    loop {
        match tok_afd.readable().await {
            Ok(mut guard) => {
                let read_res = guard.try_io(|_borrowed_timer: &AsyncFd<BorrowedFd>| {
                    match &tfd.wait() {
                        Ok(()) => {
                            // The timer ticked over, we'll fire the callback
                            // in a moment.
                            Ok(false)
                        }
                        Err(EAGAIN) => {
                            // It's tokio's job now.
                            io::Result::Err(std::io::ErrorKind::WouldBlock.into())
                        }
                        Err(ECANCELED) => {
                            // Hey, the clock changed!
                            Ok(true)
                        }
                        Err(eno) => Err((*eno).into()),
                    }
                });

                match read_res {
                    Err(_) => {
                        // No ticks yet, wait some more.
                        // TODO: is this unreachable?
                    }
                    Ok(Ok(true)) => {
                        // The clock got changed, so the timer got canceled.
                        // Set it to the next minute, _then_ fire the callback.

                        let next_tick = get_next_minute_absolute_timespec()?;
                        let wait_for_minute = Expiration::IntervalDelayed(next_tick, tick_period);

                        tfd.set(wait_for_minute, listen_flags)?;

                        clock_tick_callbacks.adjustment_happened().await?;
                    }
                    Ok(Ok(false)) => {
                        // We hit the next minute.

                        clock_tick_callbacks.changed_minute().await?;

                        // Circle back around.
                    }
                    Ok(Err(e)) => {
                        return Err(Box::new(e));
                    }
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
