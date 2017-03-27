// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use futures::{future, Future};
use line_reader::{LineReader, ReadResult};
use std::io::{Stdin, stdin};
use super::{Node, Message, RFuture};

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
