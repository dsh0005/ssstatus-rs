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

/**
 * What to do next when evaluating an input char.
 */
#[derive(Copy, Clone, Debug)]
pub enum EscapeJSONDecision {
    /**
     * Print the input char out directly.
     */
    PrintDirectly(),
    /**
     * Print out a unicode escape sequence. It might be a surrogate
     * pair.
     */
    UnicodeEscape(),
    /**
     * Print out a single char escape sequence with a different char
     * substituting for the input char, e.g. `n` for `\n`.
     */
    SingleCharEscape(),
    /**
     * Print out a single char escape sequence with the input char in
     * it, e.g. `"` for `\"`.
     */
    SelfEscape(),
}

/**
 * The states for JSON string escapes.
 */
#[derive(Copy, Clone, Debug)]
enum EscapeJSONState {
    /**
     * We're not in an escape, we need to look at the next character
     * before we know what to output next.
     */
    NotEscaping(),
    /**
     * We're in an escape sequence where we're putting out a `\uXXXX`
     * sequence, possibly as a surrogate pair. The u8 is how far we are
     * into the escape, with the initial `\` being 0. The char is the
     * input char.
     */
    InUnicodeEscape((u8, char)),
    /**
     * We're in an escape sequence where we've output a `\`, and the
     * next char is something besides the input char, e.g. `n` in `\n`.
     * The char is the input char.
     */
    InSingleCharEscape(char),
    /**
     * We've output a `\`, and the next char to output is the input
     * char itself, e.g. `"` in `\"`. The char is the input char.
     */
    HaveEscaped(char),
    /**
     * We've reached the end of the string.
     */
    EndOfString(),
}

use EscapeJSONDecision::*;

fn rfc_single_char_escape(c: char) -> Option<char> {
    match c {
        // These aren't really single char escapes by the terminology
        // of EscapeJSONDecision, but we'll allow them.
        '"' => Some('"'),
        '\\' => Some('\\'),
        '/' => Some('/'),

        '\x08' => Some('b'), // backspace
        '\x0c' => Some('f'), // form feed
        '\n' => Some('n'),   // line feed
        '\r' => Some('r'),   // carriage return
        '\t' => Some('t'),   // tab

        _ => None,
    }
}

/**
 * Perform minimal escaping, according to IETF RFC 8259. This escapes
 * quotation mark, reverse solidus, and code points 0x00-0x1F. No other
 * characters are requested to be escaped.
 */
pub fn minimal_escaping(c: &char) -> EscapeJSONDecision {
    match c {
        '"' | '\\' => SelfEscape(),
        '\x00'..='\x1f' => UnicodeEscape(),
        _ => PrintDirectly(),
    }
}

pub type StringEscapePolicy = fn(&char) -> EscapeJSONDecision;

use std::str::Chars;

#[derive(Clone, Debug)]
pub struct EscapeJSONString<'a> {
    input: Chars<'a>,
    state: EscapeJSONState,
    policy: StringEscapePolicy,
}

use EscapeJSONState::*;

impl<'a> EscapeJSONString<'a> {
    pub fn new_from_str(s: &'a str, policy: StringEscapePolicy) -> Self {
        EscapeJSONString {
            input: s.chars(),
            state: EscapeJSONState::NotEscaping(),
            policy,
        }
    }

    fn next_char(&mut self) -> Option<char> {
        match self.state {
            EndOfString() => None,
            HaveEscaped(c) => {
                self.state = NotEscaping();
                Some(c)
            }
            InSingleCharEscape(c) => {
                let escaped = rfc_single_char_escape(c).expect("We should not be doing a single char escape for a char that does not have a known single char escape.");
                self.state = NotEscaping();
                Some(escaped)
            }
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

                    let which_hexdig_in_surrogate = surrogate_finish_count - count;

                    let hexdig_value = (surrogate >> ((which_hexdig_in_surrogate) * 4)) & 0xf;

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
                Some(c) => match (self.policy)(&c) {
                    PrintDirectly() => Some(c),
                    UnicodeEscape() => {
                        self.state = InUnicodeEscape((1, c));
                        Some('\\')
                    }
                    SingleCharEscape() => {
                        self.state = InSingleCharEscape(c);
                        Some('\\')
                    }
                    SelfEscape() => {
                        self.state = HaveEscaped(c);
                        Some('\\')
                    }
                },
            },
        }
    }
}

impl Iterator for EscapeJSONString<'_> {
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
        let max_remaining = backing_hint
            .1
            .map(|amount| amount * 12 + remaining_from_current_state);

        (min_remaining, max_remaining)
    }
}

use std::iter::FusedIterator;

impl FusedIterator for EscapeJSONString<'_> {}
