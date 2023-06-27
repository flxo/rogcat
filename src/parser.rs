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
    branch::alt,
    bytes::complete::{tag, take, take_till1, take_until, take_until1, take_while_m_n},
    character::{
        complete::{char, hex_digit1, i32, space0, space1},
        is_digit,
    },
    combinator::{map, opt, peek, rest},
    error::Error,
    multi::{many0, many1},
    IResult,
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
    fn try_parse_str(&self, line: &str) -> Result<Record, ParserError>;
}

fn parse_year(line: &str) -> IResult<&str, i32> {
    //let (line, year) = map(take(4usize), |s: &str| s.parse::<i32>())(line)?;
    let (line, year) = peek_and_parse_i32(line, 4)?;
    let (line, _) = take(4usize)(line)?;
    let (line, _) = char('-')(line)?;
    Ok((line, year))
}

fn peek_and_parse_i32(line: &str, n: usize) -> IResult<&str, i32> {
    let (line, value) = peek(take::<usize, &str, Error<_>>(n))(line)?;
    let value = i32(value)?;
    Ok((line, value.1))
}

fn take_and_parse_i32(line: &str, n: usize) -> IResult<&str, i32> {
    let (line, value) = take::<usize, &str, Error<_>>(n)(line)?;
    let value = i32(value)?;
    Ok((line, value.1))
}

// 2017-03-25 19:11:19.052
// or
// 2017-03-25 19:11:19.052321
fn timestamp(line: &str) -> IResult<&str, Tm> {
    let (line, year) = opt(parse_year)(line)?;
    let (line, month) = take_and_parse_i32(line, 2)?;
    let (line, _) = char('-')(line)?;
    let (line, day) = take_and_parse_i32(line, 2)?;
    let (line, _) = space0(line)?;
    let (line, hour) = take_until1(":")(line)?;
    let hour = i32(hour)?.1;
    let (line, _) = char(':')(line)?;
    let (line, minute) = take_until1(":")(line)?;
    let minute = take_and_parse_i32(minute, 2)?.1;
    let (line, _) = char(':')(line)?;
    let (line, second) = map(take_until1("."), |s: &str| s.parse::<i32>().unwrap_or(0))(line)?;
    let (line, _) = char('.')(line)?;
    let (line, millis) = map(take_while_m_n(3, 3, |c| is_digit(c as u8)), |s| {
        take_and_parse_i32(s, 3).unwrap().1
    })(line)?;
    let (line, micros) = opt(map(take_while_m_n(3, 3, |c| is_digit(c as u8)), |s| {
        take_and_parse_i32(s, 3).unwrap().1
    }))(line)?;
    let (line, sign) = opt(alt((map(char('-'), |_| -1), map(char('+'), |_| 1))))(line)?;
    let utcoff = match sign {
        Some(sign) => {
            let (line, utc_off_hrs) = map(take(2usize), |s: &str| s.parse::<i32>())(line)?;
            let (_line, utc_off_mins) = map(take(2usize), |s: &str| s.parse::<i32>())(line)?;
            sign * (utc_off_hrs.unwrap_or(0) * 60 * 60 + utc_off_mins.unwrap_or(0) * 60)
        }
        None => 0,
    };

    Ok((
        line,
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
            tm_utcoff: utcoff,
            tm_nsec: millis * 1_000_000 + micros.unwrap_or(0) * 1000,
        },
    ))
}

fn level(line: &str) -> IResult<&str, Level> {
    alt((
        map(char('V'), |_| Level::Verbose),
        map(char('D'), |_| Level::Debug),
        map(char('I'), |_| Level::Info),
        map(char('W'), |_| Level::Warn),
        map(char('E'), |_| Level::Error),
        map(char('F'), |_| Level::Fatal),
        map(char('A'), |_| Level::Assert),
    ))(line)
}

