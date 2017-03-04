// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use clap::ArgMatches;
use errors::*;
use futures::{future, Future};
use kabuki::Actor;
use regex::Regex;
use std::fs::{DirBuilder, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use super::Message;
use super::record::Record;
use super::RFuture;

pub enum Format {
    Raw,
    Csv,
}

impl ::std::str::FromStr for Format {
    type Err = &'static str;
    fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
        match s {
            "csv" => Ok(Format::Csv),
            "raw" => Ok(Format::Raw),
            _ => Err("Format parsing error"),
        }
    }
}

pub struct FileWriter {
    filename: PathBuf,
    file: File,
    format: Format,
    records_per_file: Option<u64>,
    file_index: u32,
    file_size: u64,
}

impl<'a> FileWriter {
    pub fn new(args: &ArgMatches<'a>) -> Result<Self> {
        let filename = args.value_of("output")
            .and_then(|f| Some(PathBuf::from(f)))
            .ok_or("Invalid output filename!")?;

        let records_per_file = args.value_of("records-per-file")
            .and_then(|l| {
                Regex::new(r"^(\d+)([kMG])$")
                    .unwrap()
                    .captures(l)
                    .and_then(|caps| {
                        caps.at(1)
                            .and_then(|size| u64::from_str(size).ok())
                            .and_then(|size| Some((size, caps.at(2))))
                    })
                    .and_then(|(size, suffix)| {
                        match suffix {
                            Some("k") => Some(1000 * size),
                            Some("M") => Some(1000_000 * size),
                            Some("G") => Some(1000_000_000 * size),
                            _ => None,
                        }
                    })
            });

        let format = match args.value_of("format") {
            Some(s) => Format::from_str(s)?,
            None => Format::Raw,
        };

        let file = Self::next_file(&filename, records_per_file.map(|_| 0))?;

        Ok(FileWriter {
            file: File::create(file.clone()).chain_err(|| format!("Failed to create output file: {:?}", file.clone()))?,
            filename: filename,
            format: format,
            records_per_file: records_per_file,
            file_index: 0,
            file_size: 0,
        })
    }

    fn next_file(filename: &PathBuf, file_index: Option<u32>) -> Result<PathBuf> {
        if file_index.is_none() {
            Ok(filename.clone())
        } else {
            if filename.as_path().is_dir() {
                return Err(format!("Output file {:?} is a directory", filename).into());
            }

            let dir = filename.parent().unwrap_or(Path::new(""));
            if !dir.is_dir() {
                DirBuilder::new().recursive(true)
                    .create(dir)
                    .chain_err(|| "Failed to create directory")?
            }

            let mut name = filename.clone();
            name = PathBuf::from(format!("{}-{:03}",
                                         name.file_stem()
                                             .ok_or(format!("Invalid path"))?
                                             .to_str()
                                             .ok_or(format!("Invalid path"))?,
                                         file_index.unwrap()));
            if let Some(extension) = filename.extension() {
                name.set_extension(extension);
            }

            Ok(dir.join(name))
        }
    }

    fn format(record: &Record, format: &Format) -> Result<String> {
        Ok(match format {
            &Format::Csv => {
                format!("\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
                        record.timestamp
                            .and_then(|ts| ::time::strftime("%m-%d %H:%M:%S.%f", &ts).ok())
                            .unwrap_or("".to_owned()),
                        record.tag,
                        record.process,
                        record.thread,
                        record.level,
                        record.message)
            }
            &Format::Raw => format!("{}\n", record.raw),
        })
    }

    fn write(&mut self, record: &Record) -> Result<usize> {
        if let Some(records_per_file) = self.records_per_file {
            if self.file_size == records_per_file {
                self.file_index += 1;
                let filename = Self::next_file(&self.filename, Some(self.file_index))?;
                self.file = File::create(filename).chain_err(|| "Failed to create output file")?;
                self.file_size = 0;
            }
        }

        self.file_size += 1;
        self.file
            .write(Self::format(record, &self.format)?.as_bytes())
            .chain_err(|| "Failed to write to output file")
    }
}

impl Actor for FileWriter {
    type Request = Message;
    type Response = Message;
    type Error = Error;
    type Future = RFuture<Message>;

    fn call(&mut self, message: Message) -> Self::Future {
        match message {
            Message::Record(ref record) => {
                if let Err(e) = self.write(record) {
                    return future::err(e.into()).boxed();
                }
            }
            _ => (),
        }
        future::ok(message).boxed()
    }
}

#[test]
fn next_file() {
    use tempdir::TempDir;
    let tempdir = TempDir::new("rogcat").unwrap();
    let file = tempdir.path().join("test");
    assert_eq!(FileWriter::next_file(&file, None).unwrap(), file);
    assert_eq!(FileWriter::next_file(&PathBuf::from("tmp/test"), None).unwrap(),
               PathBuf::from("tmp/test"));
}

#[test]
fn next_file_index() {
    use tempdir::TempDir;
    let tempdir = TempDir::new("rogcat").unwrap();
    let file = tempdir.path().join("test");

    assert_eq!(FileWriter::next_file(&file, Some(0)).unwrap(),
               tempdir.path().join("test-000"));
    assert_eq!(FileWriter::next_file(&file, Some(1)).unwrap(),
               tempdir.path().join("test-001"));
    assert_eq!(FileWriter::next_file(&file, Some(2)).unwrap(),
               tempdir.path().join("test-002"));
    assert_eq!(FileWriter::next_file(&file, Some(1000)).unwrap(),
               tempdir.path().join("test-1000"));
}

#[test]
fn next_file_index_extension() {
    use tempdir::TempDir;
    let tempdir = TempDir::new("rogcat").unwrap();
    let file = tempdir.path().join("test.log");
    assert_eq!(FileWriter::next_file(&file, Some(0)).unwrap(),
               tempdir.path().join("test-000.log"));
    assert_eq!(FileWriter::next_file(&file, Some(1)).unwrap(),
               tempdir.path().join("test-001.log"));
}
