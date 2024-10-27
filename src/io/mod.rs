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

use std::sync::Arc;
use tokio::io::AsyncWrite;
use tokio::sync::Mutex;

pub struct StatusbarIOContext<SBO: AsyncWrite + Unpin, DO: AsyncWrite + Unpin> {
    pub statusbarOutput: SBO,
    pub debugOutput: DO,
}

impl<SBO, DO> From<(SBO, DO)> for StatusbarIOContext<SBO, DO>
where
    SBO: AsyncWrite + Unpin,
    DO: AsyncWrite + Unpin,
{
    fn from(value: (SBO, DO)) -> Self {
        Self {
            statusbarOutput: value.0,
            debugOutput: value.1,
        }
    }
}

impl<O> From<(O,)> for StatusbarIOContext<O, O>
where
    O: AsyncWrite + Unpin + Clone,
{
    fn from(value: (O,)) -> Self {
        Self {
            statusbarOutput: value.0.clone(),
            debugOutput: value.0,
        }
    }
}

impl<O> From<O> for StatusbarIOContext<O, O>
where
    O: AsyncWrite + Unpin + Clone,
{
    fn from(value: O) -> Self {
        StatusbarIOContext::from((value,))
    }
}
