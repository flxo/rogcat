// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use std::fs::{DirBuilder, File};
use std::path::{Path, PathBuf};
use std::io::Write;
use super::node::Node;
use super::record::Record;
use std::collections::VecDeque;

pub enum Format {
    Raw,
    Csv,
}

pub struct Args {
    pub filename: PathBuf,
    pub format: Format,
    pub lines_per_file: Option<usize>,
}

pub struct FileWriter {
    filename: PathBuf,
    file: File,
    format: Format,
    lines_per_file: Option<usize>,
    buffer: VecDeque<Record>,
    index: u32,
}

impl FileWriter {
    fn next_file(filename: &PathBuf, index: Option<u32>) -> Result<File, String> {
        if filename.as_path().is_dir() {
            return Err(format!("Output file {:?} is a directory", filename));
        }

        let dir = filename.parent().unwrap_or(Path::new(""));

        if !dir.is_dir() {
            DirBuilder::new().recursive(true)
                .create(dir)
                .map_err(|e| format!("{}", e))?
        }

        let filename = filename.file_name()
            .ok_or(format!("Invalid path"))?
            .to_str()
            .ok_or(format!("Invalid path"))?;
        let filename = if let Some(index) = index {
            // TODO: Strip and append files suffix
            format!("{}-{:03}", filename, index)
        } else {
            filename.to_owned()
        };

        File::create(dir.join(PathBuf::from(filename))).map_err(|e| format!("{}", e))
    }

    fn format(record: Record, format: &Format) -> Result<String, String> {
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

    fn store(&mut self) -> Result<(), String> {
        if !self.buffer.is_empty() {
            for record in self.buffer.drain(..) {
                let line = Self::format(record, &self.format)?;
                self.file.write(line.as_bytes()).map_err(|e| format!("{}", e))?;
            }
            self.index += 1;
        }

        Ok(())
    }
}

impl Node<Record, Args> for FileWriter {
    fn new(args: Args) -> Result<Box<Self>, String> {
        Ok(Box::new(FileWriter {
            file: Self::next_file(&args.filename, args.lines_per_file.map(|_| 0))?,
            filename: args.filename,
            format: args.format,
            lines_per_file: args.lines_per_file,
            buffer: VecDeque::new(),
            index: 0,
        }))
    }

    fn stop(&mut self) -> Result<(), String> {
        self.store()
    }

    fn message(&mut self, record: Record) -> Result<Option<Record>, String> {
        if let Some(lines_per_file) = self.lines_per_file {
            self.buffer.push_back(record);
            if self.buffer.len() >= lines_per_file {
                self.file = Self::next_file(&self.filename, Some(self.index))?;
                self.store()?
            }
        } else {
            self.file
                .write(Self::format(record, &self.format)?.as_bytes())
                .map(|_| ())
                .map_err(|e| format!("{}", e))?
        }
        Ok(None)
    }
}
