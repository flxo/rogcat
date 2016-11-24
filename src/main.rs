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

use clap::{App, Arg, ArgMatches, Shell, SubCommand};
use record::{Level, Record};
use node::Nodes;

mod filereader;
mod filewriter;
mod filter;
mod node;
mod parser;
mod record;
mod runner;
mod terminal;

#[derive(Clone)]
pub struct Args {
    command: Vec<String>,
    input: Option<String>,
    output: Option<String>,
    output_csv: bool,
    full_tag: bool,
    time_diff: bool,
    show_date: bool,
    color: bool,
    level: Level,
    tag_filter: Vec<String>,
    msg_filter: Vec<String>,
}

impl Args {
    fn new(args: ArgMatches) -> Args {
        let command = args.value_of("COMMAND")
            .unwrap_or("adb logcat")
            .split_whitespace()
            .map(|s| s.to_owned())
            .collect::<Vec<String>>();

        let filter = |arg_name| {
            if args.is_present(arg_name) {
                args.values_of(arg_name).unwrap().map(|v| v.to_owned()).collect()
            } else {
                Vec::new()
            }
        };

        let file_arg = |arg| {
            if let Some(f) = args.value_of(arg) {
                // TODO: check file existence and readability
                Some(f.to_owned())
            } else {
                None
            }
        };

        Args {
            command: command,
            input: file_arg("input"),
            output: file_arg("output"),
            output_csv: args.is_present("csv"),
            color: !args.is_present("NO-COLOR"),
            full_tag: args.is_present("NO-TAG-SHORTENING"),
            time_diff: !args.is_present("NO-TIME-DIFF"),
            show_date: args.is_present("SHOW-DATE"),
            level: value_t!(args, "level", Level).unwrap_or(Level::Trace), /* TODO: warn about invalid level */
            tag_filter: filter("tag"),
            msg_filter: filter("msg"),
        }
    }
}


pub fn build_cli() -> App<'static, 'static> {
    App::new("rogcat")
        .version(crate_version!())
        .author(crate_authors!())
        .about("A logcat (and others) wrapper")
        .arg_from_usage("-a --adb=[ADB BINARY] 'Path to adb'") // TODO unimplemented
        .arg(Arg::from_usage("-t --tag [FILTER] 'Tag filters in RE2'").multiple(true))
        .arg(Arg::from_usage("-m --msg [FILTER] 'Message filters in RE2'").multiple(true))
        .arg_from_usage("-o --output [OUTPUT] 'Write to file instead to stdout'")
        .arg_from_usage("--csv 'Write csv like format instead of raw'")
        .arg_from_usage("-i --input [INPUT] 'Read from file instead of command'")
        .arg_from_usage("-l --level [LEVEL] 'Minumum level'")
        .arg_from_usage("-c 'Clear (flush) the entire log and exit'")
        .arg_from_usage("-g 'Get the size of the log's ring buffer and exit'")
        .arg_from_usage("-S 'Output statistics'")
        .arg_from_usage("[NO-COLOR] --no-color 'Monochrome output'")
        .arg_from_usage("[NO-TAG-SHORTENING] --no-tag-shortening 'Disable shortening of tag'")
        .arg_from_usage("[NO-TIME-DIFF] --no-time-diff 'Disable tag time difference'")
        .arg_from_usage("[SHOW-DATE] --show-date 'Disable month and day display'")
        .arg_from_usage("[COMMAND] 'Optional command to run and capture stdout'")
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

    let single_shots = ["c", "g", "S"];
    for arg in &single_shots {
        if matches.is_present(arg) {
            let arg = format!("-{}", arg);
            let mut child = std::process::Command::new("adb")
                .arg("logcat")
                .arg(arg)
                .spawn()
                .expect("Failed to execute adb");
            std::process::exit(child.wait().unwrap().code().unwrap());
        }
    }

    let args = Args::new(matches);
    let mut nodes = Nodes::<Record>::default();

    let input = if args.input.is_some() {
        nodes.add_node::<filereader::FileReader>(&args)
    } else {
        nodes.add_node::<runner::Runner>(&args)
    };

    let parser = nodes.add_node::<parser::Parser>(&args);
    input.add_target(&parser);

    let processing = if args.tag_filter.is_empty() && args.msg_filter.is_empty() &&
                        args.level == Level::Trace {
        parser
    } else {
        let filter = nodes.add_node::<filter::Filter>(&args);
        parser.add_target(&filter);
        filter
    };

    if args.output.is_some() {
        let file_output = nodes.add_node::<filewriter::FileWriter>(&args);
        processing.add_target(&file_output);
    }

    let terminal = nodes.add_node::<terminal::Terminal>(&args);
    processing.add_target(&terminal);

    nodes.run();
}
