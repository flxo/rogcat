// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use bytes::BytesMut;
use clap::ArgMatches;
use errors::*;
use futures::sync::mpsc::Receiver;
use futures::sync::mpsc;
use futures::{Async, Future, Sink, Stream, Poll};
use nom::{digit, IResult};
use record::Record;
use serial::prelude::*;
use std::fs::File;
use std::io::stdin;
use std::io::{BufReader, BufRead, Read};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::from_utf8;
use std::str;
use std::thread;
use std::time::Duration;
use super::RStream;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_io::AsyncRead;
use tokio_io::codec::{Encoder, Decoder};

pub struct LineReader {
    rx: Receiver<Result<Option<Record>>>,
}

impl LineReader {
    pub fn new(reader: Box<Read + Send>, core: &Core, head: Option<usize>) -> LineReader {
        let (tx, rx) = mpsc::channel(1);
        let mut reader = BufReader::new(reader);
        let remote = core.remote();

        thread::spawn(move || {
            let mut head = head;
            loop {
                let mut tx = tx.clone();
                let mut buffer = Vec::new();
                if let Some(c) = head {
                    if c == 0 {
                        let f = ::futures::done(Ok(None));
                        remote.spawn(|_| f.then(|res| tx.send(res).map(|_| ()).map_err(|_| ())));
                        break;
                    }
                }
                match reader.read_until(b'\n', &mut buffer) {
                    Ok(len) => {
                        if len > 0 {
                            let raw = String::from_utf8_lossy(&buffer)
                                .into_owned()
                                .trim()
                                .to_owned();
                            let record = Some(Record {
                                raw,
                                ..Default::default()
                            });
                            let f = ::futures::done(Ok(record));
                            remote.spawn(|_| {
                                f.then(|res| tx.send(res).map(|_| ()).map_err(|_| ()))
                            });

                            head = head.map(|c| c - 1);
                        } else {
                            let f = ::futures::done(Ok(None));
                            remote.spawn(|_| {
                                f.then(|res| tx.send(res).map(|_| ()).map_err(|_| ()))
                            });
                            break;
                        }
                    }
                    Err(e) => {
                        let f = ::futures::future::err(e);
                        remote.spawn(|_| f.map_err(|_| ()));
                        tx.close().ok();
                    }
                }
            }
        });

        LineReader { rx }
    }
}

impl Stream for LineReader {
    type Item = Option<Record>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        // This definitly can be done smarter...
        match self.rx.poll() {
            Ok(a) => {
                match a {
                    Async::Ready(b) => {
                        match b {
                            Some(c) => c.map(|v| Async::Ready(Some(v))),
                            None => Ok(Async::Ready(None)),
                        }
                    }
                    Async::NotReady => Ok(Async::NotReady),
                }
            }
            Err(_) => Err("Channel error".into()),
        }
    }
}

#[derive(Default)]
pub struct FileReader {
    files: Vec<LineReader>,
}

impl<'a> FileReader {
    pub fn new(args: &ArgMatches<'a>, core: &Core) -> Result<Self> {
        let file_names = args.values_of("input")
            .map(|f| f.map(PathBuf::from).collect::<Vec<PathBuf>>())
            .ok_or("Failed to parse input files")?;

        let mut files = Vec::new();
        for f in file_names {
            let file = File::open(f.clone()).chain_err(
                || format!("Failed to open {:?}", f),
            )?;
            files.push(LineReader::new(
                Box::new(file),
                core,
                value_t!(args, "head", usize).ok(),
            ));
        }

        Ok(FileReader { files })
    }
}

impl Stream for FileReader {
    type Item = Option<Record>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            let p = self.files[0].poll();
            match p {
                Ok(Async::Ready(None)) => {
                    if self.files.len() > 1 {
                        self.files.remove(0);
                        continue;
                    } else {
                        return p;
                    }
                }
                _ => return p,
            }
        }
    }
}

pub struct StdinReader {
    reader: LineReader,
}

impl<'a> StdinReader {
    pub fn new(args: &ArgMatches<'a>, core: &Core) -> StdinReader {
        StdinReader {
            reader: LineReader::new(Box::new(stdin()), core, value_t!(args, "head", usize).ok()),
        }
    }
}

