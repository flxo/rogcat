// Copyright © 2017 Felix Obenhuber
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
    cli::cli,
    reader::stdin,
    utils::{self, adb},
    StreamData, DEFAULT_BUFFER,
};
use clap::{crate_name, value_t, ArgMatches};
use failure::{err_msg, Error};
use futures::{
    future::ok, stream::Stream, sync::oneshot, Async, AsyncSink, Future, Poll, Sink, StartSend,
};
use indicatif::{ProgressBar, ProgressStyle};
use rogcat::record::Level;
use std::{
    borrow::ToOwned,
    fs::{DirBuilder, File},
    io::{BufReader, Write},
    path::{Path, PathBuf},
    process::{exit, Command, Stdio},
};
use time::{now, strftime};
use tokio::{io::lines, runtime::Runtime};
use tokio_process::CommandExt;
use zip::{write::FileOptions, CompressionMethod, ZipWriter};

pub fn run(args: &ArgMatches) {
    match args.subcommand() {
        ("bugreport", Some(sub_matches)) => bugreport(sub_matches),
        ("clear", Some(sub_matches)) => clear(sub_matches),
        ("completions", Some(sub_matches)) => completions(sub_matches),
        ("devices", _) => devices(),
        ("log", Some(sub_matches)) => log(sub_matches),
        (_, _) => (),
    }
}

pub fn completions(args: &ArgMatches) {
    if let Err(e) = args
        .value_of("shell")
        .ok_or_else(|| err_msg("Required shell argument is missing"))
        .map(str::parse)
        .map(|s| {
            cli().gen_completions_to(crate_name!(), s.unwrap(), &mut std::io::stdout());
        })
    {
        eprintln!("Failed to get shell argument: {e}");
        exit(1);
    } else {
        exit(0);
    }
}

struct ZipFile {
    zip: ZipWriter<File>,
}

impl ZipFile {
    fn create(filename: &str) -> Result<Self, Error> {
        let file = File::create(format!("{filename}.zip"))?;
        let options = FileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        let filename_path = PathBuf::from(&filename);
        let f = filename_path
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .ok_or_else(|| err_msg("Failed to get filename"))?;
        let mut zip = ZipWriter::new(file);
        zip.start_file(f, options)?;
        Ok(ZipFile { zip })
    }
}

impl Write for ZipFile {
    fn write(&mut self, buf: &[u8]) -> ::std::io::Result<usize> {
        self.zip.write_all(buf).map(|_| buf.len())
    }

    fn flush(&mut self) -> ::std::io::Result<()> {
        self.zip
            .finish()
            .map_err(std::convert::Into::into)
            .map(|_| ())
    }
}

impl Drop for ZipFile {
    fn drop(&mut self) {
        self.flush().expect("Failed to close zipfile");
    }
}

fn report_filename() -> Result<String, Error> {
    #[cfg(not(windows))]
    let sep = ":";
    #[cfg(windows)]
    let sep = "_";

    let format = format!("%m-%d_%H{sep}%M{sep}%S");
    Ok(format!("{}-bugreport.txt", strftime(&format, &now())?))
}

/// Performs a dumpstate and write to fs. Note: The Android 7+ dumpstate is not supported.
pub fn bugreport(args: &ArgMatches) {
    let filename = value_t!(args.value_of("file"), String)
        .unwrap_or_else(|_| report_filename().expect("Failed to generate filename"));
    let filename_path = PathBuf::from(&filename);
    if !args.is_present("overwrite") && filename_path.exists() {
        eprintln!("File {filename} already exists");
        exit(1);
    }
    let mut adb = adb().expect("Failed to find adb");

    if args.is_present("dev") {
        let device = value_t!(args, "dev", String).unwrap_or_else(|e| e.exit());
        adb.push::<String>("-s".into());
        adb.push(device);
    }

    let mut child = Command::new(adb)
        .arg("bugreport")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn_async()
        .expect("Failed to launch adb");
    let stdout = BufReader::new(child.stdout().take().unwrap());

    let dir = filename_path.parent().unwrap_or_else(|| Path::new(""));
    if !dir.is_dir() {
        DirBuilder::new()
            .recursive(true)
            .create(dir)
            .expect("Failed to create outfile parent directory");
    }

    let progress = ProgressBar::new(u64::MAX);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.yellow} {msg:.dim.bold} {pos:>7.dim} {elapsed_precise:.dim}")
            .progress_chars(" • "),
    );
    progress.set_message("Connecting");

    let mut write = if args.is_present("zip") {
        Box::new(ZipFile::create(&filename).expect("Failed to create zip file")) as Box<dyn Write>
    } else {
        Box::new(File::create(&filename).expect("Failed to craete file")) as Box<dyn Write>
    };

    progress.set_message("Pulling bugreport line");

    // TODO: Migrate to tokio::fs::File
    let output = tokio::io::lines(stdout)
        .for_each(|l| {
            write.write_all(l.as_bytes()).expect("Failed to write");
            write.write_all(b"\n").expect("Failed to write");
            progress.inc(1);
            ok(())
        })
        .then(|r| {
            progress.set_style(ProgressStyle::default_bar().template("{msg:.dim.bold}"));
            progress.finish_with_message(&format!("Finished {}.", filename_path.display()));
            r
        })
        .map_err(|e| {
            eprintln!("Failed to create bugreport: {e}");
            exit(1);
        });

    tokio::runtime::current_thread::block_on_all(output).expect("Runtime error");
    exit(0);
}

