// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use std::fs::File;
use std::io::{BufReader, BufRead};
use std::path::PathBuf;
use super::node::Node;
use super::record::Record;

pub struct FileReader {
    files: Vec<File>,
}

impl Node<Record, Vec<PathBuf>> for FileReader {
    fn new(files: Vec<PathBuf>) -> Result<Box<Self>, String> {
        let mut f = vec!();
        for filename in &files {
            f.push(File::open(filename).map_err(|e| format!("Cannot open {:?}: {}", filename, e))?);
        }
        Ok(Box::new(FileReader { files: f }))
    }

    fn start(&mut self, send: &Fn(Record), done: &Fn()) -> Result<(), String> {
        for f in self.files.drain(..) {
            let mut reader = BufReader::new(f);
            loop {
                let mut buffer: Vec<u8> = Vec::new();
                if let Ok(len) = reader.read_until(b'\n', &mut buffer) {
                    if len == 0 {
                        done();
                        break;
                    } else {
                        send(Record {
                            raw: String::from_utf8_lossy(&buffer).trim().to_string(),
                            ..Default::default()
                        });
                    }
                } else {
                    return Err("Failed to read input file".to_owned());
                }
            }
        }
        Ok(())
    }
}
