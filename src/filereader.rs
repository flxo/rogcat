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
    filename: PathBuf,
}

impl Node<Record, PathBuf> for FileReader {
    fn new(file: PathBuf) -> Result<Box<Self>, String> {
        Ok(Box::new(FileReader { filename: file }))
    }

    fn start(&self, send: &Fn(Record), done: &Fn()) -> Result<(), String> {
        let file = File::open(self.filename.clone()).map_err(|e| format!("{}", e))?;
        let mut reader = BufReader::new(file);
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
                return Err(format!("Failed to read {:?}", self.filename));
            }
        }
        Ok(())
    }
}
