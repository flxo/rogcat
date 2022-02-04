// Copyright © 2016 Felix Obenhuber
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use chrono::NaiveDateTime;
use std::fmt::Display;

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum Level {
    None,
    Trace,
    Verbose,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
    Assert,
}

impl Display for Level {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                Level::None => "-",
                Level::Trace => "T",
                Level::Verbose => "V",
                Level::Debug => "D",
                Level::Info => "I",
                Level::Warn => "W",
                Level::Error => "E",
                Level::Fatal => "F",
                Level::Assert => "A",
            }
        )
    }
}

impl Default for Level {
    fn default() -> Level {
        Level::None
    }
}

impl<'a> From<&'a str> for Level {
    fn from(s: &str) -> Self {
        match s {
            "T" | "trace" => Level::Trace,
            "V" | "verbose" => Level::Verbose,
            "D" | "debug" => Level::Debug,
            "I" | "info" => Level::Info,
            "W" | "warn" => Level::Warn,
            "E" | "error" => Level::Error,
            "F" | "fatal" => Level::Fatal,
            "A" | "assert" => Level::Assert,
            _ => Level::None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Record<'a> {
    pub timestamp: NaiveDateTime,
    pub message: &'a str,
    pub level: Level,
    pub tag: &'a str,
    pub process: u32,
    pub thread: u32,
    pub raw: &'a str,
}
