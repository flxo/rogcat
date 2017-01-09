// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use std::fs::File;
use std::path::PathBuf;
use std::io::Write;
use super::node::Node;
use super::record::Record;

enum Format {
    Raw,
    Csv,
}

pub struct FileWriter {
    format: Format,
    file: File,
}

impl Node<Record, (PathBuf, bool)> for FileWriter {
    fn new(args: (PathBuf, bool)) -> Result<Box<Self>, String> {
        Ok(Box::new(FileWriter {
            format: if args.1 { Format::Csv } else { Format::Raw },
            file: File::create(args.0).map_err(|e| format!("{}", e))?,
        }))
    }

    fn message(&mut self, record: Record) -> Result<Option<Record>, String> {
        let line = match self.format {
            Format::Csv => {
                let timestamp = if let Some(ts) = record.timestamp {
                    ::time::strftime("%m-%d %H:%M:%S.%f", &ts).unwrap_or("".to_owned())
                } else {
                    "".to_owned()
                };
                format!("\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
                        timestamp,
                        record.tag,
                        record.process,
                        record.thread,
                        record.level,
                        record.message)
            }
            Format::Raw => format!("{}\n", record.raw),
        };
        match self.file.write(&line.into_bytes()) {
            Ok(_) => (),
            Err(e) => panic!("{}", e),
        }
        Ok(None)
    }
}

#[test]
fn open() {
    use ::tempdir::TempDir;
    let tmp_dir = TempDir::new("filewriter").expect("create temp dir");
    let file = tmp_dir.path().join("my-temporary-note.txt");
    let filewriter = FileWriter::new((PathBuf::from(file), false));
    assert!(filewriter.is_ok());
}
