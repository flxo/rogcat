// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

extern crate appdirs;
extern crate bytes;
#[macro_use]
extern crate clap;
extern crate config;
extern crate csv;
extern crate crc;
#[macro_use]
extern crate error_chain;
extern crate handlebars;
extern crate futures;
extern crate indicatif;
#[macro_use]
extern crate nom;
#[macro_use]
extern crate lazy_static;
#[cfg(test)]
extern crate rand;
extern crate regex;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate serial;
#[cfg(test)]
extern crate tempdir;
extern crate time;
extern crate term_painter;
extern crate term_size;
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
use error_chain::ChainedError;
use errors::*;
use filewriter::FileWriter;
use filter::Filter;
use futures::future::ok;
use futures::{Future, Sink, Stream};
use parser::Parser;
use profiles::Profiles;
use reader::{FileReader, SerialReader, StdinReader, TcpReader};
use record::Record;
use runner::Runner;
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
mod profiles;
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
#[cfg(test)]
mod tests;

lazy_static! {
	static ref CONFIG: RwLock<Config> = RwLock::new(Config::default());
}

pub type RSink = Box<Sink<SinkItem = Option<Record>, SinkError = Error>>;
pub type RStream = Box<Stream<Item = Option<Record>, Error = Error>>;

fn main() {
    match run() {
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

/// Detect configuration directory
fn config_dir() -> Result<PathBuf> {
    appdirs::user_config_dir(Some("rogcat"), None, false)
            .map_err(|_| "Failed to detect config dir".into())
}

/// Read a value from the configuration file
/// config_dir/config.toml
fn config_get<'a, T>(key: &'a str) -> Option<T>
    where T: serde::Deserialize<'a>
{
    CONFIG.read()
        .ok()
        .and_then(|c| c.get::<T>(key).ok())
}

fn input(core: &mut Core, args: &ArgMatches) -> Result<RStream> {
    if args.is_present("input") {
        let input = args.value_of("input").ok_or("Invalid input value")?;
        if SerialReader::parse_serial_arg(input).is_ok() {
            Ok(Box::new(SerialReader::new(args, input, core)?))
        } else {
            Ok(Box::new(FileReader::new(args, core)?))
        }
    } else {
        match args.value_of("COMMAND") {
            Some(c) => {
                if c == "-" {
                    Ok(Box::new(StdinReader::new(args, core)))
                } else {
                    if let Ok(url) = Url::parse(c) {
                        match url.scheme() {
                            "tcp" => {
                                let addr =
                                    url.to_socket_addrs()?.next().ok_or("Failed to parse addr")?;
                                Ok(Box::new(TcpReader::new(args, &addr, core)?))
                            }
                            "serial" => Ok(Box::new(SerialReader::new(args, c, core)?)),
                            _ => Ok(Box::new(Runner::new(&args, core.handle())?)),
                        }
                    } else {
                        Ok(Box::new(Runner::new(&args, core.handle())?))
                    }
                }
            }
            None => Ok(Box::new(Runner::new(args, core.handle())?)),
        }
    }
}

fn run() -> Result<i32> {
    let args = cli().get_matches();
    let config_file = config_dir()?.join("config.toml");
    CONFIG.write().map_err(|_| "Failed to get config lock")?
        .merge(config::File::from(config_file)).ok();
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
        let buffer = ::config_get::<Vec<String>>("buffer")
            .unwrap_or(vec!("all".to_owned()))
            .join(" -b ");
        let child = Command::new(adb()?)
            .arg("logcat")
            .arg("-c")
            .arg("-b")
            .args(buffer.split(" "))
            .spawn_async(&core.handle())?;
        let output = core.run(child)?;
        exit(output.code().ok_or("Failed to get exit code")?);
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

    let result = input.select(ctrl_c)
        .take_while(|i| ok(!i.is_none()))
        .and_then(|m| parser.process(m))
        .filter(|m| filter.filter(m))
        .forward(output);

    core.run(result).map(|_| 0)
}
