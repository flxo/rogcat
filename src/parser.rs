// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use csv::Reader;
use errors::*;
use futures::{future, Future};
use nom::{digit, IResult, hex_digit, space, anychar};
use std::str::from_utf8;
use super::record::{Level, Record};
use super::{Message, Node, RFuture};
use time::Tm;

named!(colon <char>, char!(':'));

named!(dot <char>, char!('.'));

named!(i32 <i32>,
    map_res!(
        map_res!(
            digit,
            from_utf8
        ),
        str::parse::<i32>
    )
);

named!(level <Level>,
       alt!(
            char!('V') => { |_| Level::Verbose }
          | char!('D') => { |_| Level::Debug }
          | char!('I') => { |_| Level::Info }
          | char!('W') => { |_| Level::Warn }
          | char!('E') => { |_| Level::Error }
          | char!('F') => { |_| Level::Fatal }
          | char!('A') => { |_| Level::Assert }
      )
);

named!(timestamp <Tm>,
    do_parse!(
        month: i32 >> char!('-') >> day: i32 >>
        space >>
        hour: i32 >> colon >>
        minute: i32 >> colon >>
        second: i32 >> dot >>
        millisecond: i32 >>
        (
            Tm {
                tm_sec: second,
                tm_min: minute,
                tm_hour: hour,
                tm_mday: day,
                tm_mon: month,
                tm_year: 0,
                tm_wday: 0,
                tm_yday: 0,
                tm_isdst: 0,
                tm_utcoff: 0,
                tm_nsec: millisecond * 1000_000,
            }
        )
    )
);

named!(printable <Record>,
    do_parse!(
        timestamp: timestamp >>
        many1!(space) >>
        process: map_res!(hex_digit, from_utf8) >>
        many1!(space) >>
        thread: map_res!(hex_digit, from_utf8) >>
        many1!(space) >>
        level: level >>
        space >>
        tag: map_res!(take_until!(":"), from_utf8) >>
        tag!(": ") >>
        message: many0!(anychar) >>
        (
            Record {
                timestamp: Some(timestamp),
                level: level,
                tag: tag.trim().to_owned(),
                process: process.trim().to_owned(),
                thread: thread.trim().to_owned(),
                message: message.into_iter().collect::<String>().trim().to_owned(),
                ..Default::default()
            }
        )
    )
);

// TODO: Extend with timestamp format
named!(mindroid <Record>,
    do_parse!(
        level: level >>
        char!('/') >>
        tag: map_res!(take_until!("("), from_utf8) >>
        char!('(') >>
        process: map_res!(hex_digit, from_utf8) >>
        tag!("): ") >>
        message: many0!(anychar) >>
        (
            Record {
                timestamp: None,
                level: level,
                tag: tag.trim().to_owned(),
                process: process.trim().to_owned(),
                message: message.into_iter().collect::<String>().trim().to_owned(),
                ..Default::default()
            }
        )
    )
);

pub struct Parser {}

impl Parser {
    pub fn new() -> Self {
        Parser {}
    }

    fn parse_timestamp(line: &str) -> Result<Option<Tm>> {
        match timestamp(line.as_bytes()) {
            IResult::Done(_, v) => Ok(Some(v)),
            IResult::Error(_) => Err("Failed to parse".into()),
            IResult::Incomplete(_) => Err("Not enough data".into()),
        }
    }

    fn parse_default(line: &str) -> Result<Record> {
        match printable(line.as_bytes()) {
            IResult::Done(_, mut v) => {
                v.raw = line.to_owned();
                Ok(v)
            }
            IResult::Error(_) => Err("Failed to parse".into()),
            IResult::Incomplete(_) => Err("Not enough data".into()),
        }
    }

    fn parse_mindroid(line: &str) -> Result<Record> {
        match mindroid(line.as_bytes()) {
            IResult::Done(_, mut v) => {
                v.raw = line.to_owned();
                Ok(v)
            }
            IResult::Error(_) => Err("Failed to parse".into()),
            IResult::Incomplete(_) => Err("Not enough data".into()),
        }
    }

    fn parse_csv(line: &str) -> Result<Record> {
        type Row = (String, String, String, String, String, String);

        let mut reader = Reader::from_string(line).has_headers(false);
        let rows: Vec<Row> = reader.decode().collect::<::csv::Result<Vec<Row>>>()?;
        let row = &rows.first().ok_or("Failed to parse CSV")?;
        Ok(Record {
            timestamp: Self::parse_timestamp(&row.0)?,
            level: Level::from(row.4.as_str()),
            tag: row.1.clone(),
            process: row.2.clone(),
            thread: row.3.clone(),
            message: row.5.clone(),
            raw: line.to_owned(),
        })
    }
}

impl Node for Parser {
    type Input = Message;

