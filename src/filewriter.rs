// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use clap::ArgMatches;
use errors::*;
use futures::{future, Future};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use std::fs::{DirBuilder, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use super::{Format, Message, Node, RFuture};
use super::record::Record;

pub struct FileWriter {
    file: File,
    file_size: u64,
    filename: PathBuf,
    format: Format,
    records_per_file: Option<u64>,
    overwrite: bool,
    path: Option<PathBuf>,
    process: Option<ProgressBar>,
}

impl<'a> FileWriter {
    pub fn new(args: &ArgMatches<'a>) -> Result<Self> {
        let filename = args.value_of("output")
            .and_then(|f| Some(PathBuf::from(f)))
            .ok_or("Invalid output filename!")?;

        let records_per_file = args.value_of("RECORDS_PER_FILE").and_then(|l| {
            Regex::new(r"^(\d+)([kMG])$")
                .unwrap()
                .captures(l)
                .and_then(|caps| {
                              caps.at(1)
                                  .and_then(|size| u64::from_str(size).ok())
                                  .and_then(|size| Some((size, caps.at(2))))
                          })
                .and_then(|(size, suffix)| match suffix {
                              Some("k") => Some(1000 * size),
                              Some("M") => Some(1000_000 * size),
                              Some("G") => Some(1000_000_000 * size),
                              _ => None,
                          })
                .or_else(|| {
                    u64::from_str(l).ok()
                })
        });

        let format = match args.value_of("FILE_FORMAT") {
            Some(s) => Format::from_str(s)?,
            None => Format::Raw,
        };
        let overwrite = args.is_present("OVERWRITE");

        let file = Self::next_file(&filename, records_per_file.is_some(), overwrite)?;

        let progress = if !args.is_present("VERBOSE") {
            let (pb, chars, template) = if let Some(n) = records_per_file {
                (ProgressBar::new(n), "•• ", "{spinner:.yellow} Writing {msg:.dim.bold} {pos:>7.dim}/{len:.dim} {elapsed_precise:.dim} [{bar:40.yellow/green}] ({eta:.dim})")
            } else {
                (ProgressBar::new(::std::u64::MAX), " • ", "{spinner:.yellow} Writing {msg:.dim.bold} {pos:>7.dim} {elapsed_precise:.dim}")
            };
            pb.set_style(ProgressStyle::default_bar().template(template).progress_chars(chars));
            pb.set_message(file.to_str().ok_or("Failed to render filename")?);
            Some(pb)
        } else {
            None
        };

        Ok(FileWriter {
               file:
                   File::create(file.clone()).chain_err(|| {
                                      format!("Failed to create output file: {:?}", file.clone())
                                  })?,
               file_size: 0,
               filename: filename,
               format: format,
               records_per_file: records_per_file,
               overwrite: overwrite,
               path: None,
               process: progress,
           })
    }

    fn next_file(filename: &PathBuf, indexed: bool, overwrite: bool) -> Result<PathBuf> {
        if !indexed {
            if filename.exists() && !overwrite {
                Err(format!("{:?} exists. Use overwrite flag to force!", filename).into())
            } else {
                Ok(filename.clone())
            }
        } else {
            if filename.as_path().is_dir() {
                return Err(format!("Output file {:?} is a directory", filename).into());
            }

            let dir = filename.parent().unwrap_or_else(|| Path::new(""));
            if !dir.is_dir() {
                DirBuilder::new().recursive(true)
                    .create(dir)
                    .chain_err(|| "Failed to create directory")?
            }

            let next = |index| -> Result<PathBuf> {
                let mut name = filename.clone();
                name = PathBuf::from(format!("{}-{:03}",
                                             name.file_stem()
                                                 .ok_or("Invalid path")?
                                                 .to_str()
                                                 .ok_or("Invalid path")?,
                                             index));
                if let Some(extension) = filename.extension() {
                    name.set_extension(extension);
                }
                Ok(dir.join(name))
            };

            for index in 0.. {
                let n = next(index)?;
                if !n.exists() {
                    return Ok(n);
                }
            }

            Err("Unable find next file index".into())
        }
    }

    fn write(&mut self, record: &Record) -> Result<usize> {
        if let Some(records_per_file) = self.records_per_file {
            if self.file_size == records_per_file {
                let file = Self::next_file(&self.filename, true, self.overwrite)?;
                self.file = File::create(&file).chain_err(|| "Failed to create output file")?;
                if let Some(ref pb) = self.process {
                    pb.set_message(file.to_str().ok_or("Failed to render file name")?);
                }
                self.path = Some(file);
                self.file_size = 0;
            }
        }

        let error_msg = "Failed to write to output file";
        self.file_size += 1;

        if let Some(ref pb) = self.process {
            pb.set_position(self.file_size);
        }

        self.file
            .write(record.format(self.format.clone())?.as_bytes())
            .chain_err(|| error_msg)?;
        self.file.write(b"\n").chain_err(|| error_msg)
    }
}

impl Node for FileWriter {
    type Input = Message;

    fn process(&mut self, message: Message) -> RFuture {
        if let Message::Record(ref record) = message {
            if let Err(e) = self.write(record) {
                return future::err(e.into()).boxed();
            }
        }
        future::ok(message).boxed()
    }
}

#[test]
fn next_file() {
    use tempdir::TempDir;
    let tempdir = TempDir::new("rogcat").unwrap();
    let file = tempdir.path().join("test");
    assert_eq!(FileWriter::next_file(&file, false, false).unwrap(), file);
    assert_eq!(FileWriter::next_file(&PathBuf::from("tmp/test"), false, false).unwrap(),
               PathBuf::from("tmp/test"));
}

#[test]
fn next_file_index() {
    use tempdir::TempDir;
    let tempdir = TempDir::new("rogcat").unwrap();
    let file = tempdir.path().join("test");

    assert_eq!(FileWriter::next_file(&file, true, false).unwrap(), tempdir.path().join("test-000"));
    File::create(tempdir.path().join("test-000")).unwrap();
    assert_eq!(FileWriter::next_file(&file, true, false).unwrap(), tempdir.path().join("test-001"));
    File::create(tempdir.path().join("test-001")).unwrap();
    assert_eq!(FileWriter::next_file(&file, true, false).unwrap(), tempdir.path().join("test-002"));
    File::create(tempdir.path().join("test-002")).unwrap();
}

#[test]
fn next_file_index_extension() {
    use tempdir::TempDir;
    let tempdir = TempDir::new("rogcat").unwrap();
    let file = tempdir.path().join("test.log");
    assert_eq!(FileWriter::next_file(&file, true, false).unwrap(),
               tempdir.path().join("test-000.log"));
    File::create(tempdir.path().join("test-000.log")).unwrap();
    assert_eq!(FileWriter::next_file(&file, true, false).unwrap(),
               tempdir.path().join("test-001.log"));
}
