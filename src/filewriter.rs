// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use boolinator::Boolinator;
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
use time::{now, strftime};

pub struct FileWriter {
    file: Option<File>,
    file_size: u64,
    filename: PathBuf,
    filename_format: FilenameFormat,
    format: Format,
    process: Option<ProgressBar>,
}

#[derive(Clone)]
enum FilenameFormat {
    Single(bool),
    Enumerate(bool, u64),
    Date(bool, u64),
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
                .or_else(|| u64::from_str(l).ok())
        });

        let format = match args.value_of("FILE_FORMAT") {
            Some(s) => Format::from_str(s)?,
            None => Format::Raw,
        };

        let overwrite = args.is_present("OVERWRITE");

        let records = records_per_file.unwrap_or(::std::u64::MAX);
        let filename_format = match args.value_of("FILENAME_FORMAT") {
            Some("enumerate") => FilenameFormat::Enumerate(overwrite, records),
            Some("date") => FilenameFormat::Date(overwrite, records),
            // If records per file is set, default to enumerated even if
            // no file format argument is supplied.
            Some(_) | None => {
                if let Some(n) = records_per_file {
                    FilenameFormat::Enumerate(overwrite, n)
                } else {
                    FilenameFormat::Single(overwrite)
                }
            }
        };

        let progress = if !args.is_present("VERBOSE") {
            let (pb, chars, template) = if let Some(n) = records_per_file {
                (ProgressBar::new(n),
                 "•• ",
                 "{spinner:.yellow} Writing {msg:.dim.bold} {pos:>7.dim}/{len:.dim} {elapsed_precise:.dim} [{bar:40.yellow/green}] ({eta:.dim})")
            } else {
                (ProgressBar::new(::std::u64::MAX),
                 " • ",
                 "{spinner:.yellow} Writing {msg:.dim.bold} {pos:>7.dim} {elapsed_precise:.dim}")
            };
            pb.set_style(ProgressStyle::default_bar().template(template).progress_chars(chars));
            Some(pb)
        } else {
            None
        };

        Ok(FileWriter {
               file: None,
               file_size: 0,
               filename: filename,
               format: format,
               filename_format: filename_format,
               process: progress,
           })
    }

    fn next_file(&self) -> Result<PathBuf> {
        match self.filename_format {
            FilenameFormat::Single(overwrite) => {
                if self.filename.exists() && !overwrite {
                    Err(format!("{:?} exists. Use overwrite flag to force!", self.filename).into())
                } else {
                    Ok(self.filename.clone())
                }
            }
            FilenameFormat::Enumerate(_overwrite, _) => {
                if self.filename.as_path().is_dir() {
                    return Err(format!("Output file {:?} is a directory", self.filename).into());
                }

                let dir = self.filename.parent().unwrap_or(Path::new(""));
                if !dir.is_dir() {
                    DirBuilder::new().recursive(true)
                        .create(dir)
                        .chain_err(|| "Failed to create outfile parent directory")?
                }

                let next = |index| -> Result<PathBuf> {
                    let mut name = self.filename.clone();
                    name = PathBuf::from(format!("{}-{:03}",
                                                 name.file_stem()
                                                     .ok_or("Invalid path")?
                                                     .to_str()
                                                     .ok_or("Invalid path")?,
                                                 index));
                    if let Some(extension) = self.filename.extension() {
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

                unreachable!("Could not find a file - this is proably a bug here...")
            }
            FilenameFormat::Date(overwrite, _) => {
                // If the overwrite flag is set the files are
                // enumerated from the first one to get nice
                // aligned filenames.
                let mut e: Option<u32> = (!overwrite).as_some(0);

                loop {
                    let dir = self.filename.parent().unwrap_or(Path::new(""));
                    if !dir.is_dir() {
                        DirBuilder::new().recursive(true)
                            .create(dir)
                            .chain_err(|| {
                                           format!("Failed to create outfile parent directory: {}",
                                                   dir.display())
                                       })?
                    }

                    let now = strftime("%F-%T", &now())?;
                    let enumeration = e.map(|a| format!("-{:03}", a))
                        .unwrap_or("".to_owned());
                    let filename = self.filename
                        .file_name()
                        .ok_or("Invalid path")?
                        .to_str()
                        .ok_or("Invalid path")?;
                    let candidate = PathBuf::from(format!("{}{}_{}", now, enumeration, filename));
                    let candidate = dir.join(candidate);
                    if !overwrite && candidate.exists() {
                        e = Some(e.unwrap_or(0) + 1);
                        continue;
                    } else {
                        return Ok(candidate);
                    }
                }
            }
        }
    }

    fn write(&mut self, record: &Record) -> Result<usize> {
        let error_msg = "Failed to write to output file";
        let r = match self.file {
            Some(ref mut file) => {
                file.write(record.format(self.format.clone())?.as_bytes()).chain_err(|| error_msg)?;
                file.write(b"\n").chain_err(|| error_msg)
            }
            None => {
                let next_file = self.next_file()?;
                let mut file = File::create(next_file.clone()).chain_err(|| {
                                   format!("Failed to create output file: {:?}",
                                           next_file.display())
                               })?;
                if let Some(ref pb) = self.process {
                    pb.set_message(next_file.to_str().ok_or("Failed to render file name")?);
                }
                file.write(record.format(self.format.clone())?.as_bytes()).chain_err(|| error_msg)?;
                let r = file.write(b"\n").chain_err(|| error_msg);
                self.file = Some(file);
                r
            }
        };
        self.file_size += 1;

        if let Some(ref pb) = self.process {
            pb.set_position(self.file_size);
        }

        match self.filename_format {
            FilenameFormat::Enumerate(_, n) |
            FilenameFormat::Date(_, n) => {
                if self.file_size >= n {
                    self.file_size = 0;
                    self.file = None
                }
            }
            _ => (),
        }

        r
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
