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

pub enum JSONSafeString<'a> {
    AlreadySafe(&'a str),
    MadeSafe(String),
}

use JSONSafeString::*;

fn json_char_needs_minimal_string_escaping(c: char) -> bool {
    match c {
        '\x00'..='\x1f' | '"' | '\\' => true,
        _ => false,
    }
}

fn json_string_is_minimal_safe(input: &str) -> bool {
    !input.chars().any(json_char_needs_minimal_string_escaping)
}

enum EscapeJSONState {
    NotEscaping(),
    InUnicodeEscape((u8, char)),
    InSingleCharEscape(char),
    HaveEscaped(char),
    EndOfString(),
}

use std::str::Chars;

struct EscapeJSONMinimal<'a> {
    input: Chars<'a>,
    state: EscapeJSONState,
}

use EscapeJSONState::*;

impl<'a> EscapeJSONMinimal<'a> {
    pub fn new_from_str(s: &'a str) -> Self {
        EscapeJSONMinimal {
            input: s.chars(),
            state: EscapeJSONState::NotEscaping(),
        }
    }

    fn next_char(&mut self) -> Option<char> {
        match self.state {
            EndOfString() => None,
            HaveEscaped(c) => Some(c),
            InSingleCharEscape(_) => unimplemented!(),
            InUnicodeEscape((count, c @ '\u{0000}'..='\u{ffff}')) => match count {
                0 => unreachable!("This is handled in the NotEscaping case."),
                1 => {
                    self.state = InUnicodeEscape((2, c));
                    Some('u')
                }
                count @ 2..=5 => {
                    let code_point =
                        u16::try_from(c).expect("We just checked the char is in the BMP.");

                    let hexdig_value = (code_point >> ((5 - count) * 4)) & 0xf;

                    let hexdig = match hexdig_value {
                        x @ 0..=9 => char::from(
                            u8::try_from('0').expect("'0' fits in a u8")
                                + u8::try_from(x).expect("We just checked the value of x."),
                        ),
                        x @ 10..=15 => char::from(
                            u8::try_from('A').expect("'A' fits in a u8")
                                + u8::try_from(x - 10).expect("We just checked the value of x."),
                        ),
                        _ => unreachable!("u16 & 0xf should not be more than 15."),
                    };

                    if count == 5 {
                        self.state = NotEscaping();
                    } else {
                        self.state = InUnicodeEscape((count + 1, c));
                    }

                    Some(hexdig)
                }
                6.. => unreachable!("We should not get here."),
            },
            InUnicodeEscape((count, c @ '\u{10000}'..)) => match count {
                0 => unreachable!("This is handled in the NotEscaping case."),
                1 => {
                    self.state = InUnicodeEscape((2, c));
                    Some('u')
                }
                6 => {
                    self.state = InUnicodeEscape((7, c));
                    Some('\\')
                }
                7 => {
                    self.state = InUnicodeEscape((8, c));
                    Some('u')
                }
                count @ (2..=5 | 8..=11) => {
                    let mut utf16_encoded = [0; 2];
                    c.encode_utf16(&mut utf16_encoded);

                    let (surrogate, surrogate_finish_count) = match count {
                        2..=5 => (utf16_encoded[0], 5),
                        8..=11 => (utf16_encoded[1], 11),
                        _ => unreachable!("Match results are inconsistent."),
                    };

                    let hexdig_value = (surrogate >> ((11 - surrogate_finish_count) * 4)) & 0xf;

                    let hexdig = match hexdig_value {
                        x @ 0..=9 => char::from(
                            u8::try_from('0').expect("'0' fits in a u8")
                                + u8::try_from(x).expect("We just checked the value of x."),
                        ),
                        x @ 10..=15 => char::from(
                            u8::try_from('A').expect("'A' fits in a u8")
                                + u8::try_from(x - 10).expect("We just checked the value of x."),
                        ),
                        _ => unreachable!("u16 & 0xf should not be more than 15."),
                    };

                    if count == 11 {
                        self.state = NotEscaping();
                    } else {
                        self.state = InUnicodeEscape((count + 1, c));
                    }

                    Some(hexdig)
                }
                12.. => unreachable!("We should not get here."),
            },
            NotEscaping() => match self.input.next() {
                None => {
                    self.state = EndOfString();
                    None
                }
                Some(needs_simple_escape @ ('"' | '\\')) => {
                    self.state = HaveEscaped(needs_simple_escape);
                    Some('\\')
                }
                Some(needs_unicode_escape @ '\x00'..='\x1f') => {
                    self.state = InUnicodeEscape((1, needs_unicode_escape));
                    Some('\\')
                }
                Some(c) => Some(c),
            },
        }
    }
}

impl<'a> Iterator for EscapeJSONMinimal<'a> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        self.next_char()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let backing_hint = self.input.size_hint();

        let remaining_from_current_state: usize = match self.state {
            EndOfString() => 0,
            NotEscaping() => 0,
            HaveEscaped(_) => 1,
            InSingleCharEscape(_) => 1,
            InUnicodeEscape((count @ 1..=5, '\u{0000}'..='\u{ffff}')) => (6 - count).into(),
            InUnicodeEscape((0 | 6.., '\u{0000}'..='\u{ffff}')) => {
                unreachable!("Invalid count/state.")
            }
            InUnicodeEscape((count @ 1..=11, '\u{10000}'..)) => (12 - count).into(),
            InUnicodeEscape((0 | 12.., '\u{10000}'..)) => unreachable!("Invalid count/state."),
        };

        let min_remaining = backing_hint.0 + remaining_from_current_state;
        let max_remaining = match backing_hint.1 {
            None => None,
            Some(amount) => Some(amount * 12 + remaining_from_current_state),
        };

        (min_remaining, max_remaining)
    }
}

use std::iter::FusedIterator;

impl<'a> FusedIterator for EscapeJSONMinimal<'a> {}

impl<'a> From<&'a str> for JSONSafeString<'a> {
    fn from(input: &'a str) -> Self {
        if json_string_is_minimal_safe(input) {
            return AlreadySafe(input);
        }

        MadeSafe(String::from_iter(EscapeJSONMinimal::new_from_str(input)))
    }
}

impl<'a> From<&'a String> for JSONSafeString<'a> {
    fn from(input: &'a String) -> Self {
        if json_string_is_minimal_safe(input) {
            return AlreadySafe(input);
        }

        MadeSafe(String::from_iter(EscapeJSONMinimal::new_from_str(input)))
    }
}

impl From<String> for JSONSafeString<'_> {
    fn from(input: String) -> Self {
        MadeSafe(String::from_iter(EscapeJSONMinimal::new_from_str(&input)))
    }
}

use std::fmt;

impl fmt::Display for JSONSafeString<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AlreadySafe(s) => write!(f, "{}", s),
            MadeSafe(s) => write!(f, "{}", s),
        }
    }
}
