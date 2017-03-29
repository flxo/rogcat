// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

#[macro_use]
extern crate clap;
extern crate csv;
#[macro_use]
extern crate error_chain;
extern crate futures;
#[macro_use]
extern crate nom;
extern crate regex;
extern crate serial;
extern crate time;
extern crate terminal_size;
extern crate term_painter;
extern crate tempdir;
extern crate which;

use clap::{App, AppSettings, Arg, ArgMatches, Shell, SubCommand};
use error_chain::ChainedError;
use futures::future::*;
use record::Record;
use std::env;
use std::io::{stderr, stdout, Write};
use std::path::PathBuf;
use std::process::{Command, exit};
use term_painter::{Color, ToStyle};
use which::which_in;

mod errors;
mod filewriter;
mod filter;
mod parser;
mod record;
mod reader;
mod runner;
mod terminal;

use errors::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Record(Record),
    Drop,
    Done,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Format {
    Csv,
    Human,
    Raw,
}

impl ::std::str::FromStr for Format {
    type Err = &'static str;
    fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
        match s {
            "csv" => Ok(Format::Csv),
            "human" => Ok(Format::Human),
            "raw" => Ok(Format::Raw),
            _ => Err("Format parsing error"),
        }
    }
}

type RFuture = Box<Future<Item = Message, Error = Error>>;

pub trait Node {
    type Input;
    fn process(&mut self, msg: Self::Input) -> RFuture;
}

