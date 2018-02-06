// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use futures::{Poll, Stream};
use std::env;
use std::io::BufRead;
use term_size::dimensions;
use tokio_io::AsyncRead;

pub fn terminal_width() -> Option<usize> {
    match dimensions() {
        Some((width, _)) => Some(width),
        None => env::var("COLUMNS")
            .ok()
            .and_then(|e| e.parse::<usize>().ok()),
    }}

pub struct LossyLines<A> {
    io: A,
    buffer: Vec<u8>,
}

pub fn lossy_lines<A>(a: A) -> LossyLines<A>
where
    A: AsyncRead + BufRead,
{    LossyLines {
        io: a,
        buffer: Vec::new(),
    }
}

impl<A> Stream for LossyLines<A>
where
    A: AsyncRead + BufRead,
{
    type Item = String;
    type Error = ::std::io::Error;

    fn poll(&mut self) -> Poll<Option<String>, ::std::io::Error> {
        let n = try_nb!(self.io.read_until(b'\n', &mut self.buffer));
        if n == 0 && self.buffer.is_empty() {
            Ok(None.into())
        } else {
            // Strip all \r\n occurences because on Windows "adb logcat" ends lines with "\r\r\n"
            while self.buffer.ends_with(&[b'\r']) || self.buffer.ends_with(&[b'\n']) {
                self.buffer.pop();
            }
            let line = String::from_utf8_lossy(&self.buffer).into();
            self.buffer.clear();
            Ok(Some(line).into())
        }
    }
}