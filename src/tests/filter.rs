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

#[test]
fn filter_message() {
    let input = svec!("A", "B", "C", "D", "EF", "FE");
    let output = run_rogcat_with_input_file(svec!("-m", "A"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 1);

    let output = run_rogcat_with_input_file(svec!("-m", "A", "-m", "B"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 2);
}

#[test]
fn filter_message_opt_long() {
    let opt = "--message";
    let input = svec!("A", "B", "C", "D");
    let output = run_rogcat_with_input_file(svec!(opt, "A"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 1);

    let output = run_rogcat_with_input_file(svec!(opt, "A", opt, "B"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 2);
}

#[test]
fn filter_message_opt_short_long() {
    let long = "--message";
    let short = "-m";
    let input = svec!("A", "B", "C", "D");
    let output = run_rogcat_with_input_file(svec!(short, "A", long, "B"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 2);

    let output = run_rogcat_with_input_file(svec!(long, "A", short, "B"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 2);
}

#[test]
fn filter_regex() {
    let input = svec!("A", "B", "CF", "D", "EF", "FE", "monkey");
    let output = run_rogcat_with_input_file(svec!("-r", "^.*nk.*"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 1);

    let output = run_rogcat_with_input_file(svec!("-r", "^E.*"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 1);

    let input = svec!(
        "I/Runtime: Mindroid runtime system node id: 1",
        "I/Other: Mindroid runtime system node id: 1"
    );
    let output = run_rogcat_with_input_file(svec!("-r", "^Other$"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 1);
}

#[test]
fn filter_message_regex() {
    let input = svec!("A", "B", "CF", "D", "EF", "FE", "monkey");
    let output = run_rogcat_with_input_file(svec!("-m", "^.*nk.*"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 1);

    let output = run_rogcat_with_input_file(svec!("-m", "^E.*"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 1);

    // match CF, EF, FE
    let output = run_rogcat_with_input_file(svec!("-m", "^E.*", "-m", "^.*F"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 3);
}

#[test]
fn filter_message_ignorecase() {
    let input = svec!("A", "B", "C");
    let output = run_rogcat_with_input_file(svec!("-M", "a"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 1);
}

#[test]
fn filter_tag_ignorecase() {
    let input = svec!(
        "I/Runtime: Mindroid runtime system node id: 1",
        "I/Other: Mindroid runtime system node id: 1"
    );
    let output = run_rogcat_with_input_file(svec!("-T", "Runtime"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 1);
}
