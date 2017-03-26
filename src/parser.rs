// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use csv::Reader;
use errors::*;
use futures::{future, Future};
use nom::{digit, hex_digit, IResult, rest, space};
use std::str::from_utf8;
use super::record::{Level, Record};
use super::{Message, Node, RFuture};
use time::Tm;

named!(colon <char>, char!(':'));

named!(dot <char>, char!('.'));

named!(num_i32 <i32>,
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
        year: opt!(
            do_parse!(
                y: flat_map!(peek!(take!(4)), num_i32) >>
                take!(4) >>
                char!('-') >>
                (y)
            )
        ) >>
        month: num_i32 >> char!('-') >> day: num_i32 >>
        space >>
        hour: num_i32 >> colon >>
        minute: num_i32 >> colon >>
        second: num_i32 >> dot >>
        millisecond: num_i32 >>
        (
            Tm {
                tm_sec: second,
                tm_min: minute,
                tm_hour: hour,
                tm_mday: day,
                tm_mon: month,
                tm_year: year.unwrap_or(0),
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
        message: map_res!(rest, from_utf8) >>
        (
            Record {
                timestamp: Some(timestamp),
                level: level,
                tag: tag.trim().to_owned(),
                process: process.trim().to_owned(),
                thread: thread.trim().to_owned(),
                message: message.trim().to_owned(),
                ..Default::default()
            }
        )
    )
);

named!(mindroid <Record>,
    alt!(
        // Short format without timestamp
        do_parse!(
            level: level >>
            char!('/') >>
            tag: map_res!(take_until!("("), from_utf8) >>
            char!('(') >>
            process: map_res!(hex_digit, from_utf8) >>
            tag!("): ") >>
            message: map_res!(rest, from_utf8) >>
            (
                Record {
                    timestamp: None,
                    level: level,
                    tag: tag.trim().to_owned(),
                    process: process.trim().to_owned(),
                    message: message.trim().to_owned(),
                    ..Default::default()
                }
            )
        ) |
        // Long format with timestamp
        do_parse!(
            timestamp: timestamp >>
            many0!(space) >>
            process: map_res!(hex_digit, from_utf8) >>
            many1!(space) >>
            level: level >>
            space >>
            tag: map_res!(take_until!(":"), from_utf8) >>
            tag!(": ") >>
            message: map_res!(rest, from_utf8) >>
            (
                Record {
                    timestamp: Some(timestamp),
                    level: level,
                    tag: tag.trim().to_owned(),
                    process: process.trim().to_owned(),
                    message: message.trim().to_owned(),
                    ..Default::default()
                }
            )
        )
    )
);

pub struct Parser {
    last: Option<fn(&str) -> Result<Record>>,
}

impl Parser {
    pub fn new() -> Self {
        Parser { last: None }
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
            if let Some(p) = self.last {
                if let Ok(record) = p(&r.raw) {
                    return future::ok(Message::Record(record)).boxed();
                }
            }

            let record = if let Ok(record) = Self::parse_default(&r.raw) {
                self.last = Some(Self::parse_default);
                record
            } else if let Ok(record) = Self::parse_mindroid(&r.raw) {
                self.last = Some(Self::parse_mindroid);
                record
            } else if let Ok(record) = Self::parse_csv(&r.raw) {
                self.last = Some(Self::parse_csv);
                record
            } else {
                Record {
                    message: r.raw.clone(),
                    raw: r.raw,
                    ..Default::default()
                }
            };

            future::ok(Message::Record(record)).boxed()
        } else {
            future::ok(message).boxed()
        }
    }
}

#[test]
fn parse_level() {
    assert_eq!(level("V".as_bytes()).unwrap().1, Level::Verbose);
    assert_eq!(level("D".as_bytes()).unwrap().1, Level::Debug);
    assert_eq!(level("I".as_bytes()).unwrap().1, Level::Info);
    assert_eq!(level("W".as_bytes()).unwrap().1, Level::Warn);
    assert_eq!(level("E".as_bytes()).unwrap().1, Level::Error);
    assert_eq!(level("F".as_bytes()).unwrap().1, Level::Fatal);
    assert_eq!(level("A".as_bytes()).unwrap().1, Level::Assert);
}

#[test]
fn parse_i32() {
    assert_eq!(num_i32("123".as_bytes()).unwrap().1, 123);
    assert_eq!(num_i32("0".as_bytes()).unwrap().1, 0);
}

#[test]
fn parse_unparseable() {
    assert!(Parser::parse_default("").is_err());
}

#[test]
fn parse_printable() {
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
fn parse_mindroid() {
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

    let t = "2017-03-25 19:11:19.052  3b7fe700  D SomeThing: Parsing IPV6 address \
             fd53:7cb8:383:4:0:0:0:68";
    let r = Parser::parse_mindroid(t).unwrap();
    assert_eq!(r.level, Level::Debug);
    assert_eq!(r.tag, "SomeThing");
    assert_eq!(r.process, "3b7fe700");
    assert_eq!(r.thread, "");
    assert_eq!(r.message, "Parsing IPV6 address fd53:7cb8:383:4:0:0:0:68");
}

#[test]
fn csv_unparseable() {
    assert!(Parser::parse_csv("").is_err());
    assert!(Parser::parse_csv(",,,").is_err());
}

#[test]
fn csv() {
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
