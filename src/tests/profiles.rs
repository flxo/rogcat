// Copyright © 2017 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use errors::*;
use tests::utils::*;

const CONFIG: &str = "[profile.A]\n
                      message = [\"^A$\"]\n
                      \n
                      [profile.AandB]\n
                      message = [\"A\", \"B\"]
                      \n
                      [profile.Highlight]\n
                      highlight = [\"A\", \"B\"]";

fn run_rogcat_with_config_and_input_file(args: &SVec, payload: &SVec) -> Result<SVec> {
    let lines = CONFIG.lines().map(|s| s.to_string()).collect();
    let config = tempfile_with_content(&lines)?.display().to_string();
    let mut a = svec!("-C", config);
    a.extend(args.clone());
    let output = run_rogcat_with_input_file(&a, payload).expect("Failed to run rogcat with config and input file");
    assert!(output.0);
    Ok(output.1)
}

#[test]
fn cannot_find_config() {
    let file = tempfile().unwrap().display().to_string();
    let args = svec!("-C", file);
    let output = run_rogcat_with_input_file(&args, &vec!()).expect("Failed to run rogcat with config and input file");
    assert!(!output.0);
}

#[test]
fn malformed_config() {
    let config = "[";
    let config = tempfile_with_content(&svec!(config)).unwrap().display().to_string();
    let args = svec!("-C", config);
    let output = run_rogcat_with_input_file(&args, &vec!()).expect("Failed to run rogcat with config and input file");
    assert!(!output.0);
}

#[test]
fn list_profiles() {
    let output = run_rogcat(&svec!("profiles", "--list"), None).unwrap();
    assert!(output.0);
    assert!(output.1.len() >= 1); // check for >1 if default location settings are found

    let output = run_rogcat(&svec!("profiles", "-l"), None).unwrap();
    assert!(output.1.len() >= 1); // check for >1 if default location settings are found

    let output = run_rogcat_with_config_and_input_file(&svec!("profiles", "-l"), &vec!()).unwrap();
    assert_eq!(output.len(), 4);

    let output = run_rogcat_with_config_and_input_file(&svec!("profiles", "--list"), &vec!()).unwrap();
    assert_eq!(output.len(), 4);
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

    let output = run_rogcat_with_config_and_input_file(&svec!("-p", "A", "-m", "B"), &input).unwrap();
    assert_eq!(output.len(), 2);

    let output = run_rogcat_with_config_and_input_file(&svec!("-p", "AandB"), &input).unwrap();
    assert_eq!(output.len(), 2);

    let output = run_rogcat_with_config_and_input_file(&svec!("-p", "AandB", "-m", "C"), &input).unwrap();
    assert_eq!(output.len(), 3);
}

#[test]
fn highlight() {
    let input = svec!("A", "B", "C");
    let output = run_rogcat_with_config_and_input_file(&svec!("-p", "Highlight"), &input).unwrap();
    assert_eq!(output.len(), 3);
}