pub fn devices() {
    let mut child = Command::new(adb().expect("Failed to find adb"))
        .arg("devices")
        .stdout(Stdio::piped())
        .spawn_async()
        .expect("Failed to run adb devices");
    let reader = BufReader::new(child.stdout().take().unwrap());
    let result = lines(reader)
        .skip(1)
        .filter(|l| !l.is_empty())
        .filter(|l| !l.starts_with("* daemon"))
        .for_each(|l| {
            let mut s = l.split_whitespace();
            let id: &str = s.next().unwrap_or("unknown");
            let name: &str = s.next().unwrap_or("unknown");
            println!("{id} {name}");
            Ok(())
        });

    tokio::run(
        result
            .map_err(|e| {
                eprintln!("Failed to run adb devices: {e}");
                exit(1)
            })
            .map(|_| exit(0)),
    );
}

struct Logger {
    tag: String,
    level: Level,
}

impl Logger {
    fn level(level: &Level) -> &str {
        match *level {
            Level::Trace | Level::Verbose => "v",
            Level::Debug | Level::None => "d",
            Level::Info => "i",
            Level::Warn => "w",
            Level::Error | Level::Fatal | Level::Assert => "e",
        }
    }
}

impl Sink for Logger {
    type SinkItem = String;
    type SinkError = Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        let child = Command::new(adb()?)
            .arg("shell")
            .arg("log")
            .arg("-p")
            .arg(Self::level(&self.level))
            .arg("-t")
            .arg(format!("\"{}\"", &self.tag))
            .arg(&item)
            .stdout(Stdio::piped())
            .output_async()
            .map(|_| ())
            .map_err(|_| ());
        tokio::spawn(child);
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}

/// Call something like adb shell log <message>
pub fn log(args: &ArgMatches) {
    let message = args.value_of("MESSAGE").unwrap_or("");
    let tag = args.value_of("tag").unwrap_or("Rogcat").to_owned();
    let level = Level::from(args.value_of("level").unwrap_or(""));
    match message {
        "-" => {
            let sink = Logger { tag, level };
            let stream = stdin()
                .map(|d| match d {
                    StreamData::Line(l) => l,
                    _ => panic!("Received non line item during log"),
                })
                .forward(sink)
                .map(|_| ())
                .map_err(|_| ());
            tokio::run(stream);
        }
        _ => {
            let child = Command::new(adb().expect("Failed to find adb"))
                .arg("shell")
                .arg("log")
                .arg("-p")
                .arg(Logger::level(&level))
                .arg("-t")
                .arg(&tag)
                .arg(format!("\"{message}\""))
                .stdout(Stdio::piped())
                .output_async()
                .map(|_| ())
                .map_err(|_| ());
            tokio::run(child)
        }
    }

    exit(0);
}

/// Call adb logcat -c -b BUFFERS
pub fn clear(args: &ArgMatches) {
    let buffer = args
        .values_of("buffer")
        .map(|m| m.map(ToOwned::to_owned).collect::<Vec<String>>())
        .or_else(|| utils::config_get("buffer"))
        .unwrap_or_else(|| DEFAULT_BUFFER.iter().map(|&s| s.to_owned()).collect())
        .join(" -b ");
    let child = Command::new(adb().expect("Failed to find adb"))
        .arg("logcat")
        .arg("-c")
        .arg("-b")
        .args(buffer.split(' '))
        .spawn_async()
        .expect("Failed to run adb");

    let runtime = Runtime::new().expect("Failed to start runtime");
    let h = oneshot::spawn(child, &runtime.executor());
    exit(h.wait().expect("Failed to run").code().unwrap_or(1));
}
