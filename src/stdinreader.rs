// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use std::io::stdin;
use super::node::Handler;
use super::record::{Level, Record};
use super::Args;

pub struct StdinReader;

impl Handler<Record> for StdinReader {
    fn new(_args: Args) -> Box<Self> {
        Box::new(StdinReader {})
    }

    fn start(&self, send: &Fn(Record), done: &Fn()) {
        loop {
            let mut buffer = String::new();
            if let Ok(len) = stdin().read_line(&mut buffer) {
                if len == 0 {
                    done();
                    break;
                } else {
                    send(Record {
                        timestamp: ::time::now(),
                        level: Level::default(),
                        tag: String::default(),
                        process: String::default(),
                        thread: String::default(),
                        message: String::default(),
                        raw: buffer.trim().to_string(),
                    });
                }
            } else {
                panic!("Failed to read"); // TODO: handle this nicely
            }
        }
    }
}
