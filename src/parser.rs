// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use csv::ReaderBuilder;
use failure::{err_msg, Error};
use nom::{digit, hex_digit, rest, space, IResult};
use record::{Level, Record, Timestamp};
use serde_json::from_str;
use std::str::from_utf8;
use time::Tm;

named!(colon<char>, char!(':'));

named!(dot<char>, char!('.'));

named!(
    num_i32<i32>,
    map_res!(map_res!(digit, from_utf8), str::parse::<i32>)
);

named!(
    level<Level>,
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

named!(
    timestamp<Tm>,
    do_parse!(
        year:
            opt!(do_parse!(
                y: flat_map!(peek!(take!(4)), num_i32) >> take!(4) >> char!('-') >> (y)
            )) >> month: num_i32 >> char!('-') >> day: num_i32 >> space >> hour: num_i32
            >> colon >> minute: num_i32 >> colon >> second: num_i32 >> dot
            >> millisecond: num_i32
            >> utcoff:
                opt!(complete!(do_parse!(
                    space >> sign: map!(alt!(char!('-') | char!('+')), |c| if c == '-' {
                        -1
                    } else {
                        1
                    }) >> utc_off_hours: flat_map!(take!(2), num_i32)
                        >> utc_off_minutes: flat_map!(take!(2), num_i32)
                        >> (sign * (utc_off_hours * 60 * 60 + utc_off_minutes * 60))
                ))) >> (Tm {
            tm_sec: second,
            tm_min: minute,
            tm_hour: hour,
            tm_mday: day,
            tm_mon: month - 1,
            tm_year: year.unwrap_or(0),
            tm_wday: 0,
            tm_yday: 0,
            tm_isdst: 0,
            tm_utcoff: utcoff.unwrap_or(0),
            tm_nsec: millisecond * 1_000_000,
        })
    )
);

named!(
    printable<Record>,
    do_parse!(
        timestamp: timestamp >> many1!(space) >> process: map_res!(hex_digit, from_utf8)
            >> many1!(space) >> thread: map_res!(hex_digit, from_utf8) >> many1!(space)
            >> level: level >> space >> tag: map_res!(take_until!(":"), from_utf8)
            >> char!(':') >> message: opt!(map_res!(rest, from_utf8)) >> (Record {
            timestamp: Some(Timestamp::new(timestamp)),
            level,
            tag: tag.trim().to_owned(),
            process: process.trim().to_owned(),
            thread: thread.trim().to_owned(),
            message: message.unwrap_or("").trim().to_owned(),
            ..Default::default()
        })
    )
);

named!(
    mindroid<Record>,
    alt!(
        // Short format without timestamp
        do_parse!(
            level: level >>
            char!('/') >>
            tag: map_res!(take_until!("("), from_utf8) >>
            tag!("(0x") >>
            process: map_res!(hex_digit, from_utf8) >>
            tag!("):") >>
            message: opt!(map_res!(rest, from_utf8)) >>
            (
                Record {
                    timestamp: None,
                    level,
                    tag: tag.trim().to_owned(),
                    process: process.trim().to_owned(),
                    message: message.unwrap_or("").trim().to_owned(),
                    ..Default::default()
                }
            )
        ) |
        // Long format with timestamp
        do_parse!(
            timestamp: timestamp >>
            many0!(space) >>
            tag!("0x") >>
            process: map_res!(hex_digit, from_utf8) >>
            many1!(space) >>
            level: level >>
            space >>
            tag: map_res!(take_until!(":"), from_utf8) >>
            tag!(":") >>
            message: opt!(map_res!(rest, from_utf8)) >>
            (
                Record {
                    timestamp: Some(Timestamp::new(timestamp)),
                    level,
                    tag: tag.trim().to_owned(),
                    process: process.trim().to_owned(),
                    message: message.unwrap_or("").trim().to_owned(),
                    ..Default::default()
                }
            )
        )
    )
);

named!(
    bugreport_section<(String, String)>,
    do_parse!(
        tag: map_res!(take_until!("("), from_utf8) >> char!('(')
            >> msg: map_res!(take_until!(")"), from_utf8) >> char!(')')
            >> ((tag.to_owned(), msg.to_owned()))
    )
);

named!(
    property<(String, String)>,
    do_parse!(
        char!('[') >> prop: map_res!(take_until!("]"), from_utf8) >> tag!("]: [")
            >> value: map_res!(take_until!("]"), from_utf8) >> char!(']')
            >> ((prop.to_owned(), value.to_owned()))
    )
);

type Parse = fn(&str) -> Result<Record, Error>;

pub struct Parser {
    last: Option<Parse>,
}

impl Parser {
    pub fn new() -> Self {
        Parser { last: None }
    }

    fn parse_default(line: &str) -> Result<Record, Error> {
        match printable(line.as_bytes()) {
            IResult::Done(_, mut v) => {
                v.raw = line.to_owned();
                Ok(v)
            }
            IResult::Error(e) => Err(format_err!("{}", e)),
            IResult::Incomplete(_) => Err(err_msg("Not enough data")),
        }
    }

    fn parse_mindroid(line: &str) -> Result<Record, Error> {
        match mindroid(line.as_bytes()) {
            IResult::Done(_, mut v) => {
                v.raw = line.to_owned();
                Ok(v)
            }
            IResult::Error(e) => Err(format_err!("{}", e)),
            IResult::Incomplete(_) => Err(err_msg("Not enough data")),
        }
    }

    fn parse_csv(line: &str) -> Result<Record, Error> {
        let mut line = line.to_owned();
        line.push('\n');
        let mut rdr = ReaderBuilder::new()
            .has_headers(false)
            .from_reader(line.as_bytes());
        if let Some(result) = rdr.deserialize().next() {
            return result.map_err(|e| e.into());
        }
        Err(err_msg("Failed to parse csv"))
    }

    fn parse_json(line: &str) -> Result<Record, Error> {
        from_str(line).map_err(|e| format_err!("Failed to deserialize json: {}", e))
    }

    fn parse_gtest(line: &str) -> Result<Record, Error> {
        if line.len() >= 12 {
            let mut chars = line.chars();
            if let Some('[') = chars.next() {
                if let Some(']') = chars.skip(10).next() {
                    Ok(Record {
                        timestamp: None,
                        level: Level::Info,
                        tag: "Test".to_owned(),
                        process: line[1..11].trim().trim_matches('-').trim_matches('=').to_owned(),
                        message: line[12..].trim().to_owned(),
                        ..Default::default()
                    })
                } else {
                    Err(err_msg("Failed to parse gtest"))
                }
            } else {
                Err(err_msg("Failed to parse gtest"))
            }
        } else {
            Err(err_msg("Failed to parse gtest"))
        }
    }

    fn parse_bugreport(line: &str) -> Result<Record, Error> {
        if line.starts_with('=') || line.starts_with('-')
            || (line.starts_with('[') && line.ends_with(']'))
        {
            if line.chars().all(|c| c == '=') {
                Ok(Record {
                    level: Level::Info,
                    message: line.to_owned(),
                    raw: line.to_owned(),
                    ..Default::default()
                })
            } else if line.starts_with("== ") {
                Ok(Record {
                    level: Level::Info,
                    message: line[3..].to_owned(),
                    raw: line.to_owned(),
                    tag: line[3..].to_owned(),
                    ..Default::default()
                })
            } else if line.is_empty() {
                Err(err_msg("Unparseable"))
            } else if let IResult::Done(_, (prop, value)) = property(line.as_bytes()) {
                Ok(Record {
                    message: value,
                    tag: prop,
                    raw: line.to_owned(),
                    ..Default::default()
                })
            } else {
                let line = line.trim_matches('=').trim_matches('-').trim();
                match bugreport_section(line.as_bytes()) {
                    IResult::Done(_, (tag, message)) => Ok(Record {
                        level: Level::Info,
                        message,
                        raw: line.to_owned(),
                        tag,
                        ..Default::default()
                    }),
                    IResult::Error(e) => Err(format_err!("{}", e)),
                    IResult::Incomplete(_) => Err(err_msg("Not enough data")),
                }
            }
        } else {
            Err(err_msg("Unparseable"))
        }
    }

    pub fn process(&mut self, record: Option<Record>) -> Option<Record> {
        record.map(|mut record| {
            macro_rules! try_parse {
                ($p:expr) => {
                    if let Ok(r) = $p(&record.raw) {
                        self.last = Some($p);
                        return r;
                    }
                };
            }

            if let Some(p) = self.last {
                try_parse!(p);
            }
            try_parse!(Self::parse_default);
            try_parse!(Self::parse_mindroid);
            try_parse!(Self::parse_csv);
            try_parse!(Self::parse_json);
            try_parse!(Self::parse_gtest);
            try_parse!(Self::parse_bugreport);

            // Seems that we cannot parse this record
            // Treat the raw input as message
            record.message = record.raw.clone();
            record
        })
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
    assert_eq!(
        r.message,
        "mounted filesystem with ordered data mode. Opts: (null)"
    );

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

    let t = "03-26 13:17:38.345     0     0 I [114416.534450,0] mdss_dsi_off-:";
    let r = Parser::parse_default(t).unwrap();
    assert_eq!(r.level, Level::Info);
    assert_eq!(r.tag, "[114416.534450,0] mdss_dsi_off-");
    assert_eq!(r.message, "");
}

#[test]
fn parse_mindroid() {
    let t = "D/ServiceManager(0x123): Service MediaPlayer has been created in process main";
    let r = Parser::parse_mindroid(t).unwrap();
    assert_eq!(r.level, Level::Debug);
    assert_eq!(r.tag, "ServiceManager");
    assert_eq!(r.process, "123");
    assert_eq!(r.thread, "");
    assert_eq!(
        r.message,
        "Service MediaPlayer has been created in process main"
    );

    let t = "D/ServiceManager(0xabc): Service MediaPlayer has been created in process main";
    let r = Parser::parse_mindroid(t).unwrap();
    assert_eq!(r.process, "abc");

    let t = "2017-03-25 19:11:19.052  0x3b7fe700  D SomeThing: Parsing IPV6 address \
             fd53:7cb8:383:4:0:0:0:68";
    let r = Parser::parse_mindroid(t).unwrap();
    assert_eq!(r.level, Level::Debug);
    assert_eq!(r.tag, "SomeThing");
    assert_eq!(r.process, "3b7fe700");
    assert_eq!(r.thread, "");
    assert_eq!(r.message, "Parsing IPV6 address fd53:7cb8:383:4:0:0:0:68");

    let t = "2017-03-25 19:11:19.052  0x3b7fe700  D SomeThing:";
    let r = Parser::parse_mindroid(t).unwrap();
    assert_eq!(r.message, "");
}

#[test]
fn parse_csv_unparseable() {
    assert!(Parser::parse_csv("").is_err());
    assert!(Parser::parse_csv(",,,").is_err());
}

#[test]
fn parse_csv() {
    let t = "07-01 14:13:14.446000000,Sensor:batt_therm:29000 mC,Info,ThermalEngine,225,295,07-01 14:13:14.446   225   295 I ThermalEngine: Sensor:batt_therm:29000 mC";
    let r = Parser::parse_csv(t).unwrap();
    assert_eq!(r.level, Level::Info);
    assert_eq!(r.tag, "ThermalEngine");
    assert_eq!(r.process, "225");
    assert_eq!(r.thread, "295");
    assert_eq!(r.message, "Sensor:batt_therm:29000 mC");
    assert_eq!(
        r.raw,
        "07-01 14:13:14.446   225   295 I ThermalEngine: Sensor:batt_therm:29000 mC"
    );
}

#[test]
fn parse_property() {
    let t = "[ro.build.tags]: [release-keys]";
    assert_eq!(
        property(t.as_bytes()).unwrap().1,
        ("ro.build.tags".to_owned(), "release-keys".to_owned())
    );
}