impl Stream for StdinReader {
    type Item = Option<Record>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.reader.poll()
    }
}

named!(num_usize <usize>,
    map_res!(
        map_res!(
            digit,
            from_utf8
        ),
        str::parse::<usize>
    )
);

named!(baudrate <::serial::BaudRate>,
       map!(num_usize, |b| {
           match b {
                110 => ::serial::Baud110,
                300 => ::serial::Baud300,
                600 => ::serial::Baud600,
                1200 => ::serial::Baud1200,
                2400 => ::serial::Baud2400,
                4800 => ::serial::Baud4800,
                9600 => ::serial::Baud9600,
                19200 => ::serial::Baud19200,
                38400 => ::serial::Baud38400,
                57600 => ::serial::Baud57600,
                115200 => ::serial::Baud115200,
                _ => ::serial::BaudOther(b),
           }
       })
);

named!(char_size <::serial::CharSize>,
   map!(
       alt!(char!('5') | char!('6') | char!('7') | char!('8')),
           |b| {
           match b {
               '5' => ::serial::Bits5,
               '6' => ::serial::Bits6,
               '7' => ::serial::Bits7,
               '8' => ::serial::Bits8,
               _ => panic!("Invalid parser"),
           }
       })
);

named!(parity <::serial::Parity>,
   map!(
       alt!(char!('N') | char!('O') | char!('E')),
           |b| {
           match b {
               'N' => ::serial::ParityNone,
               'O' => ::serial::ParityOdd,
               'E' => ::serial::ParityEven,
               _ => panic!("Invalid parser"),
           }
        })
);

named!(stop_bits <::serial::StopBits>,
   map!(
       alt!(char!('1') | char!('2')),
           |b| {
           match b {
               '1' => ::serial::Stop1,
               '2' => ::serial::Stop2,
               _ => panic!("Invalid parser"),
           }
        })
);

named!(serial <(String, ::serial::PortSettings)>,
       do_parse!(
           tag!("serial://") >>
           port: map_res!(take_until!("@"), from_utf8) >>
           char!('@') >>
           baudrate: baudrate >>
           opt!(complete!(char!(','))) >>
           char_size: opt!(complete!(char_size)) >>
           parity: opt!(complete!(parity)) >>
           stop_bits: opt!(complete!(stop_bits)) >>
           (
               (port.to_owned(),
                ::serial::PortSettings {
                    baud_rate: baudrate,
                    char_size: char_size.unwrap_or(::serial::Bits8),
                    parity: parity.unwrap_or(::serial::ParityNone),
                    stop_bits: stop_bits.unwrap_or(::serial::Stop1),
                    flow_control: ::serial::FlowNone
                })
           )
       )
);

pub struct SerialReader {
    reader: LineReader,
}

impl<'a> SerialReader {
    pub fn new(args: &ArgMatches<'a>, settings: &str, core: &Core) -> Result<Self> {
        let p = Self::parse_serial_arg(settings)?;
        let mut port = ::serial::open(&p.0)?;
        port.configure(&p.1)?;
        port.set_timeout(Duration::from_secs(999999999))?;

        Ok(SerialReader {
            reader: LineReader::new(Box::new(port), core, value_t!(args, "head", usize).ok()),
        })
    }

    pub fn parse_serial_arg(arg: &str) -> Result<(String, ::serial::PortSettings)> {
        match serial(arg.as_bytes()) {
            IResult::Done(_, v) => Ok(v),
            IResult::Error(_) => Err("Failed to parse serial arguments".into()),
            IResult::Incomplete(_) => Err("Not enough data".into()),
        }
    }
}

impl Stream for SerialReader {
    type Item = Option<Record>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.reader.poll()
    }
}

struct LineCodec;

impl Decoder for LineCodec {
    type Item = Record;
    type Error = ::std::io::Error;

    fn decode(
        &mut self,
        buf: &mut BytesMut,
    ) -> ::std::result::Result<Option<Record>, ::std::io::Error> {
        if let Some(n) = buf.as_ref().iter().position(|b| *b == b'\n') {
            let line = buf.split_to(n);
            buf.split_to(1);
            let s = String::from_utf8_lossy(&line).into_owned();
            return Ok(Some(Record {
                raw: s,
                ..Default::default()
            }));
        }

        Ok(None)
    }
}