fn printable(line: &str) -> IResult<&str, Record> {
    let (line, timestamp) = timestamp(line)?;
    let (line, _) = many0(space1)(line)?;
    let (line, process) = hex_digit1(line)?;
    let (line, _) = many0(space1)(line)?;
    let (line, thread) = hex_digit1(line)?;
    let (line, _) = many0(space1)(line)?;
    let (line, level) = level(line)?;
    let (line, _) = space0(line)?;
    let (line, logtag) = take_until(": ")(line)?;
    let (line, _) = tag(": ")(line)?;
    let (line, message) = opt(rest)(line)?;

    let rec = Record {
        timestamp: Some(Timestamp::new(timestamp)),
        message: message.unwrap_or("").trim().to_owned(),
        level,
        tag: logtag.trim().to_owned(),
        process: process.trim().to_owned(),
        thread: thread.trim().to_owned(),
        ..Default::default()
    };

    Ok((line, rec))
}

fn parse_mindroid_short(line: &str) -> IResult<&str, Record> {
    let (line, level) = level(line)?;
    let (line, _) = char('/')(line)?;
    let (line, logtag) = take_till1(|c| c == '(' || c == ':')(line)?;
    let (line, _) = opt(tag("("))(line)?;
    let (line, _) = opt(tag("0x"))(line)?;
    let (line, process) = opt(hex_digit1)(line)?;
    let (line, _) = opt(tag(")"))(line)?;
    let (line, _) = opt(tag(": "))(line)?;
    let (line, message) = opt(rest)(line)?;
    let rec = Record {
        process: process.unwrap_or("").trim().to_owned(),
        timestamp: None,
        message: message.unwrap_or("").trim().to_owned(),
        level,
        tag: logtag.trim().to_owned(),
        ..Default::default()
    };
    Ok((line, rec))
}

fn parse_mindroid_long(line: &str) -> IResult<&str, Record> {
    let (line, timestamp) = timestamp(line)?;
    let (line, _) = many1(space1)(line)?;
    let (line, _) = opt(tag("0x"))(line)?;
    let (line, process) = hex_digit1(line)?;
    let (line, _) = many1(space1)(line)?;
    let (line, level) = level(line)?;
    let (line, _) = space0(line)?;
    let (line, logtag) = take_until(": ")(line)?;
    let (line, _) = tag(": ")(line)?;
    let (line, message) = opt(rest)(line)?;
    let rec = Record {
        process: process.trim().to_owned(),
        timestamp: Some(Timestamp::new(timestamp)),
        message: message.unwrap_or("").trim().to_owned(),
        level,
        tag: logtag.trim().to_owned(),
        ..Default::default()
    };
    Ok((line, rec))
}

fn parse_mindroid(line: &str) -> IResult<&str, Record> {
    let mindroid = alt((parse_mindroid_short, parse_mindroid_long))(line)?;
    Ok(mindroid)
}

fn bugreport_section(line: &str) -> IResult<&str, (String, String)> {
    let (line, logtag) = take_until("(")(line)?;
    let (line, _) = char('(')(line)?;
    let (line, msg) = take_until(")")(line)?;
    let (line, _) = char(')')(line)?;

    Ok((line, (msg.to_owned(), logtag.to_owned())))
}

fn property(line: &str) -> IResult<&str, (String, String)> {
    let (line, _) = char('[')(line)?;
    let (line, prop) = take_until("]")(line)?;
    let (line, _) = tag("]: [")(line)?;
    let (line, val) = take_until("]")(line)?;
    let (line, _) = char(']')(line)?;
    Ok((line, (prop.to_string(), val.to_string())))
}

pub struct DefaultParser;

impl FormatParser for DefaultParser {
    fn try_parse_str(&self, line: &str) -> Result<Record, ParserError> {
        printable(line)
            .map(|(_, mut v)| {
                v.raw = line.into();
                v
            })
            .map_err(|e| ParserError(format!("{e}")))
    }
}

pub struct MindroidParser;

impl FormatParser for MindroidParser {
    fn try_parse_str(&self, line: &str) -> Result<Record, ParserError> {
        parse_mindroid(line)
            .map(|(_, mut v)| {
                v.raw = line.into();
                v
            })
            .map_err(|e| ParserError(format!("{e}")))
    }
}

pub struct CsvParser;

impl FormatParser for CsvParser {
    fn try_parse_str(&self, line: &str) -> Result<Record, ParserError> {
        let reader = Cursor::new(line).chain(Cursor::new([b'\n']));
        let mut rdr = ReaderBuilder::new().has_headers(false).from_reader(reader);
        if let Some(result) = rdr.deserialize().next() {
            result.map_err(|e| ParserError(format!("{e}")))
        } else {
            Err(ParserError("Failed to parse csv".to_string()))
        }
    }
}

