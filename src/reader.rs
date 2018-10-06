// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use bytes::BytesMut;
use clap::ArgMatches;
use failure::{err_msg, Error};
use futures::sync::mpsc;
use futures::{stream, Future, Sink, Stream};
use record::Record;
use std::fs::File;
use std::io::stdin;
use std::io::{BufRead, BufReader, Read};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::thread;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_io::codec::{Decoder, Encoder};
use RStream;

fn records<T: Read + Send + Sized + 'static>(reader: T, core: &Core) -> Result<RStream, Error> {
    let (tx, rx) = mpsc::channel(1);
    let mut reader = BufReader::new(reader);
    let remote = core.remote();

    thread::spawn(move || {
        let mut buffer = Vec::new();
        loop {
            let mut tx = tx.clone();
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(len) => {
                    if len > 0 {
                        while buffer.ends_with(&[b'\r']) || buffer.ends_with(&[b'\n']) {
                            buffer.pop();
                        }
                        let record = Record {
                            raw: String::from_utf8_lossy(&buffer).into(),
                            ..Default::default()
                        };
                        let f = tx.send(Some(record)).map(|_| ()).map_err(|_| ());
                        remote.spawn(|_| f);
                    } else {
                        let f = tx.clone().send(None).map(|_| ()).map_err(|_| ());
                        remote.spawn(|_| f);
                        tx.close().ok();
                        break;
                    }
                }
                Err(_) => {
                    tx.close().ok();
                    break;
                }
            }
        }
    });

    Ok(Box::new(
        rx.map_err(|e| format_err!("Channel error: {:?}", e)),
    ))
}

pub fn file_reader<'a>(args: &ArgMatches<'a>, core: &Core) -> Result<RStream, Error> {
    let files = args
        .values_of("input")
        .map(|f| f.map(PathBuf::from).collect::<Vec<PathBuf>>())
        .ok_or_else(|| err_msg("Failed to parse input files"))?;

    let mut streams = Vec::new();
    for f in &files {
        if !f.exists() {
            return Err(format_err!("Cannot open {}", f.display()));
        }

        let file =
            File::open(f).map_err(|e| format_err!("Failed to open {}: {}", f.display(), e))?;

        streams.push(records(file, core)?);
    }

    // The flattened streams emmit None in between - filter those
    // here...
    let mut nones = streams.len();
    let flat = stream::iter_ok::<_, Error>(streams)
        .flatten()
        .filter(move |f| {
            if f.is_some() {
                true
            } else {
                nones -= 1;
                nones == 0
            }
        });
    Ok(Box::new(flat))
}

pub fn stdin_reader(core: &Core) -> Result<RStream, Error> {
    records(Box::new(stdin()), core)
}

pub fn serial_reader<'a>(args: &ArgMatches<'a>, core: &Core) -> Result<RStream, Error> {
    let i = args
        .value_of("input")
        .ok_or_else(|| err_msg("Invalid input value"))?;
    let port = ::serial::open(&i.trim_left_matches("serial://"))?;

    records(port, core)
}

struct LossyLineCodec;

impl Decoder for LossyLineCodec {
    type Item = Record;
    type Error = ::std::io::Error;

    fn decode(
        &mut self,
        buf: &mut BytesMut,
    ) -> ::std::result::Result<Option<Record>, ::std::io::Error> {
        if let Some(n) = buf.as_ref().iter().position(|b| *b == b'\n') {
            let line = buf.split_to(n);
            buf.split_to(1);
            return Ok(Some(Record {
                raw: String::from_utf8_lossy(&line).into_owned(),
                ..Default::default()
            }));
        }

        Ok(None)
    }
}

impl Encoder for LossyLineCodec {
    type Item = Record;
    type Error = ::std::io::Error;

    fn encode(&mut self, _msg: Record, _buf: &mut BytesMut) -> ::std::io::Result<()> {
        unimplemented!();
    }
}

pub fn tcp_reader(addr: &SocketAddr, core: &mut Core) -> Result<RStream, Error> {
    let handle = core.handle();
    let s = core
        .run(TcpStream::connect(addr, &handle))
        .map(|s| Decoder::framed(LossyLineCodec {}, s))
        .map_err(|e| format_err!("Failed to connect: {}", e))?
        .map(Some)
        .map_err(|e| e.into());
    Ok(Box::new(s))
}
