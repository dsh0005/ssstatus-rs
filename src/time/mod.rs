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

use crate::StatusbarIOContext;
use chrono::{DurationRound, Local, TimeDelta};
use std::sync::Arc;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::time::sleep;

pub async fn wait_till_next_minute<SBO, DO>(
    io_ctx: Arc<Mutex<StatusbarIOContext<SBO, DO>>>,
) -> Result<(), Box<dyn Error>>
where
    SBO: AsyncWrite + Unpin,
    DO: AsyncWrite + Unpin,
{
    let start = Local::now();
    let half_minute = TimeDelta::seconds(30);
    let minute = TimeDelta::minutes(1);
    let next_minute = (start + half_minute).duration_round(minute)?;

    let sleep_duration = next_minute - start;
    let std_sleep_duration = sleep_duration.to_std()?;

    if cfg!(feature = "debug_sleep") {
        let output = &mut io_ctx.lock().await.debug_output;

        output
            .write_all(
                format!(
                    "start wait at {}\nwait until {}\nexpected duration {}\n",
                    start, next_minute, sleep_duration
                )
                .as_bytes(),
            )
            .await?;
        output.flush().await?;
    }

    sleep(std_sleep_duration).await;

    let finish = Local::now();

    if cfg!(feature = "debug_sleep") {
        let output = &mut io_ctx.lock().await.debug_output;

        output
            .write_all(format!("finish wait: {}\n", finish).as_bytes())
            .await?;
        output.flush().await?;
    }

    Ok(())
}

use chrono::Utc;
use libc::ECANCELED;
use std::io;
use std::panic;
use timerfd::{ClockId, SetTimeFlags, TimerFd, TimerState};
use tokio::io::unix::AsyncFd;

pub trait ClockChangedCallback {
    async fn clock_change_maybe_lost(&self) -> Result<(), Box<dyn Error>>;
}

fn get_abs_utc_time_in_future(time: TimeDelta) -> Result<std::time::Duration, Box<dyn Error>> {
    Ok((Utc::now() + time)
        .signed_duration_since(DateTime::<Utc>::UNIX_EPOCH)
        .to_std()?)
}

pub async fn wait_till_time_change(
    callback: &impl ClockChangedCallback,
) -> Result<(), Box<dyn Error>> {
    // The timerfd crate makes the TCOS flag imply the Abstime flag.
    let listen_flags = SetTimeFlags::TimerCancelOnSet;

    // We need to choose some time in the future to wait on, that's
    // just how timerfd TCOS works. So choose something silly
    // infrequent.
    let wait_far_into_future =
        TimerState::Oneshot(get_abs_utc_time_in_future(TimeDelta::weeks(1))?);

    let mut tfd = TimerFd::new_custom(ClockId::Realtime, true, true)?;
    tfd.set_state(wait_far_into_future, listen_flags.clone());
    let mut tok_afd = AsyncFd::new(tfd)?;

    // We just set the timer, so we'll catch changes from now on,
    // but we might have missed one earlier.
    callback.clock_change_maybe_lost().await?;

    loop {
        match tok_afd.readable_mut().await {
            Ok(mut guard) => {
                let read_res = guard.try_io(|tim: &mut AsyncFd<TimerFd>| {
                    let t = &tim.get_mut();
                    match panic::catch_unwind(|| t.read()) {
                        Ok(0) => io::Result::Err(std::io::ErrorKind::WouldBlock.into()),
                        Ok(_) => {
                            // The timer expired, we need to set it farther in the future.
                            // We'll do so outside of this try_io.
                            Ok(false)
                        }
                        Err(cause) => {
                            if let Some(msg) = cause.downcast_ref::<&str>() {
                                if *msg == format!("Unexpected read error: {}", ECANCELED) {
                                    // The clock got changed.
                                    return Ok(true);
                                }
                            }
                            io::Result::Err(io::Error::other(
                                "some panic from timerfd::TimerFd::read()",
                            ))
                        }
                    }
                });

                match read_res {
                    Err(_) => {
                        // Good! Wait some more!
                    }
                    Ok(Ok(true)) => {
                        // The clock got changed.
                        return Ok(());
                    }
                    Ok(Ok(false)) => {
                        // The timer expired, and _we_ need to set it farther out in the
                        // future.
                        guard.get_inner_mut().set_state(
                            TimerState::Oneshot(get_abs_utc_time_in_future(TimeDelta::weeks(1))?),
                            listen_flags.clone(),
                        );

                        // Again, since we've now set the timer, we'll catch all the
                        // changes, but we might have missed some in that short window.
                        // Signal as such.
                        callback.clock_change_maybe_lost().await?;
                        // Circle back around.
                    }
                    Ok(Err(e)) => {
                        return Err(Box::new(e));
                    }
                }
            }
            Err(e) => match e.raw_os_error() {
                None => {
                    // TODO: log? panic?
                    return Err(Box::new(e));
                }
                Some(ose) => match ose {
                    ECANCELED => {
                        // The clock got changed.
                        return Ok(());
                    }
                    _ => {
                        // TODO: log? panic?
                        return Err(Box::new(e));
                    }
                },
            },
        }
    }
}
