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

use crate::record::{Level, Record, Timestamp};
use csv::ReaderBuilder;
use failure::Fail;
use nom::{
    alt, char, complete, do_parse, flat_map, hex_digit, many0, many1, map, named, opt, parse_to,
    peek, rest, space, tag, take, take_until, take_until_either, types::CompleteStr,
};
use serde_json::from_str;
use std::{
    convert::Into,
    io::{Cursor, Read},
};

use time::Tm;

#[derive(Fail, Debug)]
#[fail(display = "{}", _0)]
pub struct ParserError(String);

pub trait FormatParser: Send + Sync {
    fn try_parse_str<'a>(&self, line: &'a str) -> Result<Record, ParserError>;
}

named!(
    level<CompleteStr, Level>,
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

// 2017-03-25 19:11:19.052
named!(
    timestamp<CompleteStr, Tm>,
    do_parse!(
        year: opt!(do_parse!(
            y: flat_map!(peek!(take!(4)), parse_to!(i32)) >> take!(4) >> char!('-') >> (y)
        )) >> month: flat_map!(take!(2), parse_to!(i32))
            >> char!('-')
            >> day: flat_map!(take!(2), parse_to!(i32))
            >> space
            >> hour: flat_map!(take!(2), parse_to!(i32))
            >> char!(':')
            >> minute: flat_map!(take!(2), parse_to!(i32))
            >> char!(':')
            >> second: flat_map!(take!(2), parse_to!(i32))
            >> char!('.')
            >> millisecond: flat_map!(take!(3), parse_to!(i32))
            >> utcoff:
                opt!(complete!(do_parse!(
                    space
                        >> sign: map!(alt!(char!('-') | char!('+')), |c| if c == '-' {
                            -1
                        } else {
                            1
                        })
                        >> utc_off_hours: flat_map!(take!(2), parse_to!(i32))
                        >> utc_off_minutes: flat_map!(take!(2), parse_to!(i32))
                        >> (sign * (utc_off_hours * 60 * 60 + utc_off_minutes * 60))
                )))
            >> (Tm {
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
    printable<CompleteStr, Record>,
    do_parse!(
        timestamp: timestamp
            >> many1!(space)
            >> process: hex_digit
            >> many1!(space)
            >> thread: hex_digit
            >> many1!(space)
            >> level: level
            >> space
            >> tag: take_until!(":")
            >> char!(':')
            >> message: opt!(rest)
            >> (Record {
                timestamp: Some(Timestamp::new(timestamp)),
                level,
                tag: tag.trim().to_owned(),
                process: process.trim().to_owned(),
                thread: thread.trim().to_owned(),
                message: message.unwrap_or(CompleteStr("")).trim().to_owned(),
                ..Default::default()
            })
    )
);

named!(
    mindroid<CompleteStr, Record>,
    alt!(
        // Short format without timestamp
        do_parse!(
            level: level >>
            char!('/') >>
            tag: take_until_either!("(:") >>
            opt!(tag!("(")) >>
            opt!(tag!("0x")) >>
            process: opt!(hex_digit) >>
            opt!(tag!(")")) >>
            tag!(":") >>
            message: opt!(rest) >>
            (
                Record {
                    timestamp: None,
                    level,
                    tag: tag.trim().to_owned(),
                    process: process.map(|s| s.trim()).unwrap_or("").to_owned(),
                    message: message.unwrap_or(CompleteStr("")).trim().to_owned(),
                    ..Default::default()
                }
            )
        ) |
        // Long format with timestamp
        do_parse!(
            timestamp: timestamp >>
            many0!(space) >>
            opt!(tag!("0x")) >>
            process: hex_digit >>
            many1!(space) >>
            level: level >>
            space >>
            tag: take_until!(":") >>
            tag!(":") >>
            message: opt!(rest) >>
            (
                Record {
                    timestamp: Some(Timestamp::new(timestamp)),
                    level,
                    tag: tag.trim().to_owned(),
                    process: process.trim().to_owned(),
                    message: message.unwrap_or(CompleteStr("")).trim().to_string(),
                    ..Default::default()
                }
            )
        )
    )
);

named!(
    bugreport_section<CompleteStr, (String, String)>,
    do_parse!(
        tag: take_until!("(")
            >> char!('(')
            >> msg: take_until!(")")
            >> char!(')')
            >> ((tag.to_string(), msg.to_string()))
    )
);

named!(
    property<CompleteStr, (String, String)>,
    do_parse!(
        char!('[')
            >> prop: take_until!("]")
            >> tag!("]: [")
            >> value: take_until!("]")
            >> char!(']')
            >> ((prop.to_string(), value.to_string()))
    )
);

pub struct DefaultParser;

impl FormatParser for DefaultParser {
    fn try_parse_str<'a>(&self, line: &'a str) -> Result<Record, ParserError> {
        printable(CompleteStr(line))
            .map(|(_, mut v)| {
                v.raw = line.into();
                v
            })
            .map_err(|e| ParserError(format!("{}", e)))
    }
}

pub struct MindroidParser;

impl FormatParser for MindroidParser {
    fn try_parse_str<'a>(&self, line: &'a str) -> Result<Record, ParserError> {
        mindroid(CompleteStr(line))
            .map(|(_, mut v)| {
                v.raw = line.into();
                v
            })
            .map_err(|e| ParserError(format!("{}", e)))
    }
}

pub struct CsvParser;

impl FormatParser for CsvParser {
    fn try_parse_str<'a>(&self, line: &'a str) -> Result<Record, ParserError> {
        let reader = Cursor::new(line).chain(Cursor::new([b'\n']));
        let mut rdr = ReaderBuilder::new().has_headers(false).from_reader(reader);
        if let Some(result) = rdr.deserialize().next() {
            result.map_err(|e| ParserError(format!("{}", e)))
        } else {
            Err(ParserError(format!("Failed to parse csv")))
        }
    }
}

pub struct JsonParser;

impl FormatParser for JsonParser {
    fn try_parse_str<'a>(&self, line: &'a str) -> Result<Record, ParserError> {
        from_str(line).map_err(|e| ParserError(format!("Failed to deserialize json: {}", e)))
    }
}

