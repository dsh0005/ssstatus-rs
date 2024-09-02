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
                Some(date_time) => write!(f, "{}", date_time),
                None => write!(f, "none"),
            },
            Err(e) => write!(f, "{}", e),
        }
    }
}

use chrono::{DurationRound, Local, TimeDelta};
use tokio::time::sleep;

pub async fn wait_till_next_minute() -> Result<(), Box<dyn Error>> {
    let start = Local::now();
    let halfMinute = TimeDelta::seconds(30);
    let minute = TimeDelta::minutes(1);
    let nextMinute = (start + halfMinute).duration_round(minute)?;

    let sleepDuration = nextMinute - start;
    let stdSleepDuration = sleepDuration.to_std()?;

    println!("start wait at {}", start);
    println!("wait until {}", nextMinute);
    println!("expected duration {}", sleepDuration);

    sleep(stdSleepDuration).await;

    let finish = Local::now();
    println!("finish wait: {}", finish);

    Ok(())
}

use chrono::Utc;
use libc::ECANCELED;
use std::any::Any;
use std::io;
use std::panic;
use timerfd::{ClockId, SetTimeFlags, TimerFd, TimerState};
use tokio::io::unix::AsyncFd;

fn get_abs_utc_time_in_future(time: TimeDelta) -> Result<std::time::Duration, Box<dyn Error>> {
    Ok((Utc::now() + time)
        .signed_duration_since(DateTime::<Utc>::UNIX_EPOCH)
        .to_std()?)
}

pub async fn wait_till_time_change() -> Result<(), Box<dyn Error>> {
    // The timerfd crate make TCOS imply Abstime.
    let listen_flags = SetTimeFlags::TimerCancelOnSet;

    let wait_far_into_future =
        TimerState::Oneshot(get_abs_utc_time_in_future(TimeDelta::weeks(1))?);

    let mut tfd = TimerFd::new_custom(ClockId::Realtime, true, true)?;
    tfd.set_state(wait_far_into_future, listen_flags.clone());
    let mut tok_afd = AsyncFd::new(tfd)?;

    loop {
        match tok_afd.readable_mut().await {
            Ok(mut guard) => {
                let mut panic_cause = Option::<Box<dyn Any + Send>>::None;

                guard.try_io(|tim: &mut AsyncFd<TimerFd>| {
                    let mut t = &tim.get_mut();
                    match panic::catch_unwind(|| t.read()) {
                        Ok(0) => io::Result::Err(std::io::ErrorKind::WouldBlock.into()),
                        Ok(_) => {
                            // The timer expired, we need to set it farther in the future.
                            // TODO: set timer farther in the future
                            Ok(false)
                        }
                        Err(cause) => {
                            if let Some(msg) = cause.downcast_ref::<&str>() {
                                if *msg == format!("Unexpected read error: {}", ECANCELED) {
                                    // The clock got changed.
                                    return Ok(true);
                                }
                            }
                            panic_cause = Some(cause);
                            io::Result::Err(io::Error::other(
                                "some panic from timerfd::TimerFd::read()",
                            ))
                        }
                    }
                });

                if let Some(cause) = panic_cause {
                    panic::resume_unwind(cause);
                }

                guard.get_inner_mut().set_state(
                    TimerState::Oneshot(get_abs_utc_time_in_future(TimeDelta::weeks(1))?),
                    listen_flags.clone(),
                );
                // TODO: set the timer further in the future
            }
            Err(e) => match e.raw_os_error() {
                None => {
                    // TODO: log? panic?
                }
                Some(ose) => match ose {
                    ECANCELED => {
                        // The clock got changed.
                        return Ok(());
                    }
                    _ => {
                        // TODO: log? panic?
                    }
                },
            },
        }
    }

    Ok(())
}
