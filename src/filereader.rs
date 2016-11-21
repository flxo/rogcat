// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use std::io::{BufReader, BufRead};
use super::node::Handler;
use super::record::{Level, Record};
use super::Args;

pub struct FileReader {
    file: String,
}

impl Handler<Record> for FileReader {
    fn new(args: Args) -> Box<Self> {
        Box::new(FileReader { file: args.input.unwrap() })
    }

    fn start(&self, send: &Fn(Record), done: &Fn()) {
        let file = ::std::fs::File::open(&self.file).expect("Failed to open file");
        let mut reader = BufReader::new(file);

        loop {
            let mut buffer: Vec<u8> = Vec::new();
            if let Ok(len) = reader.read_until(b'\n', &mut buffer) {
                if len == 0 {
                    done();
                    break;
                } else {
                    send(Record {
                        timestamp: ::time::now(),
                        level: Level::Debug,
                        tag: "".to_string(),
                        process: "".to_string(),
                        thread: "".to_string(),
                        message: String::from_utf8_lossy(&buffer).trim().to_string(),
                    });
                }
            } else {
                panic!("Failed to read"); // TODO: handle this nicely
            }
        }
    }
}
