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

use std::collections::HashMap;

use crate::swaybar::json::*;

use EscapePolicy::*;

#[derive(Clone, Debug)]
struct InputAndExpectedResults {
    input: String,
    expected_results: HashMap<EscapePolicy, String>,
}

#[derive(Clone, Debug)]
struct ExpectedAndResult {
    expected: String,
    result: String,
}

#[derive(Clone, Debug)]
struct EscapingResults {
    input: String,
    results: HashMap<EscapePolicy, ExpectedAndResult>,
}

#[test]
fn check_non_escapes() {
    let test_vectors = vec![
        InputAndExpectedResults {
            input: "".to_string(),
            expected_results: HashMap::from([(MinimalEscaping(), "".to_string())]),
        },
        InputAndExpectedResults {
            input: "some text".to_string(),
            expected_results: HashMap::from([(MinimalEscaping(), "some text".to_string())]),
        },
    ];

    let test_results = test_vectors
        .into_iter()
        .map(|test_vector| EscapingResults {
            results: test_vector
                .expected_results
                .into_iter()
                .map(|policy_and_expected| {
                    (
                        policy_and_expected.0,
                        ExpectedAndResult {
                            expected: policy_and_expected.1,
                            result: EscapeJSONString::new_from_str(
                                &test_vector.input,
                                policy_and_expected.0,
                            )
                            .collect::<String>(),
                        },
                    )
                })
                .collect::<HashMap<_, _>>(),
            input: test_vector.input,
        })
        .collect::<Vec<_>>();

    for test_result in test_results {
        for (policy, e_and_r) in test_result.results {
            assert_eq!(e_and_r.expected, e_and_r.result);
        }
    }
}

#[test]
fn check_control_escapes() {
    let selected_vectors = vec![
        InputAndExpectedResults {
            input: "\x00".to_string(),
            expected_results: HashMap::from([(MinimalEscaping(), "\\u0000".to_string())]),
        },
        InputAndExpectedResults {
            input: "some\x1btext".to_string(),
            expected_results: HashMap::from([(MinimalEscaping(), "some\\u001Btext".to_string())]),
        },
    ];

    let selected_results = selected_vectors
        .into_iter()
        .map(|test_vector| EscapingResults {
            results: test_vector
                .expected_results
                .into_iter()
                .map(|policy_and_expected| {
                    (
                        policy_and_expected.0,
                        ExpectedAndResult {
                            expected: policy_and_expected.1,
                            result: EscapeJSONString::new_from_str(
                                &test_vector.input,
                                policy_and_expected.0,
                            )
                            .collect::<String>(),
                        },
                    )
                })
                .collect::<HashMap<_, _>>(),
            input: test_vector.input,
        })
        .collect::<Vec<_>>();

    for res in selected_results {
        for (policy, e_and_r) in res.results {
            assert_eq!(e_and_r.expected, e_and_r.result);
        }
    }

    // TODO: make sure all control characters get escaped
}

#[test]
fn check_mandatory_escapes() {
    let always_needs_escape_vectors = vec![
        InputAndExpectedResults {
            input: "\x00".to_string(),
            expected_results: HashMap::from([(MinimalEscaping(), "\\u0000".to_string())]),
        },
        InputAndExpectedResults {
            input: "some\x1btext".to_string(),
            expected_results: HashMap::from([(MinimalEscaping(), "some\\u001Btext".to_string())]),
        },
        InputAndExpectedResults {
            input: "\"".to_string(),
            expected_results: HashMap::from([(MinimalEscaping(), "\\\"".to_string())]),
        },
        InputAndExpectedResults {
            input: "\\".to_string(),
            expected_results: HashMap::from([(MinimalEscaping(), "\\\\".to_string())]),
        },
    ];

    let should_always_escape_results = always_needs_escape_vectors
        .into_iter()
        .map(|test_vector| EscapingResults {
            results: test_vector
                .expected_results
                .into_iter()
                .map(|policy_and_expected| {
                    (
                        policy_and_expected.0,
                        ExpectedAndResult {
                            expected: policy_and_expected.1,
                            result: EscapeJSONString::new_from_str(
                                &test_vector.input,
                                policy_and_expected.0,
                            )
                            .collect::<String>(),
                        },
                    )
                })
                .collect::<HashMap<_, _>>(),
            input: test_vector.input,
        })
        .collect::<Vec<_>>();

    for res in should_always_escape_results {
        for (policy, e_and_r) in res.results {
            assert_eq!(e_and_r.expected, e_and_r.result);
        }
    }
}
