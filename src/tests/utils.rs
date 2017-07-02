// Copyright © 2017 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use errors::*;
use rand::*;
use std::env;
use std::fs::File;
use std::fs;
use std::io::BufReader;
use std::io::prelude::*;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tempdir::TempDir;

macro_rules! svec {
    ( $( $x:expr ),* ) => {
        vec!($( $x.to_owned(), )*)
    };
}

type SVec = Vec<String>;

pub fn tempdir() -> Result<PathBuf> {
    TempDir::new("rogcat").map(|e| e.into_path()).map_err(
        |e| e.into(),
    )
}

pub fn tempfile() -> Result<PathBuf> {
    let mut path = tempdir()?;
    let filename: String = thread_rng().gen_iter::<char>().take(8).collect();
    path.push(filename);
    Ok(path)
}

pub fn tempfile_with_content(c: &SVec) -> Result<PathBuf> {
    let path = tempfile()?;
    File::create(path.clone())?.write_all(
        c.join("\n").as_bytes(),
    )?;
    Ok(path)
}

pub fn file_content(file: &PathBuf) -> Result<SVec> {
    let content = BufReader::new(File::open(file)?)
        .lines()
        .map(|e| e.unwrap())
        .collect();
    Ok(content)
}

pub fn check_file_content(file: &PathBuf, content: &SVec) -> Result<bool> {
    Ok(content == &file_content(file)?)
}

pub fn find_rogcat_binary() -> PathBuf {
    let exe = env::current_exe().unwrap();
    let this_dir = exe.parent().unwrap();
    let dirs = &[&this_dir, &this_dir.parent().unwrap()];
    dirs.iter()
        .map(|d| {
            d.join("rogcat").with_extension(env::consts::EXE_EXTENSION)
        })
        .filter_map(|d| fs::metadata(&d).ok().map(|_| d))
        .next()
        .expect(&format!(
            "Error: rogcat binary not found, looked in `{:?}`. Do you need to run `cargo build`?",
            dirs
        ))
}

pub fn run_rogcat(args: &SVec, input: Option<SVec>) -> Result<(bool, SVec)> {
    let rogcat = find_rogcat_binary();
    let mut process = Command::new(format!("{}", rogcat.display()))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run rogcat");

    {
        if let Some(input) = input {
            let stdin = process.stdin.as_mut().expect("failed to get stdin");
            let mut input = input.join("\n");
            if input.len() != 0 {
                input.push('\n');
            }
            stdin.write_all(input.as_bytes()).unwrap();
        }
    }

    let output = process.wait_with_output().expect("Failed to run rogcat");
    let stdout = String::from_utf8(output.stdout)
        .expect("Malformed stdout")
        .lines()
        .map(|s| s.to_string())
        .collect();
    Ok((output.status.success(), stdout))
}

pub fn run_rogcat_with_input_file(args: &SVec, payload: &SVec) -> Result<(bool, SVec)> {
    let input = tempfile_with_content(payload).expect("Failed to crate input file");
    let mut a = svec!("-i", format!("{}", input.display()));
    a.extend(args.clone());
    run_rogcat(&a, None)
}

#[test]
fn test_tempdir() {
    let dirs: Vec<PathBuf> = [..100].iter().map(|_| tempdir().unwrap()).collect();
    for d in dirs {
        assert!(d.exists());
    }
}

#[test]
fn test_tempfile_with_content() {
    let content = svec!("A", "B", "C");
    let tempfile = tempfile_with_content(&content).expect("Failed to create tempfile with content");
    let file = File::open(tempfile).expect("Failed to open tempfile");
    let reader: BufReader<File> = BufReader::new(file);
    assert_eq!(reader.lines().count(), content.len());
}

#[test]
fn test_file_content() {
    let content = svec!("A", "B", "C");
    let tempfile = tempfile_with_content(&content).expect("Failed to create tempfile with content");
    assert!(check_file_content(&tempfile, &content).unwrap());
}

#[test]
fn test_stdin_stdout() {
    let input = Some(vec![]);
    let output = run_rogcat(&svec!("-"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 0);

    let input = Some(svec!("A", "B", "C"));
    let output = run_rogcat(&svec!("-"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 3);

    let input = Some(svec!("A", "B", "C", "D"));
    let output = run_rogcat(&svec!("-"), input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 4);
}

#[test]
fn test_run_rogcat_with_input_file() {
    let input = svec!("A", "B", "C");
    let output = run_rogcat_with_input_file(&vec![], &input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 3);
}
