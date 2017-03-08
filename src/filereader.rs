// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use clap::ArgMatches;
use errors::*;
use futures::{future, Future};
use kabuki::Actor;
use std::fs::File;
use std::io::{BufReader, BufRead};
use std::path::PathBuf;
use super::Message;
use super::record::Record;
use super::RFuture;

enum ReadResult {
    Empty,
    Line(String),
}

pub struct FileReader {
    files: Vec<Box<BufReader<File>>>,
}

impl<'a> FileReader {
    pub fn new(args: &ArgMatches<'a>) -> Result<Self> {
        let files = args.values_of("input")
            .map(|f| f.map(PathBuf::from).collect::<Vec<PathBuf>>())
            .ok_or("Failed to parse input files")?;

        // No early return from iteration....
        let mut reader = Vec::new();
        for f in files {
            let file = File::open(f.clone()).chain_err(|| format!("Failed to open {:?}", f))?;
            let bufreader = BufReader::new(file);
            reader.push(Box::new(bufreader));
        }

        Ok(FileReader { files: reader })
    }

    fn read(&mut self) -> Result<ReadResult> {
        let reader = &mut self.files[0];
        let mut buffer = Vec::new();
        if reader.read_until(b'\n', &mut buffer).chain_err(|| "Failed read")? > 0 {
            let line = String::from_utf8_lossy(&buffer).trim().to_string();
            Ok(ReadResult::Line(line))
        } else {
            Ok(ReadResult::Empty)
        }
    }
}

impl Actor for FileReader {
    type Request = ();
    type Response = Message;
    type Error = Error;
    type Future = RFuture<Message>;

    fn call(&mut self, _: ()) -> Self::Future {
        loop {
            match self.read() {
                Ok(v) => {
                    match v {
                        ReadResult::Empty => {
                            if self.files.len() > 1 {
                                self.files.remove(0);
                            } else {
                                return future::ok(Message::Done).boxed();
                            }
                        }
                        ReadResult::Line(line) => {
                            let record = Record { raw: line, ..Default::default() };
                            return future::ok(Message::Record(record)).boxed();
                        }
                    }
                }
                Err(e) => return future::err(e.into()).boxed(),
            }
        }
    }
}
