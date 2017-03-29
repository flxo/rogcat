// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use clap::ArgMatches;
use errors::*;
use futures::{future, Future};
use record::Record;
use std::fs::File;
use std::io::{BufReader, BufRead, Read};
use std::io::{Stdin, stdin};
use std::path::PathBuf;
use super::{Message, Node, RFuture};

pub enum ReadResult {
    Record(Record),
    Done,
}

pub struct LineReader<T>
    where T: Read
{
    reader: BufReader<T>,
}

impl<T: Read> LineReader<T> {
    pub fn new(reader: T) -> LineReader<T> {
        LineReader { reader: BufReader::new(reader) }
    }

    pub fn read(&mut self) -> Result<ReadResult> {
        let mut buffer = Vec::new();
        if self.reader
               .read_until(b'\n', &mut buffer)
               .chain_err(|| "Failed read")? > 0 {
            let line = String::from_utf8(buffer)?.trim().to_string();
            let record = Record { raw: line, ..Default::default() };
            Ok(ReadResult::Record(record))
        } else {
            Ok(ReadResult::Done)
        }
    }
}

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

pub struct StdinReader {
    reader: LineReader<Stdin>,
}

impl StdinReader {
    pub fn new() -> StdinReader {
        StdinReader { reader: LineReader::new(stdin()) }
    }
}

impl Node for StdinReader {
    type Input = ();
    fn process(&mut self, _: Self::Input) -> RFuture {
        match self.reader.read() {
                Ok(r) => {
                    match r {
                        ReadResult::Done => future::ok(Message::Done),
                        ReadResult::Record(r) => future::ok(Message::Record(r)),
                    }
                }
                Err(e) => future::err(e.into()),
            }
            .boxed()
    }
}
