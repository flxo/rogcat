// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use clap::ArgMatches;
use errors::*;
use futures::{Async, Stream, Poll};
use record::Record;
use serial::prelude::*;
use std::fs::File;
use std::io::{BufReader, BufRead, Read};
use std::io::{Stdin, stdin};
use std::path::PathBuf;
use super::Message;
use std::time::Duration;
use nom::{digit, IResult};
use std::str::from_utf8;

pub enum ReadResult {
    Record(Record),
    Done,
}

pub struct LineReader<T>
    where T: Read
{
    reader: BufReader<T>,
}

impl<T: Read> LineReader<T> {
    pub fn new(reader: T) -> LineReader<T> {
        LineReader { reader: BufReader::new(reader) }
    }

    pub fn read(&mut self) -> Result<ReadResult> {
        let mut buffer = Vec::new();
        if self.reader
               .read_until(b'\n', &mut buffer)
               .chain_err(|| "Failed read")? > 0 {
            let line = String::from_utf8(buffer)?.trim().to_string();
            let record = Record { raw: line, ..Default::default() };
            Ok(ReadResult::Record(record))
        } else {
            Ok(ReadResult::Done)
        }
    }
}

pub struct FileReader {
    files: Vec<LineReader<File>>,
}

impl<'a> FileReader {
    pub fn new(args: &ArgMatches<'a>) -> Result<Self> {
        let files = args.values_of("input")
            .map(|f| f.map(PathBuf::from).collect::<Vec<PathBuf>>())
            .ok_or("Failed to parse input files")?;

        // No early return from iteration....
        let mut reader = Vec::new();
        for f in files {
            let file = File::open(f.clone()).chain_err(|| format!("Failed to open {:?}", f))?;
            reader.push(LineReader::new(file));
        }

        Ok(FileReader { files: reader })
    }

    fn read(&mut self) -> Result<ReadResult> {
        let reader = &mut self.files[0];
        reader.read()
    }
}

impl Stream for FileReader {
    type Item = Message;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.read() {
            Ok(v) => {
                match v {
                    ReadResult::Done => {
                        if self.files.len() > 1 {
                            self.files.remove(0);
                            // TODO multi file feature
                            Ok(Async::Ready(Some(Message::Done)))
                        } else {
                            Ok(Async::Ready(Some(Message::Done)))
                        }
                    }
                    ReadResult::Record(r) => Ok(Async::Ready(Some(Message::Record(r)))),
                }
            }
            Err(e) => Err(e),
        }
    }
}

pub struct StdinReader {
    reader: LineReader<Stdin>,
}

impl StdinReader {
    pub fn new() -> StdinReader {
        StdinReader { reader: LineReader::new(stdin()) }
    }
}

impl Stream for StdinReader {
    type Item = Message;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.reader.read().map(|r| match r {
                                   ReadResult::Done => Async::Ready(None),
                                   ReadResult::Record(r) => Async::Ready(Some(Message::Record(r))),
                               })
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
    reader: LineReader<Box<SerialPort>>,
}

impl SerialReader {
    pub fn new(settings: &str) -> Result<Self> {
        let args = Self::parse_serial_arg(settings)?;
        let mut port = ::serial::open(&args.0)?;
        port.configure(&args.1)?;
        port.set_timeout(Duration::from_secs(999999999))?;

        Ok(SerialReader { reader: LineReader::new(Box::new(port)) })
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
    type Item = Message;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.reader.read() {
            Ok(r) => {
                match r {
                    ReadResult::Done => Ok(Async::Ready(None)),
                    ReadResult::Record(r) => Ok(Async::Ready(Some(Message::Record(r)))),
                }
            }

            Err(e) => Err(e),
        }
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

    let s = serial("serial:///dev/ttyUSB0@115200,7O2".as_bytes()).unwrap().1;
    assert_eq!("/dev/ttyUSB0", s.0);
    assert_eq!(::serial::Baud115200, s.1.baud_rate);
    assert_eq!(::serial::Bits7, s.1.char_size);
    assert_eq!(::serial::ParityOdd, s.1.parity);
    assert_eq!(::serial::Stop2, s.1.stop_bits);
}
