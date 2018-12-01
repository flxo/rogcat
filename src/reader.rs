// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use crate::utils::{adb, config_get};
use crate::{LogStream, StreamData, DEFAULT_BUFFER};
use clap::value_t;
use clap::ArgMatches;
use failure::err_msg;
use failure::format_err;
use failure::Error;
use futures::Future;
use futures::{Async, Stream};
use std::io::BufReader;
use std::net::ToSocketAddrs;
use url::Url;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use tokio::codec::{Decoder, FramedRead, LinesCodec};
use tokio::fs::File;
use tokio::io::lines;
use tokio::net::TcpStream;
use tokio_process::{Child, CommandExt};

struct Process {
    _child: Child,
    inner: Box<LogStream>,
}

impl Stream for Process {
    type Item = StreamData;
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
        self.inner.poll()
    }
}

/// Open a file and provide a stream of lines
pub fn file(file: PathBuf) -> Box<LogStream> {
    let s = File::open(file.clone())
        .map(|s| Decoder::framed(LinesCodec::new(), s))
        .flatten_stream()
        .map_err(move |e| format_err!("Failed to read {}: {}", file.display(), e))
        .map(StreamData::Line);
    Box::new(s)
}

/// Open stdin and provide a stream of lines
pub fn stdin() -> Box<LogStream> {
    let s = FramedRead::new(tokio::io::stdin(), LinesCodec::new())
        .map_err(|e| e.into())
        .map(StreamData::Line);
    Box::new(s)
}

/// Open a serial port and provide a stream of lines
pub fn serial<'a>(_args: &ArgMatches<'a>) -> Box<LogStream> {
    unimplemented!()
}

/// Connect to tcp socket and profile a stream of lines
pub fn tcp(addr: &Url) -> Result<Box<LogStream>, Error> {
    let addr = addr
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| err_msg("Failed to parse addr"))?;
    let s = TcpStream::connect(&addr)
        .map(|s| Decoder::framed(LinesCodec::new(), s))
        .flatten_stream()
        .map_err(|e| format_err!("Failed to connect: {}", e))
        .map(StreamData::Line);

    Ok(Box::new(s))
}

/// Start a process and stream it stdout
pub fn logcat<'a>(args: &ArgMatches<'a>) -> Result<Box<LogStream>, Error> {
    let adb = format!("{}", adb()?.display());
    let mut logcat_args = vec![];

    let mut restart = args.is_present("restart");
    if !restart {
        restart = config_get::<bool>("restart").unwrap_or(true);
    }

    if args.is_present("tail") {
        let count = value_t!(args, "tail", u32).unwrap_or_else(|e| e.exit());
        logcat_args.push(format!("-t {}", count));
        restart = false;
    };

    if args.is_present("dump") {
        logcat_args.push("-d".to_owned());
        restart = false;
    }

    let buffer = args
        .values_of("buffer")
        .map(|m| m.map(|f| f.to_owned()).collect::<Vec<String>>())
        .or_else(|| config_get("buffer"))
        .unwrap_or_else(|| DEFAULT_BUFFER.iter().map(|&s| s.to_owned()).collect())
        .join(" -b ");
    let cmd = format!("{} logcat -b {} {}", adb, buffer, logcat_args.join(" "));
    let cmd = cmd.split_whitespace().collect::<Vec<&str>>();

    let mut child = Command::new(cmd[0])
        .args(&cmd[1..])
        .stdout(Stdio::piped())
        .spawn_async()?;

    let stdout = BufReader::new(child.stdout().take().unwrap());
    let stream = lines(stdout).map_err(|e| e.into()).map(StreamData::Line);

    if restart {
        // TODO
    }

    Ok(Box::new(Process {
        _child: child,
        inner: Box::new(stream),
    }))
}

/// Start a process and stream it stdout
pub fn process<'a>(args: &ArgMatches<'a>) -> Result<Box<LogStream>, Error> {
    let _restart = args.is_present("restart");
    let cmd = value_t!(args, "COMMAND", String)?;
    let cmd = cmd.split_whitespace().collect::<Vec<&str>>();
    let mut child = Command::new(cmd[0])
        .args(&cmd[1..])
        .stdout(Stdio::piped())
        .spawn_async()?;

    let stdout = BufReader::new(child.stdout().take().unwrap());
    let stream = lines(stdout).map_err(|e| e.into()).map(StreamData::Line);

    Ok(Box::new(Process {
        _child: child,
        inner: Box::new(stream),
    }))
}
