// Copyright © 2016 Felix Obenhuber
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crate::{
    lossy_lines::{lossy_lines, LossyLinesCodec},
    utils::{adb, config_get},
    LogStream, StreamData, DEFAULT_BUFFER,
};
use clap::{value_t, ArgMatches};
use failure::{err_msg, format_err, Error};
use futures::{stream::iter_ok, Async, Future, Stream};
use rogcat::record::Level;
#[cfg(target_os = "linux")]
use rogcat::record::{Record, Timestamp};
use std::{
    borrow::ToOwned,
    convert::Into,
    io::BufReader,
    net::ToSocketAddrs,
    path::PathBuf,
    process::{Command, Stdio},
};
use tokio::{
    codec::{Decoder, FramedRead},
    fs::File,
    net::TcpStream,
};
use tokio_process::{Child, CommandExt};
use url::Url;

/// A spawned child process that implements LogStream
struct Process {
    cmd: Vec<String>,
    /// Respawn cmd upone termination
    respawn: bool,
    child: Option<Child>,
    stream: Option<LogStream>,
}

/// Open a file and provide a stream of lines
pub fn files(args: &ArgMatches) -> Result<LogStream, Error> {
    let files = args
        .values_of("input")
        .ok_or_else(|| err_msg("Missing input argument"))?
        .map(PathBuf::from)
        .collect::<Vec<PathBuf>>();

    let f = iter_ok::<_, Error>(files)
        .map(|f| {
            File::open(f.clone())
                .map(|s| Decoder::framed(LossyLinesCodec::new(), s))
                .flatten_stream()
                .map(StreamData::Line)
                .map_err(move |e| format_err!("Failed to open {}: {}", f.display(), e))
        })
        .flatten();

    Ok(Box::new(f))
}

/// Open stdin and provide a stream of lines
pub fn stdin() -> LogStream {
    let s = FramedRead::new(tokio::io::stdin(), LossyLinesCodec::new())
        .map_err(Into::into)
        .map(StreamData::Line);
    Box::new(s)
}

/// Open a serial port and provide a stream of lines
pub fn serial(_args: &ArgMatches) -> LogStream {
    unimplemented!()
}

#[cfg(target_os = "linux")]
pub fn can(dev: &str) -> Result<LogStream, Error> {
    let process = dev.to_string();
    let now = time::now();
    let stream = tokio_socketcan::CANSocket::open(dev)?
        .map_err(std::convert::Into::into)
        .map(move |s| {
            let data = s
                .data()
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<String>>();
            let extended = if s.is_extended() { "E" } else { " " };
            StreamData::Record(Record {
                timestamp: Some(Timestamp::new(now)),
                message: format!("{} {} ", extended, data.join(" ")),
                tags: vec![format!("0x{:x}", s.id())],
                raw: format!(
                    "({}) {} {}#{}",
                    now.strftime("%s.%f").unwrap(),
                    process,
                    if s.is_extended() {
                        format!("{:08X}", s.id())
                    } else {
                        format!("{:X}", s.id())
                    },
                    data.join("")
                ),
                process: process.clone(),
                ..Default::default()
            })
        });
    Ok(Box::new(stream))
}

/// Connect to tcp socket and profile a stream of lines
pub fn tcp(addr: &Url) -> Result<LogStream, Error> {
    let addr = addr
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| err_msg("Failed to parse addr"))?;
    let s = TcpStream::connect(&addr)
        .map(|s| Decoder::framed(LossyLinesCodec::new(), s))
        .flatten_stream()
        .map_err(|e| format_err!("Failed to connect: {}", e))
        .map(StreamData::Line);

    Ok(Box::new(s))
}

/// Start logcat
pub fn logcat(args: &ArgMatches) -> Result<LogStream, Error> {
    let mut cmd = vec![adb()?.display().to_string()];

    if args.is_present("dev") {
        let device = value_t!(args, "dev", String).unwrap_or_else(|e| e.exit());
        cmd.push("-s".into());
        cmd.push(device);
    }

    cmd.push("logcat".into());
    let mut respawn = args.is_present("restart") | config_get::<bool>("restart").unwrap_or(true);

    if args.is_present("tail") {
        let count = value_t!(args, "tail", u32).unwrap_or_else(|e| e.exit());
        cmd.push("-t".into());
        cmd.push(count.to_string());
        respawn = false;
    };

    if args.is_present("dump") {
        cmd.push("-d".into());
        respawn = false;
    }

    if args.is_present("last") {
        cmd.push("--last".into());
        respawn = false;
    }

    for buffer in args
        .values_of("buffer")
        .map(|m| m.map(ToOwned::to_owned).collect::<Vec<String>>())
        .or_else(|| config_get("buffer"))
        .unwrap_or_else(|| DEFAULT_BUFFER.iter().map(|&s| s.to_owned()).collect())
    {
        cmd.push("-b".into());
        cmd.push(buffer);
    }

    Ok(Box::new(Process::with_cmd(cmd, respawn)))
}

/// Start ffx log
pub fn fuchsia(args: &ArgMatches) -> Result<LogStream, Error> {
    let mut cmd = vec!["ffx", "log", "--no-color"];

    if args.is_present("dump") {
        cmd.push("--dump");
    }

    if let Some(level) = args.value_of("level") {
        cmd.push("--severity");
        let level = Level::from(level);
        match level {
            Level::Verbose | Level::Trace => cmd.push("trace"),
            Level::None | Level::Debug => cmd.push("debug"),
            Level::Info => cmd.push("info"),
            Level::Warn => cmd.push("warn"),
            Level::Error => cmd.push("error"),
            Level::Fatal | Level::Assert => cmd.push("fatal"),
        };
    } else {
        cmd.extend(["--severity", "debug"]);
    }

    let cmd = cmd.iter().map(ToString::to_string).collect();

    Ok(Box::new(Process::with_cmd(cmd, false)))
}

/// Start a process and stream it stdout
pub fn process(args: &ArgMatches) -> Result<LogStream, Error> {
    let respawn = args.is_present("restart");
    let cmd = value_t!(args, "COMMAND", String)?
        .split_whitespace()
        .map(ToOwned::to_owned)
        .collect();
    Ok(Box::new(Process::with_cmd(cmd, respawn)))
}

impl Process {
    fn with_cmd(cmd: Vec<String>, respawn: bool) -> Process {
        Process {
            cmd,
            respawn,
            child: None,
            stream: None,
        }
    }

    fn spawn(&mut self) -> Result<Async<Option<StreamData>>, Error> {
        let mut child = Command::new(self.cmd[0].clone())
            .args(&self.cmd[1..])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn_async()?;

        let stdout = BufReader::new(child.stdout().take().unwrap());
        let stderr = BufReader::new(child.stderr().take().unwrap());
        self.child = Some(child);

        let stdout = lossy_lines(stdout)
            .map_err(Into::into)
            .map(StreamData::Line);
        let stderr = lossy_lines(stderr)
            .map_err(Into::into)
            .map(StreamData::Line);

        let mut stream = stdout.select(stderr);
        let poll = stream.poll();
        self.stream = Some(Box::new(stream));
        poll
    }
}

impl Stream for Process {
    type Item = StreamData;
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
        if let Some(ref mut inner) = self.stream {
            match inner.poll() {
                Ok(Async::Ready(None)) if self.respawn => self.spawn(),
                poll => poll,
            }
        } else {
            self.spawn()
        }
    }
}
