// Copyright © 2017 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use tests::utils::*;

#[test]
fn test_filter_message() {
    let input = svec!("A", "B", "C", "D", "EF", "FE");
    let output = run_rogcat_with_input_file(&svec!("-m", "A"), &input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 1);

    let output = run_rogcat_with_input_file(&svec!("-m", "A", "-m", "B"), &input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 2);
}

#[test]
fn test_filter_message_opt_long() {
    let opt = "--message";
    let input = svec!("A", "B", "C", "D");
    let output = run_rogcat_with_input_file(&svec!(opt, "A"), &input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 1);

    let output = run_rogcat_with_input_file(&svec!(opt, "A", opt, "B"), &input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 2);
}

#[test]
fn test_filter_message_opt_short_long() {
    let long = "--message";
    let short = "-m";
    let input = svec!("A", "B", "C", "D");
    let output = run_rogcat_with_input_file(&svec!(short, "A", long, "B"), &input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 2);

    let output = run_rogcat_with_input_file(&svec!(long, "A", short, "B"), &input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 2);
}

#[test]
fn test_filter_message_regex() {
    let input = svec!("A", "B", "CF", "D", "EF", "FE", "monkey");
    let output = run_rogcat_with_input_file(&svec!("-m", "^.*nk.*"), &input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 1);

    let output = run_rogcat_with_input_file(&svec!("-m", "^E.*"), &input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 1);

    // match CF, EF, FE
    let output = run_rogcat_with_input_file(&svec!("-m", "^E.*", "-m", "^.*F"), &input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 3);
}