    fn process(&mut self, message: Message) -> RFuture {
        if let Message::Record(r) = message {

            if let Ok(r) = Self::parse_default(&r.raw) {
                return future::ok(Message::Record(r)).boxed();
            }

            if let Ok(r) = Self::parse_mindroid(&r.raw) {
                return future::ok(Message::Record(r)).boxed();
            }

            if let Ok(r) = Self::parse_csv(&r.raw) {
                return future::ok(Message::Record(r)).boxed();
            }

            let r = Record {
                message: r.raw.clone(),
                raw: r.raw,
                ..Default::default()
            };
            future::ok(Message::Record(r)).boxed()
        } else {
            future::ok(message).boxed()
        }
    }
}

#[test]
fn test_level() {
    assert_eq!(level("V".as_bytes()).unwrap().1, Level::Verbose);
    assert_eq!(level("D".as_bytes()).unwrap().1, Level::Debug);
    assert_eq!(level("I".as_bytes()).unwrap().1, Level::Info);
    assert_eq!(level("W".as_bytes()).unwrap().1, Level::Warn);
    assert_eq!(level("E".as_bytes()).unwrap().1, Level::Error);
    assert_eq!(level("F".as_bytes()).unwrap().1, Level::Fatal);
    assert_eq!(level("A".as_bytes()).unwrap().1, Level::Assert);
}

#[test]
fn test_i32() {
    assert_eq!(i32("123".as_bytes()).unwrap().1, 123);
    assert_eq!(i32("0".as_bytes()).unwrap().1, 0);
}

#[test]
fn test_unparseable() {
    assert!(Parser::parse_default("").is_err());
}

#[test]
fn test_printable() {
    let t = "03-01 02:19:45.207     1     2 I EXT4-fs (mmcblk3p8): mounted filesystem with \
             ordered data mode. Opts: (null)";
    let r = Parser::parse_default(t).unwrap();
    assert_eq!(r.level, Level::Info);
    assert_eq!(r.tag, "EXT4-fs (mmcblk3p8)");
    assert_eq!(r.process, "1");
    assert_eq!(r.thread, "2");
    assert_eq!(r.message, "mounted filesystem with ordered data mode. Opts: (null)");

    let t = "03-01 02:19:42.868     0     0 D /soc/aips-bus@02100000/usdhc@0219c000: \
             voltage-ranges unspecified";
    let r = Parser::parse_default(t).unwrap();
    assert_eq!(r.level, Level::Debug);
    assert_eq!(r.tag, "/soc/aips-bus@02100000/usdhc@0219c000");
    assert_eq!(r.process, "0");
    assert_eq!(r.thread, "0");
    assert_eq!(r.message, "voltage-ranges unspecified");

    let t = "11-06 13:58:53.582 31359 31420 I GStreamer+amc: 0:00:00.326067533 0xb8ef2a00";
    let r = Parser::parse_default(t).unwrap();
    assert_eq!(r.level, Level::Info);
    assert_eq!(r.tag, "GStreamer+amc");
    assert_eq!(r.process, "31359");
    assert_eq!(r.thread, "31420");
    assert_eq!(r.message, "0:00:00.326067533 0xb8ef2a00");

    let t = "11-06 13:58:53.582 31359 31420 A GStreamer+amc: 0:00:00.326067533 0xb8ef2a00";
    let r = Parser::parse_default(t).unwrap();
    assert_eq!(r.level, Level::Assert);
    assert_eq!(r.tag, "GStreamer+amc");
    assert_eq!(r.process, "31359");
    assert_eq!(r.thread, "31420");
    assert_eq!(r.message, "0:00:00.326067533 0xb8ef2a00");
}

#[test]
fn test_mindroid() {
    let t = "D/ServiceManager(123): Service MediaPlayer has been created in process main";
    let r = Parser::parse_mindroid(t).unwrap();
    assert_eq!(r.level, Level::Debug);
    assert_eq!(r.tag, "ServiceManager");
    assert_eq!(r.process, "123");
    assert_eq!(r.thread, "");
    assert_eq!(r.message, "Service MediaPlayer has been created in process main");

    let t = "D/ServiceManager(abc): Service MediaPlayer has been created in process main";
    let r = Parser::parse_mindroid(t).unwrap();
    assert_eq!(r.process, "abc");
}

#[test]
fn test_csv_unparseable() {
    assert!(Parser::parse_csv("").is_err());
    assert!(Parser::parse_csv(",,,").is_err());
}

#[test]
fn test_csv() {
    let t = "11-06 13:58:53.582,GStreamer+amc,31359,31420,A,0:00:00.326067533 0xb8ef2a00";
    let r = Parser::parse_csv(t).unwrap();
    assert_eq!(r.level, Level::Assert);
    assert_eq!(r.tag, "GStreamer+amc");
    assert_eq!(r.process, "31359");
    assert_eq!(r.thread, "31420");
    assert_eq!(r.message, "0:00:00.326067533 0xb8ef2a00");

    let t = "11-06 13:58:53.582,GStreamer+amc,31359,31420,A,0:00:00.326067533 0xb8ef2a00";
    let r = Parser::parse_csv(t).unwrap();
    assert_eq!(r.level, Level::Assert);
    assert_eq!(r.tag, "GStreamer+amc");
    assert_eq!(r.process, "31359");
    assert_eq!(r.thread, "31420");
    assert_eq!(r.message, "0:00:00.326067533 0xb8ef2a00");
}
