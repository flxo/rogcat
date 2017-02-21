// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use errors::*;
use std::fs::{DirBuilder, File};
use std::path::{Path, PathBuf};
use std::io::Write;
use super::node::Node;
use super::record::Record;

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

pub struct Args {
    pub filename: PathBuf,
    pub format: Format,
    pub records_per_file: Option<u64>,
}

pub struct FileWriter {
    filename: PathBuf,
    file: File,
    format: Format,
    records_per_file: Option<u64>,
    file_index: u32,
    file_size: u64,
}

impl FileWriter {
    fn next_file(filename: &PathBuf, file_index: Option<u32>) -> Result<PathBuf> {
        if file_index.is_none() {
            Ok(filename.clone())
        } else {
            if filename.as_path().is_dir() {
                return Err(format!("Output file {:?} is a directory", filename).into());
            }

            let dir = filename.parent().unwrap_or(Path::new(""));
            if !dir.is_dir() {
                DirBuilder::new()
                    .recursive(true)
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

    fn format(record: Record, format: &Format) -> Result<String> {
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
}

impl Node<Record, Args> for FileWriter {
    fn new(args: Args) -> Result<Box<Self>> {
        let file = Self::next_file(&args.filename, args.records_per_file.map(|_| 0))?;
        Ok(Box::new(FileWriter {
            file: File::create(file).map_err(|e| format!("{}", e))?,
            filename: args.filename,
            format: args.format,
            records_per_file: args.records_per_file,
            file_index: 0,
            file_size: 0,
        }))
    }

    fn message(&mut self, record: Record) -> Result<Option<Record>> {
        if let Some(records_per_file) = self.records_per_file {
            if self.file_size == records_per_file {
                self.file_index += 1;
                self.file = File::create(Self::next_file(&self.filename, Some(self.file_index))?)
                    .map_err(|e| format!("{}", e))?;
                self.file_size = 0;
            }
        }

        self.file
            .write(Self::format(record, &self.format)?.as_bytes())
            .map(|_| ())
            .map_err(|e| format!("{}", e))?;
        self.file_size += 1;

        Ok(None)
    }
}

#[test]
fn next_file() {
    use tempdir::TempDir;
    let tempdir = TempDir::new("rogcat").unwrap();
    let file = tempdir.path().join("test");
    assert_eq!(FileWriter::next_file(&file, None),
               Ok(file));
    assert_eq!(FileWriter::next_file(&PathBuf::from("tmp/test"), None),
               Ok(PathBuf::from("tmp/test")));
}

#[test]
fn next_file_index() {
    use tempdir::TempDir;
    let tempdir = TempDir::new("rogcat").unwrap();
    let file = tempdir.path().join("test");

    assert_eq!(FileWriter::next_file(&file, Some(0)),
               Ok(tempdir.path().join("test-000")));
    assert_eq!(FileWriter::next_file(&file, Some(1)),
               Ok(tempdir.path().join("test-001")));
    assert_eq!(FileWriter::next_file(&file, Some(2)),
               Ok(tempdir.path().join("test-002")));
    assert_eq!(FileWriter::next_file(&file, Some(1000)),
               Ok(tempdir.path().join("test-1000")));
}

#[test]
fn next_file_index_extension() {
    use tempdir::TempDir;
    let tempdir = TempDir::new("rogcat").unwrap();
    let file = tempdir.path().join("test.log");
    assert_eq!(FileWriter::next_file(&file, Some(0)),
               Ok(tempdir.path().join("test-000.log")));
    assert_eq!(FileWriter::next_file(&file, Some(1)),
               Ok(tempdir.path().join("test-001.log")));
}
