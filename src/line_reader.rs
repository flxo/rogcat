// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.


use errors::*;
use std::io::Read;
use std::io::{BufReader, BufRead};
use record::Record;

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
