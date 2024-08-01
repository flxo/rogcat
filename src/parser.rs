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

use serde::{Deserialize, Serialize};
use serde_json::from_str;
use std::io::{Cursor, Read};

use time::Tm;

#[derive(Fail, Debug)]
#[fail(display = "{}", _0)]
pub struct ParserError(String);

trait FormatParser: Send + Sync {
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
        tags: vec![logtag.trim().to_owned()],
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
        tags: vec![logtag.trim().to_owned()],
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
        tags: vec![logtag.trim().to_owned()],
        ..Default::default()
    };
    Ok((line, rec))
}

fn parse_mindroid(line: &str) -> IResult<&str, Record> {
    let mindroid = alt((parse_mindroid_short, parse_mindroid_long))(line)?;
    Ok(mindroid)
}

pub fn bugreport_section(line: &str) -> IResult<&str, (String, String)> {
    let (line, logtag) = take_until("(")(line)?;
    let (line, _) = char('(')(line)?;
    let (line, msg) = take_until(")")(line)?;
    let (line, _) = char(')')(line)?;

    Ok((line, (logtag.to_string(), msg.to_string())))
}

pub fn property(line: &str) -> IResult<&str, (String, String)> {
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
            .map(|(_, record)| record)
            .map_err(|e| ParserError(format!("{e}")))
    }
}

pub struct MindroidParser;

impl FormatParser for MindroidParser {
    fn try_parse_str(&self, line: &str) -> Result<Record, ParserError> {
        parse_mindroid(line)
            .map(|(_, record)| record)
            .map_err(|e| ParserError(format!("{e}")))
    }
}

pub struct CsvParser;

impl FormatParser for CsvParser {
    fn try_parse_str(&self, line: &str) -> Result<Record, ParserError> {
        // Shadow struct with non separate tag
        #[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
        pub struct CsvRecord {
            timestamp: Option<Timestamp>,
            message: String,
            level: Level,
            tag: String,
            process: String,
            thread: String,
            raw: String,
        }
        let reader = Cursor::new(line).chain(Cursor::new([b'\n']));
        let mut rdr = ReaderBuilder::new().has_headers(false).from_reader(reader);
        if let Some(result) = rdr.deserialize::<CsvRecord>().next() {
            let record = result.map_err(|e| ParserError(format!("{e}")))?;
            let CsvRecord {
                timestamp,
                message,
                level,
                tag,
                process,
                thread,
                raw,
            } = record;
            let record = Record {
                timestamp,
                message,
                level,
                tags: tag.split(',').map(|s| s.trim().to_owned()).collect(),
                process,
                thread,
                raw,
            };
            Ok(record)
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

// [seconds][pid][tid][tags] LEVEL: message
// [01086.023158][boot-drivers:dev][driver,platform_bus] INFO: [platform-bus.cc(292)] Boot Item ZBI_TYPE_SERIAL_NUMBER not found
fn parse_fuchsia(line: &str) -> IResult<&str, Record> {
    // Timestamp
    let (rest, _) = char('[')(line)?;
    let (rest, timestamp) = take_until1("]")(rest)?;
    let (rest, _) = char(']')(rest)?;
    let timestamp = timestamp.parse::<f64>().map(Timestamp::from_secs).ok();

    // Process
    let (rest, _) = char('[')(rest)?;
    let (rest, process) = take_until1("]")(rest)?;
    let process = process.to_string();
    let (rest, _) = char(']')(rest)?;

    // // Tid
    // let (rest, thread) = if rest.starts_with('[') {
    //     let (rest, _) = char('[')(rest)?;
    //     let (rest, thread) = take_until1("]")(rest)?;
    //     let (rest, _) = char(']')(rest)?;
    //     (rest, thread.to_string())
    // } else {
    //     (rest, String::new())
    // };

    // Tags
    let (rest, tags) = if rest.starts_with('[') {
        let (rest, _) = char('[')(rest)?;
        let (rest, tags) = take_until1("]")(rest)?;
        let (rest, _) = char(']')(rest)?;
        let mut tags = tags
            .split(',')
            .map(|s| s.trim().to_owned())
            .collect::<Vec<_>>();
        tags.sort();
        (rest, tags)
    } else {
        (rest, vec![])
    };

    let (rest, _) = char(' ')(rest)?;

    // Level
    let (rest, level) = take_until1(":")(rest)?;
    let (rest, _) = char(':')(rest)?;
    let level = match level {
        "TRACE" => Level::Trace,
        "DEBUG" => Level::Debug,
        "INFO" => Level::Info,
        "WARN" => Level::Warn,
        "ERROR" => Level::Error,
        "FATAL" => Level::Fatal,
        _ => unreachable!("unimplemented level: {}", level),
    };

    let (_, message) = nom::combinator::rest(rest)?;

    let record = Record {
        timestamp,
        message: message.trim().to_owned(),
        level,
        tags,
        process,
        ..Default::default()
    };

    Ok(("", record))
}

#[derive(Default)]
pub struct FuchsiaParser;

impl FormatParser for FuchsiaParser {
    fn try_parse_str(&self, line: &str) -> Result<Record, ParserError> {
        parse_fuchsia(line)
            .map(|(_, record)| record)
            .map_err(|e| ParserError(format!("{e}")))
    }
}

pub struct Parser(Vec<Box<dyn FormatParser>>);

impl Default for Parser {
    fn default() -> Self {
        Parser(vec![
            Box::new(DefaultParser),
            Box::new(MindroidParser),
            Box::new(CsvParser),
            Box::new(JsonParser),
            Box::new(FuchsiaParser),
        ])
    }
}

impl Parser {
    pub fn parse(&mut self, raw: String) -> Record {
        for (index, parser) in self.0.iter().map(Box::as_ref).enumerate() {
            if let Ok(mut record) = parser.try_parse_str(raw.as_str()) {
                if index > 0 {
                    self.0.swap(index, index - 1);
                }
                record.raw = raw;
                return record;
            }
        }

        // Seems that we cannot parse this record
        // Treat the raw input as message
        Record {
            message: raw.clone(),
            raw,
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
    assert_eq!(r.tags, vec!("EXT4-fs (mmcblk3p8)"));
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
    assert_eq!(r.tags, vec!("/soc/aips-bus@02100000/usdhc@0219c000"));
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
    assert_eq!(r.tags, vec!("GStreamer+amc"));
    assert_eq!(r.process, "31359");
    assert_eq!(r.thread, "31420");
    assert_eq!(r.message, "0:00:00.326067533 0xb8ef2a00");

    let t = "11-06 13:58:53.582 31359 31420 A GStreamer+amc: 0:00:00.326067533 0xb8ef2a00";
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.level, Level::Assert);
    assert_eq!(r.tags, vec!("GStreamer+amc"));
    assert_eq!(r.process, "31359");
    assert_eq!(r.thread, "31420");
    assert_eq!(r.message, "0:00:00.326067533 0xb8ef2a00");

    let t = "03-26 13:17:38.345     0     0 I [114416.534450,0] mdss_dsi_off-: ";
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.level, Level::Info);
    assert_eq!(r.tags, vec!("[114416.534450,0] mdss_dsi_off-"));
    assert_eq!(r.message, "");
}

#[test]
fn test_parse_mindroid() {
    let t = "I/Runtime: Mindroid runtime system node id: 1";
    let p = MindroidParser {};
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.level, Level::Info);
    assert_eq!(r.tags, vec!("Runtime"));
    assert_eq!(r.process, "");
    assert_eq!(r.thread, "");
    assert_eq!(r.message, "Mindroid runtime system node id: 1");

    let t = "D/ServiceManager(000000000000000C): foo bar";
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.level, Level::Debug);
    assert_eq!(r.tags, vec!("ServiceManager"));
    assert_eq!(r.process, "000000000000000C");
    assert_eq!(r.thread, "");
    assert_eq!(r.message, "foo bar");

    let t = "D/ServiceManager(0x123): Service MediaPlayer has been created in process main";
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.level, Level::Debug);
    assert_eq!(r.tags, vec!("ServiceManager"));
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
    assert_eq!(r.tags, vec!("SomeThing"));
    assert_eq!(r.process, "3b7fe700");
    assert_eq!(r.thread, "");
    assert_eq!(r.message, "Parsing IPV6 address fd53:7cb8:383:4:0:0:0:68");

