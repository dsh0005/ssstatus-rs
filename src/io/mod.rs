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

use tokio::io::AsyncWrite;

pub struct StatusbarIOContext<'a> {
    pub statusbar_output: Box<dyn AsyncWrite + Unpin + 'a>,
    pub debug_output: Box<dyn AsyncWrite + Unpin + 'a>,
}

impl<'a>
    From<(
        Box<dyn AsyncWrite + Unpin + 'a>,
        Box<dyn AsyncWrite + Unpin + 'a>,
    )> for StatusbarIOContext<'a>
{
    fn from(
        value: (
            Box<dyn AsyncWrite + Unpin + 'a>,
            Box<dyn AsyncWrite + Unpin + 'a>,
        ),
    ) -> Self {
        Self {
            statusbar_output: value.0,
            debug_output: value.1,
        }
    }
}
