// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

extern crate atty;
extern crate bytes;
#[macro_use]
extern crate clap;
extern crate config;
extern crate crc;
extern crate csv;
extern crate directories;
#[macro_use]
extern crate failure;
extern crate futures;
extern crate handlebars;
extern crate indicatif;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate nom;
#[cfg(test)]
extern crate rand;
extern crate regex;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serial;
#[cfg(test)]
extern crate tempdir;
extern crate term;
extern crate term_size;
extern crate time;
extern crate tokio_core;
#[macro_use]
extern crate tokio_io;
extern crate tokio_process;
extern crate tokio_signal;
extern crate toml;
extern crate url;
extern crate which;
extern crate zip;

use cli::cli;
use clap::ArgMatches;
use config::Config;
use failure::{err_msg, Error};
use filewriter::FileWriter;
use filter::Filter;
use futures::future::ok;
use futures::{Future, Sink, Stream};
use parser::Parser;
use profiles::Profiles;
use reader::{file_reader, serial_reader, stdin_reader, tcp_reader};
use record::Record;
use runner::runner;
use std::env;
use std::io::{stderr, Write};
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::process::{exit, Command};
use std::sync::RwLock;
use terminal::Terminal;
use tokio_core::reactor::Core;
use tokio_process::CommandExt;
use url::Url;
use which::which_in;

mod bugreport;
mod cli;
mod devices;
mod filewriter;
mod filter;
mod log;
mod parser;
mod profiles;
mod reader;
mod record;
mod runner;
mod terminal;
mod utils;
#[cfg(test)]
mod tests;

lazy_static! {
    static ref CONFIG: RwLock<Config> = RwLock::new(Config::default());
}

pub type RSink = Box<Sink<SinkItem = Option<Record>, SinkError = Error>>;
pub type RStream = Box<Stream<Item = std::option::Option<Record>, Error = Error>>;

const DEFAULT_BUFFER: [&str; 4] = ["main", "events", "crash", "kernel"];

fn main() {
    match run() {
        Err(e) => {
            let stderr = &mut stderr();
            let errmsg = "Error writing to stderr";
            writeln!(stderr, "{}", e).expect(errmsg);
            exit(1)
        }
        Ok(r) => exit(r),
    }
}

fn adb() -> Result<PathBuf, Error> {
    which_in("adb", env::var_os("PATH"), env::current_dir()?)
        .map_err(|e| format_err!("Cannot find adb: {}", e))
}

/// Detect configuration directory
fn config_dir() -> PathBuf {
    directories::BaseDirs::new().config_dir().join("rogcat")
}

/// Read a value from the configuration file
/// `config_dir/config.toml`
fn config_get<'a, T>(key: &'a str) -> Option<T>
where
    T: serde::Deserialize<'a>,
{
    CONFIG.read().ok().and_then(|c| c.get::<T>(key).ok())
}

fn input(core: &mut Core, args: &ArgMatches) -> Result<RStream, Error> {
    if args.is_present("input") {
        let input = args.value_of("input")
            .ok_or_else(|| err_msg("Invalid input value"))?;
        match Url::parse(input) {
            Ok(url) => match url.scheme() {
                "serial" => serial_reader(args, core),
                _ => file_reader(args, core),
            },
            _ => file_reader(args, core),
        }
    } else {
        match args.value_of("COMMAND") {
            Some(c) => {
                if c == "-" {
                    stdin_reader(core)
                } else if let Ok(url) = Url::parse(c) {
                    match url.scheme() {
                        "tcp" => {
                            let addr = url.to_socket_addrs()?
                                .next()
                                .ok_or_else(|| err_msg("Failed to parse addr"))?;
                            tcp_reader(&addr, core)
                        }
                        "serial" => serial_reader(args, core),
                        _ => runner(args, core.handle()),
                    }
                } else {
                    runner(args, core.handle())
                }
            }
            None => runner(args, core.handle()),
        }
    }
}

fn run() -> Result<i32, Error> {
    let args = cli().get_matches();
    let config_file = config_dir().join("config.toml");
    CONFIG
        .write()
        .map_err(|e| format_err!("Failed to get config lock: {}", e))?
        .merge(config::File::from(config_file))
        .ok();
    let profiles = Profiles::new(&args)?;
    let profile = profiles.profile();
    let mut core = Core::new()?;

    match args.subcommand() {
        ("bugreport", Some(sub_matches)) => exit(bugreport::create(sub_matches, &mut core)?),
        ("completions", Some(sub_matches)) => exit(cli::subcommand_completions(sub_matches)?),
        ("devices", _) => exit(devices::devices(&mut core)?),
        ("log", Some(sub_matches)) => exit(log::run(sub_matches, &mut core)?),
        ("profiles", Some(sub_matches)) => exit(profiles.subcommand(sub_matches)?),
        (_, _) => (),
    }

    if args.is_present("clear") {
        let buffer = args.values_of("buffer")
            .map(|m| m.map(|f| f.to_owned()).collect::<Vec<String>>())
            .or_else(|| ::config_get("buffer"))
            .unwrap_or_else(|| DEFAULT_BUFFER.iter().map(|&s| s.to_owned()).collect())
            .join(" -b ");
        let child = Command::new(adb()?)
            .arg("logcat")
            .arg("-c")
            .arg("-b")
            .args(buffer.split(' '))
            .spawn_async(&core.handle())?;
        let output = core.run(child)?;
        exit(output
            .code()
            .ok_or_else(|| err_msg("Failed to get exit code"))?);
    }

    let input = input(&mut core, &args)?;
    let ctrl_c = tokio_signal::ctrl_c(&core.handle())
        .flatten_stream()
        .map(|_| None)
        .map_err(|e| e.into());
    let mut parser = Parser::new();
    let mut filter = Filter::new(&args, &profile)?;
    let output = if args.is_present("output") {
        Box::new(FileWriter::new(&args)?) as RSink
    } else {
        Box::new(Terminal::new(&args, &profile)?) as RSink
    };

    let mut cnt = if args.is_present("head") {
        Some(value_t!(args, "head", usize)?)
    } else {
        None
    };
    let mut head = || {
        ok(match cnt {
            Some(0) => false,
            Some(_) => {
                cnt = cnt.map(|s| s - 1);
                true
            }
            None => true,
        })
    };

    let result = input
        .select(ctrl_c)
        .take_while(|i| ok(i.is_some()))
        .and_then(|m| parser.process(m))
        .filter(|m| filter.filter(m))
        .take_while(|_| head())
        .forward(output);

    core.run(result).map(|_| 0)
}
