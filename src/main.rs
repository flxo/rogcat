// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

#[macro_use]
extern crate clap;
#[macro_use]
extern crate error_chain;
extern crate futures;
extern crate kabuki;
extern crate regex;
extern crate time;
extern crate terminal_size;
extern crate term_painter;
extern crate tempdir;
extern crate tokio_core;

use clap::{App, Arg, ArgMatches, Shell, SubCommand};
use futures::Future;
use kabuki::CallError;
use record::Record;
// use std::path::PathBuf;
use std::process::exit;
// use std::str::FromStr;
// use regex::Regex;
use tokio_core::reactor;

mod errors;
// mod filereader;
// mod filewriter;
mod filter;
// mod node;
mod parser;
mod record;
// mod runner;
mod stdinreader;
mod terminal;

use errors::*;

fn build_cli() -> App<'static, 'static> {
    App::new("rogcat")
        .version(crate_version!())
        .author(crate_authors!())
        .about("A 'adb logcat' wrapper")
        .arg(Arg::from_usage("-t --tag [TAG] 'Tag filters in RE2'")
             .multiple(true))
        .arg(Arg::from_usage("-m --msg [MSG] 'Message filters in RE2'")
             .multiple(true))
        .arg_from_usage("-o --output [OUTPUT] 'Write to file and stdout'")
        .arg(Arg::with_name("records-per-file")
             .short("n")
             .long("records-per-file")
             .takes_value(true)
             .requires("output")
             .help("Write n records per file. Use k, M, G suffixes or a number e.g 9k for 9000"))
        .arg(Arg::with_name("format")
             .long("format")
             .short("f")
             .takes_value(true)
             .requires("output")
             .possible_values(&["raw", "csv"])
             .help("Write format to output files"))
        .arg(Arg::from_usage("-i --input [INPUT] 'Read from file instead of command.")
             .multiple(true))
        .arg(Arg::with_name("level")
             .short("l")
             .long("level")
             .takes_value(true)
             .possible_values(&["trace", "debug", "info", "warn", "error", "fatal", "assert", "T", "D", "I", "W", "E", "F", "A"])
             .help("Minimum level"))
        .arg_from_usage("-r --restart 'Restart command on exit'")
        .arg_from_usage("-s --silent 'Do not print on stdout'")
        .arg_from_usage("-c --clear 'Clear (flush) the entire log and exit'")
        .arg_from_usage("-g --get-ringbuffer-size 'Get the size of the log's ring buffer and exit'")
        .arg_from_usage("-S --output-statistics 'Output statistics'")
        // .arg_from_usage("--no-color 'Monochrome output'")
        // .arg_from_usage("--no-tag-shortening 'Disable shortening of tag'")
        // .arg_from_usage("--no-time-diff 'Disable tag time difference'")
        // .arg_from_usage("--show-date 'Disable month and day display'")
        .arg_from_usage("[COMMAND] 'Optional command to run and capture stdout. Pass \"stdin\" to capture stdin'")
        .subcommand(SubCommand::with_name("completions")
                    .about("Generates completion scripts for your shell")
                    .arg(Arg::with_name("SHELL")
                         .required(true)
                         .possible_values(&["bash", "fish", "zsh"])
                         .help("The shell to generate the script for")))
}

fn main() {
    let matches = build_cli().get_matches();

    // Shell completion file generation
    match matches.subcommand() {
        ("completions", Some(sub_matches)) => {
            let shell = sub_matches.value_of("SHELL").unwrap();
            build_cli().gen_completions_to("rogcat",
                                           shell.parse::<Shell>().unwrap(),
                                           &mut std::io::stdout());
            std::process::exit(0);
        }
        (_, _) => (),
    }

    for arg in &["clear", "get-ringbuffer-size", "output-statistics"] {
        if matches.is_present(arg) {
            let arg = format!("-{}",
                              match arg {
                                  &"clear" => "c",
                                  &"get-ringbuffer-size" => "g",
                                  &"output-statistics" => "S",
                                  _ => panic!(""),
                              });
            let mut child = std::process::Command::new("adb")
                .arg("logcat")
                .arg(arg)
                .spawn()
                .expect("Failed to execute adb!");
            std::process::exit(child.wait().unwrap().code().unwrap());
        }
    }

    match run(matches) {
        Ok(_) => exit(0),
        Err(e) => {
            println!("{}", e);
            exit(1)
        }
    }
}

