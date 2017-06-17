// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

extern crate boolinator;
#[macro_use]
extern crate clap;
extern crate csv;
extern crate crc;
#[macro_use]
extern crate error_chain;
extern crate handlebars;
extern crate futures;
extern crate indicatif;
#[macro_use]
extern crate nom;
extern crate regex;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate serial;
extern crate time;
extern crate term_painter;
extern crate term_size;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_process;
extern crate tempdir;
extern crate which;
extern crate zip;

use clap::{App, AppSettings, Arg, ArgMatches, Shell, SubCommand};
use error_chain::ChainedError;
use errors::*;
use filewriter::FileWriter;
use filter::Filter;
use futures::future::*;
use futures::{Sink, Stream};
use parser::Parser;
use reader::{FileReader, SerialReader, StdinReader};
use record::Record;
use runner::Runner;
use std::env;
use std::io::{stderr, stdout, Write};
use std::path::PathBuf;
use std::process::{exit, Command};
use std::str::FromStr;
use terminal::Terminal;
use tokio_core::reactor::Core;
use tokio_process::CommandExt;
use which::which_in;

mod bugreport;
mod devices;
mod errors;
mod filewriter;
mod filter;
mod log;
mod parser;
mod record;
mod reader;
mod runner;
mod terminal;

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Done,
    Drop,
    Record(Record),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Format {
    Csv,
    Html,
    Human,
    Raw,
}

impl FromStr for Format {
    type Err = &'static str;
    fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
        match s {
            "csv" => Ok(Format::Csv),
            "html" => Ok(Format::Html),
            "human" => Ok(Format::Human),
            "raw" => Ok(Format::Raw),
            _ => Err("Format parsing error"),
        }
    }
}

fn build_cli() -> App<'static, 'static> {
    App::new(crate_name!())
        .setting(AppSettings::ColoredHelp)
        .version(crate_version!())
        .author(crate_authors!())
        .about("A 'adb logcat' wrapper and log processor")
        .arg(Arg::from_usage("-t --tag [TAG] 'Tag filters in RE2. The prefix '!' inverts the match'").multiple(true))
        .arg(Arg::from_usage("-m --message [MSG] 'Message (payload) filters in RE2. The prefix ! inverts the match'").multiple(true))
        .arg(Arg::from_usage("-h --highlight [HIGHLIGHT] 'Highlight messages that match this pattern in RE2").multiple(true))
        .arg_from_usage("-o --output [OUTPUT] 'Write output to file'")
        .arg(Arg::with_name("RECORDS_PER_FILE")
            .short("n")
            .long("records-per-file")
            .takes_value(true)
            .requires("output")
            .help("Write n records per file. Use k, M, G suffixes or a plain number"))
        .arg(Arg::with_name("FILE_FORMAT")
            .long("file-format")
            .short("f")
            .takes_value(true)
            .requires("output")
            .possible_values(&["csv", "html", "raw"])
            .help("Select format for output files"))
        .arg(Arg::with_name("FILENAME_FORMAT")
            .long("filename-format")
            .short("a")
            .takes_value(true)
            .requires("output")
            .possible_values(&["single", "enumerate", "date"])
            .help("Select format for output file names. By passing 'single' the filename provided with the '-o' option is used. \
                  'enumerate' appends a file sequence number after the filename passed with '-o' option whenever a new file is \
                  created (see 'records-per-file' option). 'date' will prefix the output filename with the current local date \
                  when a new file is created"))
        .arg(Arg::with_name("TERMINAL_FORMAT")
            .long("terminal-format")
            .short("e")
            .takes_value(true)
            .default_value("human")
            .possible_values(&["human", "raw", "csv"])
            .help("Select format for stdout"))
        .arg(Arg::from_usage("-i --input [INPUT] 'Read from file instead of command. Use 'serial://COM0@11520,8N1 or similiar for reading a serial port")
            .multiple(true))
        .arg(Arg::with_name("LEVEL")
            .short("l")
            .long("level")
            .takes_value(true)
            .possible_values(&["trace", "debug", "info", "warn", "error", "fatal", "assert", "T",
                               "D", "I", "W", "E", "F", "A"])
            .help("Minimum level"))
        .arg(Arg::with_name("OVERWRITE")
            .long("overwrite")
            .requires("output")
            .help("Overwrite output file if present"))
        .arg_from_usage("-r --restart 'Restart command on exit'")
        .arg_from_usage("-c --clear 'Clear (flush) the entire log and exit'")
        .arg_from_usage("-g --get-ringbuffer-size 'Get the size of the log's ring buffer and exit'")
        .arg_from_usage("-S --output-statistics 'Output statistics'")
        .arg(Arg::with_name("TAIL")
             .short("T")
             .long("tail")
             .takes_value(true)
             .conflicts_with_all(&["input", "COMMAND"]) // remove input here once implemented
             .help("Dump only the most recent <COUNT> lines (implies --dump)"))
        .arg(Arg::with_name("DUMP")
             .short("d")
             .long("dump")
             .conflicts_with_all(&["input", "COMMAND"]) // remove input here once implemented
             .help("Dump the log and then exit (don't block)"))
        .arg(Arg::with_name("SHORTEN_TAGS")
            .long("shorten-tags")
            .conflicts_with("output")
            .help("Shorten tags by removing vovels if too long for terminal format"))
        .arg(Arg::with_name("NO_TIMESTAMP")
            .long("no-timestamp")
            .conflicts_with("output")
            .help("No timestamp in terminal output"))
        .arg(Arg::with_name("SHOW_DATE")
            .long("show-date")
            .conflicts_with("output")
            .help("Show month and day in terminal output"))
        .arg(Arg::with_name("SHOW_TIME_DIFF")
            .long("show-time-diff")
            .conflicts_with("output")
            .help("Show the time difference between the occurence of equal tags in terminal output"))
        .arg_from_usage("[COMMAND] 'Optional command to run and capture stdout from. Pass \"-\" to \
                         capture stdin'. If omitted, rogcat will run \"adb logcat -b all\" and restarts this commmand if 'adb' terminates")
        .subcommand(SubCommand::with_name("completions")
            .about("Generates completion scripts")
            .arg(Arg::with_name("SHELL")
                .required(true)
                .possible_values(&["bash", "fish", "zsh"])
                .help("The shell to generate the script for")))
        .subcommand(SubCommand::with_name("devices").about("Show list of available devices"))
        .subcommand(SubCommand::with_name("bugreport")
            .about("Capture bugreport. This is only works for Android versions < 7.")
            .arg(Arg::with_name("ZIP")
                .short("z")
                .long("zip")
                .help("Zip report"))
            .arg(Arg::with_name("OVERWRITE")
                .long("overwrite")
                .help("Overwrite report file if present"))
            .arg(Arg::with_name("FILE")
                .help("Output file name - defaults to <now>-bugreport")))
        .subcommand(SubCommand::with_name("log")
            .about("Add log message(s) log buffer")
            .arg(Arg::with_name("TAG")
                .short("t")
                .long("tag")
                .takes_value(true)
                .help("Log tag"))
            .arg(Arg::with_name("LEVEL")
                .short("l")
                .long("level")
                .takes_value(true)
                .possible_values(&["trace", "debug", "info", "warn", "error", "fatal", "assert", "T",
                                   "D", "I", "W", "E", "F", "A"])
            .help("Log on level"))
            .arg_from_usage("[MESSAGE] 'Log message. Pass \"-\" to capture from stdin'."))
}

