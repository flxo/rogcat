// Copyright © 2017 Felix Obenhuber
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crate::tests::utils::*;
use failure::Error;

const CONFIG: &str = "
[profile.A]
message = [\"A\"]

[profile.AB]
extends = [\"A\"]
message = [\"B\"]

[profile.ABC]
extends = [\"AB\"]
message = [\"C\"]

[profile.Highlight]
extends = [\"AB\"]
highlight = [\"A\"]

# CicleA extends CircleB and CircleB extends CircleA -> invalid
[profile.CircleA]
extends = [\"CircleB\"]

[profile.CircleB]
extends = [\"CircleA\"]";

fn run_rogcat_with_config_and_input_file(args: &SVec, payload: &SVec) -> Result<SVec, Error> {
    let lines = CONFIG.lines().map(|s| s.to_string()).collect();
    let config = tempfile_with_content(&lines)?.display().to_string();
    let mut a = svec!("-P", config);
    a.extend(args.clone());
    let output = run_rogcat_with_input_file(&a, payload)
        .expect("Failed to run rogcat with config and input file");
    assert!(output.0);
    Ok(output.1)
}

#[test]
fn cannot_find_config() {
    let file = tempfile().unwrap().display().to_string();
    let args = svec!("-C", file);
    let output = run_rogcat_with_input_file(&args, &vec![])
        .expect("Failed to run rogcat with config and input file");
    assert!(!output.0);
}

#[test]
fn malformed_config() {
    let config = "[";
    let config = tempfile_with_content(&svec!(config))
        .unwrap()
        .display()
        .to_string();
    let args = svec!("-C", config);
    let output = run_rogcat_with_input_file(&args, &vec![])
        .expect("Failed to run rogcat with config and input file");
    assert!(!output.0);
}

#[test]
fn filter_message_a() {
    let input = svec!("A", "B", "C");
    let output = run_rogcat_with_config_and_input_file(&svec!("-p", "A"), &input).unwrap();
    assert_eq!(output.len(), 1);
}

#[test]
fn filter_message_a_b() {
    let input = svec!("A", "B", "C");

    let output =
        run_rogcat_with_config_and_input_file(&svec!("-p", "A", "-m", "B"), &input).unwrap();
    assert_eq!(output.len(), 2);
}

#[test]
fn extends_message_a_b_c() {
    let input = svec!("A", "B", "C");

    let output = run_rogcat_with_config_and_input_file(&svec!("-p", "AB"), &input).unwrap();
    assert_eq!(output.len(), 2);

    let output =
        run_rogcat_with_config_and_input_file(&svec!("-p", "AB", "-m", "C"), &input).unwrap();
    assert_eq!(output.len(), 3);

    let output = run_rogcat_with_config_and_input_file(&svec!("-p", "ABC"), &input).unwrap();
    assert_eq!(output.len(), 3);
}

#[test]
fn extends_circle() {
    let lines = CONFIG.lines().map(|s| s.to_string()).collect();
    let config = tempfile_with_content(&lines).unwrap().display().to_string();
    // This is supposed to fail!
    let args = svec!("--profiles-path", config, "-p", "CircleA");
    let output = run_rogcat(&args, None).unwrap();
    assert!(!output.0);
}

#[test]
fn highlight() {
    let input = svec!("A", "B", "C");
    let output = run_rogcat_with_config_and_input_file(&svec!("-p", "Highlight"), &input).unwrap();
    assert_eq!(output.len(), 2);
}