fn build_cli() -> App<'static, 'static> {
    App::new(crate_name!())
        .setting(AppSettings::ColoredHelp)
        .version(crate_version!())
        .author(crate_authors!())
        .about("A 'adb logcat' wrapper")
        .arg(Arg::from_usage("-t --tag [TAG] 'Tag filters in RE2. The prefix ! inverts the match.'").multiple(true))
        .arg(Arg::from_usage("-m --message [MSG] 'Message filters in RE2. The prefix ! inverts the match.'").multiple(true))
        .arg_from_usage("-o --output [OUTPUT] 'Write to file and stdout'")
        .arg(Arg::with_name("records-per-file")
            .short("n")
            .long("records-per-file")
            .takes_value(true)
            .requires("output")
            .help("Write n records per file. Use k, M, G suffixes or a number e.g 9k for 9000"))
        .arg(Arg::with_name("file-format")
            .long("file-format")
            .short("f")
            .takes_value(true)
            .requires("output")
            .possible_values(&["raw", "csv"])
            .help("Write format to output files"))
        .arg(Arg::with_name("terminal-format")
            .long("terminal-format")
            .short("e")
            .takes_value(true)
            .default_value("human")
            .possible_values(&["human", "raw", "csv"])
            .help("Use format on stdout"))
        .arg(Arg::from_usage("-i --input [INPUT] 'Read from file instead of command. Use 'serial://COM0@11520,8N1 or similiar for reading serial ports")
            .multiple(true))
        .arg(Arg::with_name("level")
            .short("l")
            .long("level")
            .takes_value(true)
            .possible_values(&["trace", "debug", "info", "warn", "error", "fatal", "assert", "T",
                               "D", "I", "W", "E", "F", "A"])
            .help("Minimum level"))
        .arg_from_usage("-r --restart 'Restart command on exit'")
        .arg_from_usage("-c --clear 'Clear (flush) the entire log and exit'")
        .arg_from_usage("-g --get-ringbuffer-size 'Get the size of the log's ring buffer and exit'")
        .arg_from_usage("-S --output-statistics 'Output statistics'")
        .arg_from_usage("--shorten-tags 'Shorten tag by removing vovels if too long'")
        .arg_from_usage("--show-date 'Show month and day'")
        .arg_from_usage("--show-time-diff 'Show time diff of tags'")
        .arg_from_usage("-s --skip-on-restart 'Skip messages on restart until last message from \
                         previous run is (re)received'")
        .arg_from_usage("[COMMAND] 'Optional command to run and capture stdout. Pass \"-\" to \
                         capture stdin'. If omitted, rogcat will run \"adb logcat -b all\"")
        .subcommand(SubCommand::with_name("completions")
            .about("Generates completion scripts for your shell")
            .arg(Arg::with_name("SHELL")
                .required(true)
                .possible_values(&["bash", "fish", "zsh"])
                .help("The shell to generate the script for")))
        .subcommand(SubCommand::with_name("devices").about("Show list of available devices"))
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

fn input(args: &ArgMatches) -> Result<Box<Node<Input = ()>>> {
    if args.is_present("input") {
        let input = args.value_of("input").ok_or("Invalid input value")?;
        if reader::SerialReader::parse_serial_arg(input).is_ok() {
            Ok(Box::new(reader::SerialReader::new(input)?))
        } else {
            Ok(Box::new(reader::FileReader::new(args)?))
        }
    } else {
        match args.value_of("COMMAND") {
            Some(c) => {
                if c == "-" {
                    Ok(Box::new(reader::StdinReader::new()))
                } else if reader::SerialReader::parse_serial_arg(c).is_ok() {
                    Ok(Box::new(reader::SerialReader::new(c)?))
                } else {
                    let cmd = c.to_owned();
                    let restart = args.is_present("restart");
                    let skip_on_restart = args.is_present("skip-on-restart");
                    Ok(Box::new(runner::Runner::new(cmd, restart, skip_on_restart)?))
                }
            }
            None => {
                adb()?;
                let cmd = "adb logcat -b all".to_owned();
                let restart = true;
                let skip_on_restart = args.is_present("skip-on-restart");
                Ok(Box::new(runner::Runner::new(cmd, restart, skip_on_restart)?))
            }
        }
    }
}

fn run(args: &ArgMatches) -> Result<i32> {
    match args.subcommand() {
        ("completions", Some(sub_matches)) => {
            let shell = sub_matches.value_of("SHELL").unwrap();
            build_cli().gen_completions_to(crate_name!(),
                                           shell.parse::<Shell>().unwrap(),
                                           &mut stdout());
            return Ok(0);
        }
        ("devices", _) => {
            let output = String::from_utf8(Command::new(adb()?)
                .arg("devices")
                .output()?
                .stdout)?;
            let error_msg = "Failed to parse adb output";
            let mut lines = output.lines();
            println!("{}:", lines.next().ok_or(error_msg)?);
            for l in lines {
                if !l.is_empty() && !l.starts_with("* daemon") {
                    let mut s = l.split_whitespace();
                    let id = s.next().ok_or(error_msg)?;
                    let name = s.next().ok_or(error_msg)?;
                    println!("{} {}",
                             terminal::DIMM_COLOR.paint(id),
                             match name {
                                 "unauthorized" => Color::Red.paint(name),
                                 _ => Color::Green.paint(name),
                             })
                }
            }
            return Ok(0);
        }
        (_, _) => (),
    }

    for arg in &["clear", "get-ringbuffer-size", "output-statistics"] {
        if args.is_present(arg) {
            let arg = format!("-{}",
                              match arg {
                                  &"clear" => "c",
                                  &"get-ringbuffer-size" => "g",
                                  &"output-statistics" => "S",
                                  _ => panic!(""),
                              });
            let mut child = Command::new(adb()?).arg("logcat")
                .arg(arg)
                .spawn()?;
            exit(child.wait()?.code().ok_or("Failed to get exit code")?);
        }
    }

    let mut input = input(args)?;
    let mut parser = parser::Parser::new();
    let mut filter = filter::Filter::new(args)?;
    let mut terminal = terminal::Terminal::new(args)?;
    let mut filewriter = if args.is_present("output") {
        Some(filewriter::FileWriter::new(args)?)
    } else {
        None
    };

    loop {
        let f = input.process(())
            .and_then(|r| parser.process(r))
            .and_then(|r| filter.process(r))
            .and_then(|r| {
                if let Some(ref mut f) = filewriter {
                    join_all(vec![terminal.process(r.clone()), f.process(r)])
                } else {
                    join_all(vec![terminal.process(r)])
                }
            });
        let res = f.wait()?;
        if res.iter().all(|r| *r == Message::Done) {
            return Ok(0);
        }
    }
}
