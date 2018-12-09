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

use crate::record::Record;
use failure::Error;
use futures::{Future, Sink, Stream};
use std::io::{stderr, Write};
use std::process::exit;
use std::str::FromStr;
use url::Url;

mod cli;
mod filewriter;
mod filter;
mod lossy_lines;
mod parser;
mod profiles;
mod reader;
mod record;
mod subcommands;
mod terminal;
#[cfg(all(test, not(target_os = "windows")))]
mod tests;
mod utils;

const DEFAULT_BUFFER: [&str; 4] = ["main", "events", "crash", "kernel"];

#[derive(Debug, Clone)]
pub enum StreamData {
    Record(Record),
    Line(String),
}

type LogStream = Box<Stream<Item = StreamData, Error = Error> + Send>;
type LogSink = Box<Sink<SinkItem = Record, SinkError = Error> + Send>;

fn run() -> Result<i32, Error> {
    let args = cli::cli().get_matches();
    utils::config_init();
    subcommands::run(&args);

    let source = {
        if args.is_present("input") {
            reader::files(&args)?
        } else {
            match args.value_of("COMMAND") {
                Some(c) => {
                    if c == "-" {
                        reader::stdin()
                    } else if let Ok(url) = Url::parse(c) {
                        match url.scheme() {
                            "tcp" => reader::tcp(&url)?,
                            "serial" => reader::serial(&args),
                            _ => reader::process(&args)?,
                        }
                    } else {
                        reader::process(&args)?
                    }
                }
                None => reader::logcat(&args)?,
            }
        }
    };

    let profile = profiles::from_args(&args)?;
    let sink = if args.is_present("output") {
        filewriter::try_from(&args)?
    } else {
        terminal::try_from(&args, &profile)?
    };

    // Stop process after n records if argument head is passed
    let mut head = args
        .value_of("head")
        .map(|v| usize::from_str(v).expect("Invalid head arguement"));

    let filter = filter::from_args_profile(&args, &profile);
    let mut parser = parser::Parser::new();

    tokio::run(
        source
            .map(move |a| match a {
                StreamData::Line(l) => parser.parse(l),
                StreamData::Record(r) => r,
            })
            .filter(move |r| filter.filter(r))
            .take_while(move |_| {
                Ok(match head {
                    Some(0) => false,
                    Some(n) => {
                        head = Some(n - 1);
                        true
                    }
                    None => true,
                })
            })
            .forward(sink)
            .map(|_| ())
            .map_err(|e| eprintln!("{}", e)),
    );

    Ok(0)
}

fn main() {
    match run() {
        Err(e) => {
            let stderr = &mut stderr();
            let errmsg = "Error writing to stderr";
            writeln!(stderr, "{}", e).unwrap_or_else(|_| panic!(errmsg));
            exit(1)
        }
        Ok(r) => exit(r),
    }
}
