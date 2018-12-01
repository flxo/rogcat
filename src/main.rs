// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use crate::record::Record;
use failure::{err_msg, Error};
use futures::{Future, Sink, Stream};
use std::io::{stderr, Write};
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;
use url::Url;

mod cli;
mod filewriter;
mod filter;
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

type LogStream = Stream<Item = StreamData, Error = Error> + Send;
type LogSink = Sink<SinkItem = Record, SinkError = Error> + Send;

fn run() -> Result<i32, Error> {
    let args = cli::cli().get_matches();
    utils::config_init();
    subcommands::run(&args);

    let source = {
        if args.is_present("input") {
            let input = args
                .value_of("input")
                .ok_or_else(|| err_msg("Invalid input value"))?;
            match Url::parse(input) {
                Ok(url) => match url.scheme() {
                    "serial" => reader::serial(&args),
                    _ => reader::file(PathBuf::from(input)),
                },
                _ => reader::file(PathBuf::from(input)),
            }
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
        filewriter::with_args(&args)?
    } else {
        terminal::with_args(&args, &profile)
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
            .map_err(|_| ()),
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