pub struct GTestParser;

impl FormatParser for GTestParser {
    fn try_parse_str<'a>(&self, line: &'a str) -> Result<Record, ParserError> {
        if line.len() >= 12 {
            let mut chars = line.chars();
            if let Some('[') = chars.next() {
                if let Some(']') = chars.nth(10) {
                    let process = line[1..=10]
                        .trim()
                        .trim_matches('-')
                        .trim_matches('=')
                        .to_owned();
                    Ok(Record {
                        timestamp: None,
                        level: if process == "FAILED" {
                            Level::Error
                        } else {
                            Level::Info
                        },
                        tag: "googletest".to_owned(),
                        process,
                        message: line[12..].trim().to_owned(),
                        ..Default::default()
                    })
                } else {
                    Err(ParserError(String::from(
                        "Failed to parse gtest: Missing closing bracket",
                    )))
                }
            } else {
                Err(ParserError(String::from(
                    "Failed to parse gtest: Missing opening bracket",
                )))
            }
        } else {
            Err(ParserError(String::from(
                "Failed to parse gtest: Message is too short",
            )))
        }
    }
}

pub struct BugReportParser;

impl FormatParser for BugReportParser {
    fn try_parse_str<'a>(&self, line: &'a str) -> Result<Record, ParserError> {
        if line.starts_with('=')
            || line.starts_with('-')
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
                Err(ParserError(String::from("Unparseable")))
            } else if let Ok((_, (tag, value))) = property(CompleteStr(line)) {
                Ok(Record {
                    message: value,
                    tag,
                    raw: line.to_owned(),
                    ..Default::default()
                })
            } else {
                let line = line.trim_matches('=').trim_matches('-').trim();
                match bugreport_section(CompleteStr(line)) {
                    Ok((_, (tag, message))) => Ok(Record {
                        level: Level::Info,
                        message,
                        raw: line.to_owned(),
                        tag,
                        ..Default::default()
                    }),
                    Err(e) => Err(ParserError(format!("{}", e))),
                }
            }
        } else {
            Err(ParserError(String::from("Unparseable")))
        }
    }
}

pub struct Parser {
    parsers: Vec<Box<dyn FormatParser>>,
    last: Option<usize>,
}

impl Default for Parser {
    fn default() -> Self {
        Parser {
            parsers: vec![
                Box::new(DefaultParser),
                Box::new(MindroidParser),
                Box::new(CsvParser),
                Box::new(JsonParser),
                Box::new(GTestParser),
                Box::new(BugReportParser),
            ],
            last: None,
        }
    }
}

impl Parser {
    pub fn new() -> Self {
        Parser {
            parsers: Vec::new(),
            last: None,
        }
    }

    pub fn parse(&mut self, line: &str) -> Record {
        if let Some(last) = self.last {
            let p = &self.parsers[last];
            if let Ok(r) = p.try_parse_str(&line) {
                return r;
            }
        } else {
            for (i, p) in self.parsers.iter().enumerate() {
                if let Ok(r) = p.try_parse_str(&line) {
                    self.last = Some(i);
                    return r;
                }
            }
        }

        // Seems that we cannot parse this record
        // Treat the raw input as message
        Record {
            raw: String::from(line),
            message: String::from(line),
            ..Default::default()
        }
    }
}

#[test]
fn parse_level() {
    assert_eq!(level(CompleteStr("V")).unwrap().1, Level::Verbose);
    assert_eq!(level(CompleteStr("D")).unwrap().1, Level::Debug);
    assert_eq!(level(CompleteStr("I")).unwrap().1, Level::Info);
    assert_eq!(level(CompleteStr("W")).unwrap().1, Level::Warn);
    assert_eq!(level(CompleteStr("E")).unwrap().1, Level::Error);
    assert_eq!(level(CompleteStr("F")).unwrap().1, Level::Fatal);
    assert_eq!(level(CompleteStr("A")).unwrap().1, Level::Assert);
}

