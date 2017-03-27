// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use clap::ArgMatches;
use errors::*;
use futures::{future, Future};
use std::fs::File;
use std::path::PathBuf;
use super::Message;
use super::Node;
use super::RFuture;
use line_reader::{LineReader, ReadResult};

pub struct FileReader {
    files: Vec<LineReader<File>>,
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
            reader.push(LineReader::new(file));
        }

        Ok(FileReader { files: reader })
    }

    fn read(&mut self) -> Result<ReadResult> {
        let reader = &mut self.files[0];
        reader.read()
    }
}

impl Node for FileReader {
    type Input = ();

    fn process(&mut self, _: ()) -> RFuture {
        loop {
            match self.read() {
                Ok(v) => {
                    match v {
                        ReadResult::Done => {
                            if self.files.len() > 1 {
                                self.files.remove(0);
                            } else {
                                return future::ok(Message::Done).boxed();
                            }
                        }
                        ReadResult::Record(r) => return future::ok(Message::Record(r)).boxed(),
                    }
                }
                Err(e) => return future::err(e.into()).boxed(),
            }
        }
    }
}