impl Encoder for LineCodec {
    type Item = Record;
    type Error = ::std::io::Error;

    fn encode(&mut self, _msg: Record, _buf: &mut BytesMut) -> ::std::io::Result<()> {
        unimplemented!();
    }
}

pub struct TcpReader {}

impl<'a> TcpReader {
    pub fn new(_args: &ArgMatches<'a>, addr: &SocketAddr, core: &mut Core) -> Result<RStream> {
        let handle = core.handle();
        let s = core.run(TcpStream::connect(&addr, &handle))
            .chain_err(|| "Failed to connect")?
            .framed(LineCodec)
            .map(|r| Some(r))
            .map_err(|e| e.into());
        Ok(s.boxed())
    }
}

#[test]
fn parse_serial_baudrate() {
    assert_eq!(baudrate("110".as_bytes()).unwrap().1, ::serial::Baud110);
    assert_eq!(baudrate("300".as_bytes()).unwrap().1, ::serial::Baud300);
    assert_eq!(baudrate("600".as_bytes()).unwrap().1, ::serial::Baud600);
    assert_eq!(baudrate("1200".as_bytes()).unwrap().1, ::serial::Baud1200);
    assert_eq!(baudrate("2400".as_bytes()).unwrap().1, ::serial::Baud2400);
    assert_eq!(baudrate("4800".as_bytes()).unwrap().1, ::serial::Baud4800);
    assert_eq!(baudrate("9600".as_bytes()).unwrap().1, ::serial::Baud9600);
    assert_eq!(baudrate("19200".as_bytes()).unwrap().1, ::serial::Baud19200);
    assert_eq!(baudrate("38400".as_bytes()).unwrap().1, ::serial::Baud38400);
    assert_eq!(baudrate("57600".as_bytes()).unwrap().1, ::serial::Baud57600);
    assert_eq!(baudrate("115200".as_bytes()).unwrap().1, ::serial::Baud115200);
    assert_eq!(baudrate("921600".as_bytes()).unwrap().1, ::serial::BaudOther(921600));
}

#[test]
fn parse_serial_char_size() {
    assert_eq!(char_size("5".as_bytes()).unwrap().1, ::serial::Bits5);
    assert_eq!(char_size("6".as_bytes()).unwrap().1, ::serial::Bits6);
    assert_eq!(char_size("7".as_bytes()).unwrap().1, ::serial::Bits7);
    assert_eq!(char_size("8".as_bytes()).unwrap().1, ::serial::Bits8);
}

#[test]
fn parse_serial_parity() {
    assert_eq!(parity("N".as_bytes()).unwrap().1, ::serial::ParityNone);
    assert_eq!(parity("O".as_bytes()).unwrap().1, ::serial::ParityOdd);
    assert_eq!(parity("E".as_bytes()).unwrap().1, ::serial::ParityEven);
}

#[test]
fn parse_serial_stop_bits() {
    assert_eq!(stop_bits("1".as_bytes()).unwrap().1, ::serial::Stop1);
    assert_eq!(stop_bits("2".as_bytes()).unwrap().1, ::serial::Stop2);
}

#[test]
fn parse_serial_port() {
    let s = serial("serial://COM0@115200".as_bytes()).unwrap().1;
    assert_eq!("COM0", s.0);
    assert_eq!(::serial::Baud115200, s.1.baud_rate);
    assert_eq!(::serial::Bits8, s.1.char_size);
    assert_eq!(::serial::ParityNone, s.1.parity);
    assert_eq!(::serial::Stop1, s.1.stop_bits);

    let s = serial("serial:///dev/ttyUSB0@115200,7O2".as_bytes())
        .unwrap()
        .1;
    assert_eq!("/dev/ttyUSB0", s.0);
    assert_eq!(::serial::Baud115200, s.1.baud_rate);
    assert_eq!(::serial::Bits7, s.1.char_size);
    assert_eq!(::serial::ParityOdd, s.1.parity);
    assert_eq!(::serial::Stop2, s.1.stop_bits);
}