#[test]
fn parse_unparseable() {
    let p = DefaultParser {};
    assert!(p.try_parse_str("").is_err());
}

#[test]
fn parse_timestamp() {
    let t = "03-25 19:11:19.052";
    timestamp(CompleteStr(t)).unwrap();

    let t = "2017-03-25 19:11:19.052";
    timestamp(CompleteStr(t)).unwrap();
}

#[test]
fn parse_printable() {
    let t = "03-01 02:19:45.207     1     2 I EXT4-fs (mmcblk3p8): mounted filesystem with \
             ordered data mode. Opts: (null)";
    let p = DefaultParser {};
    let r = p.try_parse_str(t).unwrap();
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
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.level, Level::Debug);
    assert_eq!(r.tag, "/soc/aips-bus@02100000/usdhc@0219c000");
    assert_eq!(r.process, "0");
    assert_eq!(r.thread, "0");
    assert_eq!(r.message, "voltage-ranges unspecified");

    let t = "11-06 13:58:53.582 31359 31420 I GStreamer+amc: 0:00:00.326067533 0xb8ef2a00";
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.level, Level::Info);
    assert_eq!(r.tag, "GStreamer+amc");
    assert_eq!(r.process, "31359");
    assert_eq!(r.thread, "31420");
    assert_eq!(r.message, "0:00:00.326067533 0xb8ef2a00");

    let t = "11-06 13:58:53.582 31359 31420 A GStreamer+amc: 0:00:00.326067533 0xb8ef2a00";
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.level, Level::Assert);
    assert_eq!(r.tag, "GStreamer+amc");
    assert_eq!(r.process, "31359");
    assert_eq!(r.thread, "31420");
    assert_eq!(r.message, "0:00:00.326067533 0xb8ef2a00");

    let t = "03-26 13:17:38.345     0     0 I [114416.534450,0] mdss_dsi_off-:";
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.level, Level::Info);
    assert_eq!(r.tag, "[114416.534450,0] mdss_dsi_off-");
    assert_eq!(r.message, "");
}

#[test]
fn test_parse_mindroid() {
    let t = "I/Runtime: Mindroid runtime system node id: 1";
    let p = MindroidParser {};
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.level, Level::Info);
    assert_eq!(r.tag, "Runtime");
    assert_eq!(r.process, "");
    assert_eq!(r.thread, "");
    assert_eq!(r.message, "Mindroid runtime system node id: 1");

    let t = "D/ServiceManager(000000000000000C): foo bar";
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.level, Level::Debug);
    assert_eq!(r.tag, "ServiceManager");
    assert_eq!(r.process, "000000000000000C");
    assert_eq!(r.thread, "");
    assert_eq!(r.message, "foo bar");

    let t = "D/ServiceManager(0x123): Service MediaPlayer has been created in process main";
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.level, Level::Debug);
    assert_eq!(r.tag, "ServiceManager");
    assert_eq!(r.process, "123");
    assert_eq!(r.thread, "");
    assert_eq!(
        r.message,
        "Service MediaPlayer has been created in process main"
    );

    let t = "D/ServiceManager(0xabc): Service MediaPlayer has been created in process main";
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.process, "abc");

    let t = "2017-03-25 19:11:19.052  0x3b7fe700  D SomeThing: Parsing IPV6 address \
             fd53:7cb8:383:4:0:0:0:68";
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.level, Level::Debug);
    assert_eq!(r.tag, "SomeThing");
    assert_eq!(r.process, "3b7fe700");
    assert_eq!(r.thread, "");
    assert_eq!(r.message, "Parsing IPV6 address fd53:7cb8:383:4:0:0:0:68");

    let t = "2017-03-25 19:11:19.052  0x3b7fe700  D SomeThing:";
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.message, "");
}

#[test]
fn parse_csv_unparseable() {
    let p = CsvParser {};
    assert!(p.try_parse_str("").is_err());
    assert!(p.try_parse_str(",,,").is_err());
}

#[test]
fn test_parse_csv() {
    let t = "07-01 14:13:14.446000000,Sensor:batt_therm:29000 mC,Info,ThermalEngine,225,295,07-01 14:13:14.446   225   295 I ThermalEngine: Sensor:batt_therm:29000 mC";
    let p = CsvParser {};
    let r = p.try_parse_str(t).unwrap();
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
        property(CompleteStr(t)).unwrap().1,
        ("ro.build.tags".to_owned(), "release-keys".to_owned())
    );
}

#[test]
fn test_parse_gtest() {
    let t = "[       OK ] TestName.Test (115 ms)";
    let p = GTestParser {};
    let _r = p.try_parse_str(t).unwrap();
    // assert_eq!(r.level, Level::Info);
    // assert_eq!(r.tag, "ThermalEngine");
    // assert_eq!(r.process, "225");
    // assert_eq!(r.thread, "295");
    // assert_eq!(r.message, "Sensor:batt_therm:29000 mC");
    // assert_eq!(
    //     r.raw,
    //     "07-01 14:13:14.446   225   295 I ThermalEngine: Sensor:batt_therm:29000 mC"
    // );
}
