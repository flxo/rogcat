// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use bytes::BytesMut;
use clap::ArgMatches;
use errors::*;
use futures::sync::mpsc;
use futures::{stream, Future, Sink, Stream};
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
use std::u64;
use std::thread;
use std::time::Duration;
use super::RStream;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_io::AsyncRead;
use tokio_io::codec::{Encoder, Decoder};
use utils::trim_cr_nl;

fn records<T: Read + Send + Sized + 'static>(reader: T, core: &Core) -> Result<RStream> {
    let (tx, rx) = mpsc::channel(1);
    let mut reader = BufReader::new(reader);
    let remote = core.remote();

    thread::spawn(move || loop {
        let mut tx = tx.clone();
        let mut buffer = Vec::new();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(len) => {
                if len > 0 {
                    let raw = String::from_utf8_lossy(&buffer);
                    let record = Record {
                        raw: trim_cr_nl(&raw),
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
    });

    Ok(Box::new(rx.map_err(|_| "Channel error".into())))
}

pub fn file_reader<'a>(args: &ArgMatches<'a>, core: &Core) -> Result<RStream> {
    let files = args.values_of("input")
        .map(|f| f.map(PathBuf::from).collect::<Vec<PathBuf>>())
        .ok_or("Failed to parse input files")?;

    let mut streams = Vec::new();
    for f in &files {
        if !f.exists() {
            return Err(format!("Cannot open {}", f.display()).into());
        }

        let file = File::open(f).chain_err(
            || format!("Failed to open {}", f.display()),
        )?;

        streams.push(records(file, core)?);
    }

    // The flattened streams emmit None in between - filter those
    // here...
    let mut nones = streams.len();
    let flat = stream::iter_ok::<_, Error>(streams).flatten().filter(
        move |f| {
            if f.is_some() {
                true
            } else {
                nones -= 1;
                nones == 0
            }
        },
    );
    Ok(Box::new(flat))
}

pub fn stdin_reader(core: &Core) -> Result<RStream> {
    records(Box::new(stdin()), core)
}

pub fn serial_reader<'a>(args: &ArgMatches<'a>, core: &Core) -> Result<RStream> {
    let i = args.value_of("input").ok_or("Invalid input value")?;
    let p = match serial(i.as_bytes()) {
        IResult::Done(_, v) => v,
        IResult::Error(_) => return Err("Failed to parse serial port settings".into()),
        IResult::Incomplete(_) => return Err("Serial port settings are incomplete".into()),
    };
    let mut port = ::serial::open(&p.0)?;
    port.configure(&p.1)?;
    port.set_timeout(Duration::from_secs(u64::MAX))?;

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

pub fn tcp_reader(addr: &SocketAddr, core: &mut Core) -> Result<RStream> {
    let handle = core.handle();
    let s = core.run(TcpStream::connect(addr, &handle))
        .chain_err(|| "Failed to connect")?
        .framed(LossyLineCodec)
        .map(Some)
        .map_err(|e| e.into());
    Ok(Box::new(s))
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
                19_200 => ::serial::Baud19200,
                38_400 => ::serial::Baud38400,
                57_600 => ::serial::Baud57600,
                115_200 => ::serial::Baud115200,
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
