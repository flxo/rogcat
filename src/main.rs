// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

#[macro_use]
extern crate clap;
extern crate regex;
extern crate time;
extern crate terminal_size;
extern crate term_painter;
extern crate tempdir;

use clap::{App, Arg, ArgMatches, Shell, SubCommand};
use record::Record;
use node::Nodes;
use std::path::PathBuf;
use std::process::exit;

mod filereader;
mod filewriter;
mod filter;
mod node;
mod parser;
mod record;
mod runner;
mod stdinreader;
mod terminal;

fn build_cli() -> App<'static, 'static> {
    App::new("rogcat")
        .version(crate_version!())
        .author(crate_authors!())
        .about("A 'adb logcat' wrapper")
        //.arg_from_usage("-a --adb=[ADB BINARY] 'Path to adb'") // TODO unimplemented
        // .arg(Arg::from_usage("-t --tag [FILTER] 'Tag filters in RE2'").multiple(true))
        // .arg(Arg::from_usage("-m --msg [FILTER] 'Message filters in RE2'").multiple(true))
        .arg_from_usage("-o --output [OUTPUT] 'Write to file and stdout'")
        .arg_from_usage("--csv 'Write csv format instead'")
        .arg_from_usage("-i --input [INPUT] 'Read from file instead of command'")
        // .arg(Arg::with_name("level").short("l").long("level")
        //     .takes_value(true)
        //     .value_name("LEVEL")
        //     .help("Minimum level")
        //     .possible_values(&["trace", "debug", "info", "warn", "error", "fatal", "assert"]))
        .arg_from_usage("--restart 'Restart command on exit'")
        .arg_from_usage("-c 'Clear (flush) the entire log and exit'")
        .arg_from_usage("-g 'Get the size of the log's ring buffer and exit'")
        .arg_from_usage("-S 'Output statistics'")
        // .arg_from_usage("[NO-COLOR] --no-color 'Monochrome output'")
        // .arg_from_usage("[NO-TAG-SHORTENING] --no-tag-shortening 'Disable shortening of tag'")
        // .arg_from_usage("[NO-TIME-DIFF] --no-time-diff 'Disable tag time difference'")
        // .arg_from_usage("[SHOW-DATE] --show-date 'Disable month and day display'")
        .arg_from_usage("[COMMAND] 'Optional command to run and capture stdout. Use -- to read stdin.'")
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

    for arg in &["c", "g", "S"] {
        if matches.is_present(arg) {
            let arg = format!("-{}", arg);
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

fn run<'a>(args: ArgMatches<'a>) -> Result<(), String> {
    let mut nodes = Nodes::<Record>::default();

    let mut output = vec![(nodes.register::<terminal::Terminal, _>((), vec![]))?];
    match args.value_of("output") {
        Some(o) => output.push(try!(nodes.register::<filewriter::FileWriter, (PathBuf, bool)>((PathBuf::from(o), args.is_present("csv")), vec!()))),
        None => (),
    }

    let filter = try!(nodes.register::<filter::Filter, _>((), output));
    let parser = vec![try!(nodes.register::<parser::Parser, _>((), vec![filter]))];

    match args.value_of("input") {
        Some(i) => nodes.register::<filereader::FileReader, PathBuf>(PathBuf::from(i), parser)?,
        None => {
            match args.value_of("COMMAND") {
                Some(c) => {
                    if c.split_whitespace().last() == Some("--") {
                        nodes.register::<stdinreader::StdinReader, _>((), parser)?
                    } else {
                        let arg = (args.value_of("COMMAND")
                                       .unwrap_or("adb logcat")
                                       .split_whitespace()
                                       .map(|s| s.to_owned())
                                       .collect::<Vec<String>>(),
                                   args.is_present("restart"));
                        (nodes.register::<runner::Runner, (Vec<String>, bool)>(arg, parser))?
                    }
                }
                None => nodes.register::<stdinreader::StdinReader, _>((), parser)?,
            }
        }
    };

    nodes.run()
}
