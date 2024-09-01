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

pub struct DateTimeData<Tz: TimeZone>(pub Result<Option<DateTime<Tz>>, Box<dyn Error>>);

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
