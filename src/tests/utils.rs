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

use failure::{format_err, Error};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use std::{
    env,
    fs::{self, File},
    io::{prelude::*, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
};
use tempdir::TempDir;

macro_rules! svec {
    ( $( $x:expr ),* ) => {
        &[$( $x.to_owned(), )*]
    };
}

pub type SVec = Vec<String>;

pub fn tempdir() -> Result<PathBuf, Error> {
    TempDir::new("rogcat")
        .map(tempdir::TempDir::into_path)
        .map_err(|e| format_err!("Tempdir error: {}", e))
}

pub fn tempfile() -> Result<PathBuf, Error> {
    let mut path = tempdir()?;
    let filename: String = thread_rng().sample_iter(&Alphanumeric).take(8).collect();
    path.push(filename);
    Ok(path)
}

pub fn tempfile_with_content(c: &[String]) -> Result<PathBuf, Error> {
    let path = tempfile()?;
    File::create(path.clone())?.write_all(c.join("\n").as_bytes())?;
    Ok(path)
}

pub fn file_content(file: &PathBuf) -> Result<SVec, Error> {
    let content = BufReader::new(File::open(file)?)
        .lines()
        .map(Result::unwrap)
        .collect();
    Ok(content)
}

pub fn check_file_content(file: &PathBuf, content: &[String]) -> Result<bool, Error> {
    Ok(content == file_content(file)?.as_slice())
}

pub fn find_rogcat_binary() -> PathBuf {
    let exe = env::current_exe().unwrap();
    let this_dir = exe.parent().unwrap();
    let dirs = &[&this_dir, &this_dir.parent().unwrap()];
    dirs.iter()
        .map(|d| d.join("rogcat").with_extension(env::consts::EXE_EXTENSION))
        .filter_map(|d| fs::metadata(&d).ok().map(|_| d))
        .next()
        .unwrap_or_else(|| panic!("Error: rogcat binary not found, looked in `{:?}`. Do you need to run `cargo build`?", dirs))
}

pub fn run_rogcat(args: &[String], input: Option<&[String]>) -> Result<(bool, SVec), Error> {
    let rogcat = find_rogcat_binary();
    let mut process = Command::new(format!("{}", rogcat.display()))
        .args(args)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run rogcat");

    {
        if let Some(input) = input {
            let stdin = process.stdin.as_mut().expect("failed to get stdin");
            let mut input = input.join("\n");
            if !input.is_empty() {
                input.push('\n');
            }
            stdin.write_all(input.as_bytes()).unwrap();
        }
    }

    let output = process.wait_with_output().expect("Failed to run rogcat");
    let stdout = String::from_utf8(output.stdout)
        .expect("Malformed stdout")
        .lines()
        .map(std::string::ToString::to_string)
        .collect();
    Ok((output.status.success(), stdout))
}

pub fn run_rogcat_with_input_file(
    args: &[String],
    payload: &[String],
) -> Result<(bool, SVec), Error> {
    let input = tempfile_with_content(payload).expect("Failed to crate input file");
    let mut a = svec!("-i", format!("{}", input.display())).to_vec();
    a.extend(args.iter().cloned());
    run_rogcat(&a, None)
}

#[test]
fn tempdirs() {
    let dirs: Vec<PathBuf> = [..100].iter().map(|_| tempdir().unwrap()).collect();
    for d in dirs {
        assert!(d.exists());
    }
}

#[test]
fn create_tempfile_with_content() {
    let content = svec!("A", "B", "C");
    let tempfile = tempfile_with_content(content).expect("Failed to create tempfile with content");
    let file = File::open(tempfile).expect("Failed to open tempfile");
    let reader: BufReader<File> = BufReader::new(file);
    assert_eq!(reader.lines().count(), content.len());
}

#[test]
fn compare_file_content() {
    let content = svec!("A", "B", "C");
    let tempfile = tempfile_with_content(content).expect("Failed to create tempfile with content");
    assert!(check_file_content(&tempfile, content).unwrap());
}

#[test]
fn stdin_stdout() {
    let output = run_rogcat(svec!("-"), Some(&[])).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 0);

    let output = run_rogcat(svec!("-"), Some(svec!("A", "B", "C"))).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 3);

    let output = run_rogcat(svec!("-"), Some(svec!("A", "B", "C", "D"))).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 4);
}

#[test]
fn testrun_rogcat_with_input_file() {
    let input = svec!("A", "B", "C");
    let output = run_rogcat_with_input_file(&[], input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 3);
}