impl<T> From<CallError<T>> for Error {
    fn from(src: CallError<T>) -> Error {
        match src {
            CallError::Full(..) => "actor inbox full!".into(),
            CallError::Disconnected(..) => "actor shutdown".into(),
            CallError::Aborted => "actor aborted request".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Record(Record),
    Drop,
    Finished,
}

fn run<'a>(args: ArgMatches<'a>) -> Result<()> {
    let mut core = reactor::Core::new().unwrap();

    let mut stdinreader = kabuki::Builder::new().spawn(&core.handle(), stdinreader::StdinReader {});
    let mut parser = kabuki::Builder::new().spawn(&core.handle(), parser::Parser::new());
    let mut filter = kabuki::Builder::new().spawn(&core.handle(), filter::Filter::new(&args)?);
    let mut terminal = kabuki::Builder::new().spawn(&core.handle(), terminal::Terminal::new()?);

    loop {
        if core.run(stdinreader.call(())
                .and_then(|r| parser.call(r))
                .and_then(|r| filter.call(r))
                .and_then(|r| terminal.call(r)))? == Message::Finished {
            return Ok(());
        }
    }

    // let mut nodes = Nodes::<Record>::default();

    // let mut output = if args.is_present("silent") {
    //     vec![]
    // } else {
    //     vec![(nodes.register::<terminal::Terminal, _>((), None))?]
    // };
    // match args.value_of("output") {
    //     Some(o) => {
    //         let args = filewriter::Args {
    //             filename: PathBuf::from(o),
    //             format: match args.value_of("format") {
    //                 Some(s) => filewriter::Format::from_str(s)?,
    //                 None => filewriter::Format::Raw,
    //             },
    //             records_per_file: args.value_of("records-per-file")
    //                 .and_then(|l| {
    //                     Regex::new(r"^(\d+)([kMG])$")
    //                         .unwrap()
    //                         .captures(l)
    //                         .and_then(|caps| {
    //                             caps.at(1)
    //                                 .and_then(|size| u64::from_str(size).ok())
    //                                 .and_then(|size| Some((size, caps.at(2))))
    //                         })
    //                         .and_then(|(size, suffix)| {
    //                             match suffix {
    //                                 Some("k") => Some(1000 * size),
    //                                 Some("M") => Some(1000_000 * size),
    //                                 Some("G") => Some(1000_000_000 * size),
    //                                 _ => None,
    //                             }
    //                         })
    //                 }),
    //         };
    //         output.push(try!(nodes.register::<filewriter::FileWriter, _>(args, None)));
    //     }
    //     None => (),
    // }

    // if args.is_present("input") {
    //     let files = args.values_of("input")
    //         .map(|files| files.map(|f| PathBuf::from(f)).collect::<Vec<PathBuf>>())
    //         .ok_or("Failed to parse input file(s) argument(s)".to_owned())?;
    //     nodes.register::<filereader::FileReader, _>(files, parser)?;
    // } else {
    //     match args.value_of("COMMAND") {
    //         Some(c) => {
    //             if c == "stdin" {
    //                 nodes.register::<stdinreader::StdinReader, _>((), parser)?;
    //             } else {
    //                 let arg = (c.split_whitespace()
    //                                .map(|s| s.to_owned())
    //                                .collect::<Vec<String>>(),
    //                            args.is_present("restart"));
    //                 nodes.register::<runner::Runner, _>(arg, parser)?;
    //             }
    //         }
    //         None => {
    //             nodes.register::<runner::Runner, _>((vec!["adb".to_owned(), "logcat".to_owned()],
    //                                                 true),
    //                                                parser)?;
    //         }
    //     }
    // }

    // nodes.run()
}
