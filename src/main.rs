// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

#[macro_use]
extern crate clap;
extern crate regex;
extern crate time;
extern crate termion;

use clap::{App, Arg, ArgMatches};
use regex::Regex;
use std::io::{BufReader, BufRead};
use std::process::{Command, Stdio};

mod terminal;
mod parser;
mod filewriter;

#[derive (Clone, PartialOrd, PartialEq)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
    Assert,
}

impl ::std::fmt::Display for Level {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f,
               "{}",
               match *self {
                   Level::Trace => "T",
                   Level::Debug => "D",
                   Level::Info => "I",
                   Level::Warn => "W",
                   Level::Error => "E",
                   Level::Fatal => "F",
                   Level::Assert => "A",
               })
    }
}

impl<'a> From<&'a str> for Level {
    fn from(s: &str) -> Self {
        match s {
            "T" => Level::Trace,
            "I" => Level::Info,
            "W" => Level::Warn,
            "E" => Level::Error,
            "F" => Level::Fatal,
            "A" => Level::Assert,
            "D" | _ => Level::Debug,
        }
    }
}

impl std::str::FromStr for Level {
    type Err = bool;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(s))
    }
}

pub struct Output<'a> {
    terminal: bool,
    file: Option<&'a str>,
}

pub struct Configuration<'a> {
    command: &'a str,
    args: Vec<&'a str>,
    full_tag: bool,
    time_diff: bool,
    show_date: bool,
    color: bool,
    level: Level,
    outputs: Output<'a>,
}
#[derive(Clone)]
pub struct Record {
    pub timestamp: ::time::Tm,
    pub message: String,
    pub level: Level,
    pub tag: String,
    pub process: String,
    pub thread: String,
}

pub trait Sink {
    fn open(&self);
    fn close(&self);
    fn process(&mut self, record: &Record);
}

struct Filter {
    regex: Option<Vec<Regex>>,
}

impl Filter {
    fn new(regex: Option<clap::Values>) -> Filter {
        if let Some(values) = regex {
            Filter {
                regex: Some(values.map(|e| Regex::new(e).unwrap_or_else(|_| std::process::exit(0)))
                    .collect::<Vec<Regex>>()),
            }
        } else {
            Filter { regex: None }
        }
    }

    fn is_match(&self, t: &str) -> bool {
        match self.regex {
            Some(ref r) => {
                for m in r {
                    if m.is_match(t) {
                        return true;
                    }
                }
                false
            }
            None => true,
        }
    }
}

fn configuration<'a>(args: &'a ArgMatches) -> Configuration<'a> {
    let command = args.value_of("COMMAND")
        .unwrap_or("adb logcat")
        .split_whitespace()
        .collect::<Vec<&str>>();

    let outputs = Output {
        terminal: true,
        file: None, // TODO
    };

    Configuration {
        command: command[0],
        args: command.iter().skip(1).map(|c| *c).collect(),
        color: !args.is_present("NO-COLOR"),
        full_tag: args.is_present("NO-TAG-SHORTENING"),
        time_diff: !args.is_present("NO-TIME-DIFF"),
        show_date: args.is_present("SHOW-DATE"),
        level: value_t!(args, "level", Level).unwrap_or(Level::Debug),
        outputs: outputs,
    }
}

fn main() {
    let matches = App::new("rogcat")
        .version(crate_version!())
        .author(crate_authors!())
        .about("A logcat wrapper")
        .arg_from_usage("-a --adb=[ADB BINARY] 'Path to adb'")
        .arg(Arg::from_usage("-t --tag [FILTER] 'Tag filters in RE2'").multiple(true))
        .arg(Arg::from_usage("-m --msg [FILTER] 'Message filters in RE2'").multiple(true))
        .arg_from_usage("-f --file [FILE] 'Write to file'")
        .arg_from_usage("-i --input [INPUT] 'Read from file instead of command'")
        .arg_from_usage("-l --level [LEVEL] 'Minumum loglevel'")
        .arg_from_usage("-c 'Clear (flush) the entire log and exit'")
        .arg_from_usage("-g 'Get the size of the log's ring buffer and exit'")
        .arg_from_usage("-S 'Output statistics'")
        .arg_from_usage("[NO-COLOR] --no-color 'Monochrome output'")
        .arg_from_usage("[NO-TAG-SHORTENING] --no-tag-shortening 'Disable shortening of tag'")
        .arg_from_usage("[NO-TIME-DIFF] --no-time-diff 'Disable tag time difference'")
        .arg_from_usage("[SHOW-DATE] --show-date 'Disable month and day display'")
        .arg_from_usage("[COMMAND] 'Optional command to run and capture stdout'")
        .get_matches();

    let configuration = configuration(&matches);

    let single_shots = ["c", "g", "S"];
    for arg in &single_shots {
        if matches.is_present(arg) {
            let arg = format!("-{}", arg);
            let mut child = Command::new("adb")
                .arg("logcat")
                .arg(arg)
                .spawn()
                .expect("Failed to execute adb");
            child.wait().ok();
            return;
        }
    }

    let level_filter = |record: &Record| -> bool { record.level >= configuration.level };
    let tag_filter = Filter::new(matches.values_of("tag"));
    let msg_filter = Filter::new(matches.values_of("msg"));

    let mut reader = if matches.is_present("input") {
        if let Some(f) = matches.value_of("input") {
            if let Ok(file) = std::fs::File::open(f) {
                BufReader::new(Box::new(file) as Box<std::io::Read>)
            } else {
                println!("Failed to read {}", f);
                return;
            }
        } else {
            println!("Cannot read input");
            return;
        }
    } else {
        let mut application = Command::new(configuration.command);
        for arg in configuration.args.iter() {
            application.arg(arg);
        }
        let application = application.stdout(Stdio::piped())
            .spawn()
            .expect("failed to execute adb");
        BufReader::new(Box::new(application.stdout.unwrap()) as Box<std::io::Read>)
    };

    let mut sinks = Vec::new();
    if configuration.outputs.file.is_some() {
        sinks.push(Box::new(filewriter::FileWriter::new(&configuration)) as Box<Sink>);
    }
    if configuration.outputs.terminal {
        sinks.push(Box::new(terminal::Terminal::new(&configuration)) as Box<Sink>);
    }

    let mut parser = parser::Parser::new();

    for s in &sinks {
        s.open();
    }

    loop {
        let mut buffer: Vec<u8> = Vec::new();
        if let Ok(len) = reader.read_until(b'\n', &mut buffer) {
            if len == 0 {
                break;
            } else {
                let line = String::from_utf8_lossy(&buffer);
                let record = parser.parse(&line);
                if tag_filter.is_match(&record.tag) && msg_filter.is_match(&record.message) &&
                   level_filter(&record) {
                    for s in &mut sinks {
                        s.process(&record);
                    }
                }
            }
        } else {
            println!("Invalid line");
        }
    }

    for s in &sinks {
        s.close();
    }
}
