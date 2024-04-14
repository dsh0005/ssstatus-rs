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

pub struct MaybeData<T>(pub Result<Option<(Instant, T)>, Box<dyn Error>>);

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
                Some((_timestamp, tz)) => DateTimeData(Ok(Some(Utc::now().with_timezone(&tz)))),
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
}

impl fmt::Display for StatusbarData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} | {}", self.battery, self.time())
    }
}
// TODO: define an RCU structure?
