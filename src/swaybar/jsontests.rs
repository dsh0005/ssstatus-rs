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

use crate::swaybar::json::*;

#[derive(Clone, Debug)]
struct InputAndExpectedResult {
    input: String,
    expected_result: String,
}

#[derive(Clone, Debug)]
struct EscapingResults {
    parameters: InputAndExpectedResult,
    result: String,
}

#[test]
fn check_non_escapes() {
    let test_vectors = vec![
        InputAndExpectedResult {
            input: "".to_string(),
            expected_result: "".to_string(),
        },
        InputAndExpectedResult {
            input: "some text".to_string(),
            expected_result: "some text".to_string(),
        },
    ];

    let test_results = test_vectors
        .into_iter()
        .map(|test_vector| EscapingResults {
            result: EscapeJSONString::new_from_str(&test_vector.input, minimal_escaping)
                .collect::<String>(),
            parameters: test_vector,
        })
        .collect::<Vec<_>>();

    for test_result in test_results {
        assert_eq!(test_result.parameters.expected_result, test_result.result);
    }
}

#[test]
fn check_control_escapes() {
    let selected_vectors = vec![
        InputAndExpectedResult {
            input: "\x00".to_string(),
            expected_result: "\\u0000".to_string(),
        },
        InputAndExpectedResult {
            input: "some\x1btext".to_string(),
            expected_result: "some\\u001Btext".to_string(),
        },
    ];

    let selected_results = selected_vectors
        .into_iter()
        .map(|test_vector| EscapingResults {
            result: EscapeJSONString::new_from_str(&test_vector.input, minimal_escaping)
                .collect::<String>(),
            parameters: test_vector,
        })
        .collect::<Vec<_>>();

    for res in selected_results {
        assert_eq!(res.parameters.expected_result, res.result);
    }

    // TODO: make sure all control characters get escaped
}

#[test]
fn check_mandatory_escapes() {
    let always_needs_escape_vectors = vec![
        InputAndExpectedResult {
            input: "\x00".to_string(),
            expected_result: "\\u0000".to_string(),
        },
        InputAndExpectedResult {
            input: "some\x1btext".to_string(),
            expected_result: "some\\u001Btext".to_string(),
        },
        InputAndExpectedResult {
            input: "\"".to_string(),
            expected_result: "\\\"".to_string(),
        },
        InputAndExpectedResult {
            input: "\\".to_string(),
            expected_result: "\\\\".to_string(),
        },
    ];

    let should_always_escape_results = always_needs_escape_vectors
        .into_iter()
        .map(|test_vector| EscapingResults {
            result: EscapeJSONString::new_from_str(&test_vector.input, minimal_escaping)
                .collect::<String>(),
            parameters: test_vector,
        })
        .collect::<Vec<_>>();

    for res in should_always_escape_results {
        assert_eq!(res.parameters.expected_result, res.result);
    }
}