    let t = "2017-03-25 19:11:19.052  0x3b7fe700  D SomeThing: ";
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
    let p = CsvParser;
    let r = p.try_parse_str(t).unwrap();
    assert_eq!(r.level, Level::Info);
    assert_eq!(r.tags, vec!("ThermalEngine"));
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
        property(t).unwrap().1,
        ("ro.build.tags".to_owned(), "release-keys".to_owned())
    );
}

#[test]
fn test_parse_section() {
    let mut p = Parser::default();
    p.parse("------ EVENT LOG (logcat -d -b all) ------".into());
    let r = p.parse("07-01 14:13:14.446000000,Sensor:batt_therm:29000 mC,Info,ThermalEngine,225,295,07-01 14:13:14.446   225   295 I ThermalEngine: Sensor:batt_therm:29000 mC".into());
    assert_eq!(r.level, Level::Info);
    assert_eq!(r.tags, vec!("ThermalEngine"));
    assert_eq!(r.process, "225");
    assert_eq!(r.thread, "295");
    assert_eq!(r.message, "Sensor:batt_therm:29000 mC");
    assert_eq!(
        r.raw,
        "07-01 14:13:14.446000000,Sensor:batt_therm:29000 mC,Info,ThermalEngine,225,295,07-01 14:13:14.446   225   295 I ThermalEngine: Sensor:batt_therm:29000 mC"
    );
}

// [01086.023158][boot-drivers:dev][driver,platform_bus] INFO: [platform-bus.cc(292)] Boot Item ZBI_TYPE_SERIAL_NUMBER not found
// [01086.062890][remote-control] WARN: Failed to get serial from SysInfo status=NOT_FOUND
#[test]
fn test_parse_fuchsia() {
    let p = FuchsiaParser {};
    let r = p.try_parse_str("[01086.023158][boot-drivers:dev][driver,platform_bus] INFO: [platform-bus.cc(292)] Boot Item ZBI_TYPE_SERIAL_NUMBER not found").unwrap();
    assert_eq!(r.level, Level::Info);
    assert_eq!(r.process, "boot-drivers:dev");
    assert_eq!(r.tags, vec!("driver", "platform_bus"));
    assert_eq!(
        r.message,
        "[platform-bus.cc(292)] Boot Item ZBI_TYPE_SERIAL_NUMBER not found"
    );

    let r = p
        .try_parse_str("[01086.023158][klog] INFO: [foo] blah")
        .unwrap();
    assert_eq!(r.level, Level::Info);
    assert_eq!(r.process, "klog");
    assert!(r.tags.is_empty());
    assert_eq!(r.message, "[foo] blah");

    let r = p
        .try_parse_str("[01067.896485][dhcpv6-client] WARN: ignoring Reply to Information-Request: missing Server Id option")
        .unwrap();
    assert_eq!(r.level, Level::Warn);
    assert_eq!(r.process, "dhcpv6-client");
    assert!(r.tags.is_empty());
    assert_eq!(
        r.message,
        "ignoring Reply to Information-Request: missing Server Id option"
    );
}