pub struct JsonParser;

impl FormatParser for JsonParser {
    fn try_parse_str(&self, line: &str) -> Result<Record, ParserError> {
        from_str(line).map_err(|e| ParserError(format!("Failed to deserialize json: {e}")))
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
            if let Ok(r) = p.try_parse_str(line) {
                return r;
            }
        }

        for (i, p) in self.parsers.iter().map(Box::as_ref).enumerate() {
            if let Ok(r) = p.try_parse_str(line) {
                self.last = Some(i);
                return r;
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
    assert_eq!(level("V").unwrap().1, Level::Verbose);
    assert_eq!(level("D").unwrap().1, Level::Debug);
    assert_eq!(level("I").unwrap().1, Level::Info);
    assert_eq!(level("W").unwrap().1, Level::Warn);
    assert_eq!(level("E").unwrap().1, Level::Error);
    assert_eq!(level("F").unwrap().1, Level::Fatal);
    assert_eq!(level("A").unwrap().1, Level::Assert);
}
#[test]
fn parser_year() {
    let date_wrong = "00";
    let just_date = "2000";
    let date_with_month = "2000-03";
    let val = opt(parse_year)(date_wrong);
    let val2 = opt(parse_year)(date_with_month);
    let val3 = opt(parse_year)(just_date);
    assert_eq!(Ok(("00", None)), val);
    assert_eq!(Ok(("2000", None)), val3);
    assert_eq!(Ok(("03", Some(2000))), val2);
}

#[test]
fn parse_timestamp() {
    let ts = timestamp("03-25 19:11:19.054211").unwrap();
    assert_eq!(00, ts.1.tm_year);
    assert_eq!(3, ts.1.tm_mon);
    assert_eq!(25, ts.1.tm_mday);
    assert_eq!(19, ts.1.tm_hour);
    assert_eq!(11, ts.1.tm_min);
    assert_eq!(19, ts.1.tm_sec);
    timestamp("03-25 7:11:19.052").unwrap();
    timestamp("2017-03-25 19:11:19.052").unwrap();
    timestamp("2017-03-25 19:11:19.052123").unwrap();
}
#[test]
fn parse_unparseable() {
    let p = DefaultParser {};
    assert!(p.try_parse_str("").is_err());
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
    assert_eq!(
        r.timestamp,
        Some(Timestamp {
            tm: Tm {
                tm_year: 0,
                tm_mon: 11,
                tm_mday: 6,
                tm_hour: 13,
                tm_min: 58,
                tm_sec: 53,
                tm_nsec: 582000000,
                tm_wday: 0,
                tm_yday: 0,
                tm_isdst: 0,
                tm_utcoff: 0,
            }
        })
    );
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

    let t = "03-26 13:17:38.345     0     0 I [114416.534450,0] mdss_dsi_off-: ";
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

    let t = "2017-03-25 19:11:19.052  0x3b7fe700  D SomeThing: ";
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.message, "");
}

/*
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
#[ignore]
fn parse_property() {
    let t = "[ro.build.tags]: [release-keys]";
    assert_eq!(
        property(CompleteStr(t)).unwrap().1,
        ("ro.build.tags".to_owned(), "release-keys".to_owned())
    );
}

#[test]
#[ignore]
fn test_parse_section() {
    let mut p = Parser::default();
    p.parse("------ EVENT LOG (logcat -d -b all) ------");
    let r = p.parse("07-01 14:13:14.446000000,Sensor:batt_therm:29000 mC,Info,ThermalEngine,225,295,07-01 14:13:14.446   225   295 I ThermalEngine: Sensor:batt_therm:29000 mC");
    assert_eq!(r.level, Level::Info);
    assert_eq!(r.tag, "ThermalEngine");
    assert_eq!(r.process, "225");
    assert_eq!(r.thread, "295");
    assert_eq!(r.message, "Sensor:batt_therm:29000 mC");
    assert_eq!(
        r.raw,
        "07-01 14:13:14.446   225   295 I ThermalEngine: Sensor:batt_therm:29000 mC"
    );
}*/