fn main() {
    match run(&build_cli().get_matches()) {
        Err(e) => {
            let stderr = &mut stderr();
            let errmsg = "Error writing to stderr";
            writeln!(stderr, "{}", e.display()).expect(errmsg);
            exit(1)
        }
        Ok(r) => exit(r),
    }
}

fn adb() -> Result<PathBuf> {
    which_in("adb", env::var_os("PATH"), env::current_dir()?)
        .map_err(|e| format!("Cannot find adb: {}", e).into())
}

fn input(core: &Core, args: &ArgMatches) -> Result<Box<Stream<Item = Message, Error = Error>>> {
    if args.is_present("input") {
        let input = args.value_of("input").ok_or("Invalid input value")?;
        if SerialReader::parse_serial_arg(input).is_ok() {
            Ok(Box::new(SerialReader::new(input, core)?))
        } else {
            Ok(Box::new(FileReader::new(args, core)?))
        }
    } else {
        match args.value_of("COMMAND") {
            Some(c) => {
                if c == "-" {
                    Ok(Box::new(StdinReader::new(core)))
                } else if SerialReader::parse_serial_arg(c).is_ok() {
                    Ok(Box::new(SerialReader::new(c, core)?))
                } else {
                    let cmd = c.to_owned();
                    let restart = args.is_present("restart");
                    Ok(Box::new(Runner::new(core.handle(), cmd, restart, false)?))
                }
            }
            None => {
                let mut logcat_args = vec![];
                if args.is_present("LAST") {
                    let count = value_t!(args, "LAST", u32).unwrap_or_else(|e| e.exit());
                    logcat_args.push(format!("-t {}", count));
                };

                if args.is_present("DUMP") {
                    logcat_args.push("-d".to_owned());
                }
                let cmd = format!(
                    "{} logcat -b all {}",
                    adb()?.display(),
                    logcat_args.join(" ")
                );
                Ok(Box::new(Runner::new(
                    core.handle(),
                    cmd,
                    logcat_args.is_empty(),
                    false,
                )?))
            }
        }
    }
}

fn run(args: &ArgMatches) -> Result<i32> {
    let mut core = Core::new()?;

    match args.subcommand() {
        ("completions", Some(sub_matches)) => {
            let shell = sub_matches.value_of("SHELL").unwrap();
            build_cli().gen_completions_to(
                crate_name!(),
                shell.parse::<Shell>().unwrap(),
                &mut stdout(),
            );
            return Ok(0);
        }
        ("devices", _) => exit(devices::devices(&mut core)?),
        ("bugreport", Some(sub_matches)) => exit(bugreport::create(sub_matches, &mut core)?),
        ("log", Some(sub_matches)) => exit(log::run(sub_matches, &mut core)?),
        (_, _) => (),
    }

    for arg in &["clear", "get-ringbuffer-size", "output-statistics"] {
        if args.is_present(arg) {
            let arg = format!(
                "-{}",
                match arg {
                    &"clear" => "c",
                    &"get-ringbuffer-size" => "g",
                    &"output-statistics" => "S",
                    _ => panic!(""),
                }
            );
            let child = Command::new(adb()?).arg("logcat").arg(arg).spawn_async(
                &core.handle(),
            )?;
            let output = core.run(child)?;
            exit(output.code().ok_or("Failed to get exit code")?);
        }
    }

    type RSink = Box<Sink<SinkItem = Message, SinkError = Error>>;
    let output = if args.is_present("output") {
        Box::new(FileWriter::new(args)?) as RSink
    } else {
        Box::new(Terminal::new(args)?) as RSink
    };
    let mut parser = Parser::new();
    let mut filter = Filter::new(args)?;

    let result = input(&core, args)?
        .take_while(|r| ok(r != &Message::Done))
        .and_then(|m| parser.process(m))
        .filter(|m| filter.filter(m))
        .forward(output);

    core.run(result).map(|_| 0)
}
