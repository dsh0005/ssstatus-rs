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

use chrono::Utc;
use chrono_tz::Tz;
use std::error::Error;
use std::fmt;
use std::time::Instant;

use crate::time::DateTimeData;

pub mod battery;

use battery::BatteryStatus;

pub struct MaybeData<T>(pub Result<Option<(Instant, T)>, Box<dyn Error + Send + Sync>>);

impl<T: fmt::Display> fmt::Display for MaybeData<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.0 {
            Ok(opt) => match opt {
                Some((_timestamp, val)) => write!(f, "{}", val),
                None => write!(f, "None"),
            },
            Err(e) => write!(f, "{}", e),
        }
    }
}

pub struct StatusbarData {
    battery: MaybeData<BatteryStatus>,
    timezone: MaybeData<Tz>,
}

impl StatusbarData {
    pub fn time(&self) -> DateTimeData<Tz> {
        match &self.timezone.0 {
            Ok(opt) => match opt {
                Some((_timestamp, tz)) => DateTimeData(Ok(Some(Utc::now().with_timezone(tz)))),
                None => DateTimeData(Ok(None)),
            },
            Err(e) => DateTimeData(Err(e.to_string().into())),
        }
    }

    pub fn new() -> StatusbarData {
        StatusbarData {
            battery: MaybeData(Ok(None)),
            timezone: MaybeData(Ok(None)),
        }
    }

    pub fn update_battery_maybedata(&mut self, bat: MaybeData<BatteryStatus>) {
        self.battery = bat;
    }

    pub fn update_battery_result(
        &mut self,
        bat: Result<BatteryStatus, Box<dyn Error + Send + Sync>>,
    ) {
        match bat {
            Ok(status) => self.update_battery(status),
            Err(e) => self.battery = MaybeData(Err(e)),
        }
    }

    pub fn update_battery(&mut self, bat: BatteryStatus) {
        self.battery = MaybeData(Ok(Some((Instant::now(), bat))))
    }

    pub fn update_timezone_maybedata(&mut self, tz: MaybeData<Tz>) {
        self.timezone = tz;
    }

    pub fn update_timezone_result(&mut self, tz: Result<Tz, Box<dyn Error + Send + Sync>>) {
        match tz {
            Ok(tz) => self.update_timezone(tz),
            Err(e) => self.timezone = MaybeData(Err(e)),
        }
    }

    pub fn update_timezone(&mut self, tz: Tz) {
        self.timezone = MaybeData(Ok(Some((Instant::now(), tz))))
    }
}

impl fmt::Display for StatusbarData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} | {}", self.battery, self.time())
    }
}

pub enum StatusbarChangeCause {
    // We're at the next minute.
    NextMinute,

    // The timezone changed.
    TzChange(MaybeData<Tz>),

    // The clock was (maybe) adjusted.
    ClockAdjust,

    // The battery status changed.
    BatteryChange(MaybeData<BatteryStatus>),
}

// TODO: define an RCU structure?
