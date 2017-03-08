// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use errors::*;
use futures::{future, Future};
use kabuki::Actor;
use regex::Regex;
use super::Message;
use super::record::{Level, Record};
use super::RFuture;

trait Format {
    fn parse(&self, line: &str) -> Result<Record>;
}

macro_rules! parser {
    ($v:ident, $r:expr) => (
        #[derive(PartialEq)]
        struct $v { regex: Regex, }

        impl $v {
            fn new() -> $v {
                $v { regex: Regex::new($r).unwrap(), }
            }
        }
    );
}

parser!(PrintableFormat,
        r"(\d\d-\d\d \d\d:\d\d:\d\d\.\d\d\d)\s+(\d+)\s+(\d+) (\D)\s([a-zA-Z0-9@_\{\}\[\]=\\/\.\+\s\(\)-]*)\s*:\s*(.*)");

impl Format for PrintableFormat {
    fn parse(&self, line: &str) -> Result<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Ok(Record {
                    timestamp: captures.at(1)
                        .and_then(|c| ::time::strptime(c.trim(), "%m-%d %H:%M:%S.%f").ok()),
                    level: Level::from(captures.at(4).unwrap_or("")),
                    tag: captures.at(5).unwrap_or("").to_string().trim().to_string(),
                    process: captures.at(2).unwrap_or("").trim().to_string(),
                    thread: captures.at(3).unwrap_or("").trim().to_string(),
                    message: captures.at(6).unwrap_or("").to_string().trim().to_string(),
                    raw: line.to_owned(),
                })
            }
            None => Err("Parsing error".into()),
        }
    }
}

parser!(OldPrintableFormat,
        r"(\d\d-\d\d \d\d:\d\d:\d\d\.\d\d\d) \++\d\d\d\d (\D)/([a-zA-Z0-9-_\{\}\[\]=\\/\.\+\s]*)\(\s*(\d+)\):\s*(.*)");

impl Format for OldPrintableFormat {
    fn parse(&self, line: &str) -> Result<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Ok(Record {
                    timestamp: captures.at(1)
                        .and_then(|c| ::time::strptime(c.trim(), "%m-%d %H:%M:%S.%f").ok()),
                    level: Level::from(captures.at(2).unwrap_or("")),
                    tag: captures.at(3).unwrap_or("").to_string().trim().to_string(),
                    process: captures.at(4).unwrap_or("").to_string(),
                    thread: "".to_string(),
                    message: captures.at(5).unwrap_or("").to_string().trim().to_string(),
                    raw: line.to_owned(),
                })
            }
            None => Err("Parsing error".into()),
        }
    }
}

// D/ConnectivityService: notifyType CAP_CHANGED for NetworkAgentInfo [WIFI () - 145]
parser!(TagFormat,
        r"^(\D)/([a-zA-Z0-9-_\{\}\[\]=\\/\.\+]*)\s*:\s*(.*)");

impl Format for TagFormat {
    fn parse(&self, line: &str) -> Result<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Ok(Record {
                    timestamp: None,
                    level: Level::from(captures.at(1).unwrap_or("")),
                    tag: captures.at(2).unwrap_or("").to_string().trim().to_string(),
                    process: "".to_string(),
                    thread: "".to_string(),
                    message: captures.at(3).unwrap_or("").to_string().trim().to_string(),
                    raw: line.to_owned(),
                })
            }
            None => Err("Parsing error".into()),
        }
    }
}

// I(  801:  815) uid=1000(system) Binder_1 expire 3 lines
parser!(ThreadFormat, r"(\D)\(\s*(\d+):\s*(\d+)\)\s*(.*)");

impl Format for ThreadFormat {
    fn parse(&self, line: &str) -> Result<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Ok(Record {
                    timestamp: None,
                    level: Level::from(captures.at(1).unwrap_or("")),
                    tag: "".to_string(),
                    process: captures.at(2).unwrap_or("").to_string(),
                    thread: captures.at(3).unwrap_or("").to_string(),
                    message: captures.at(4).unwrap_or("").to_string().trim().to_string(),
                    raw: line.to_owned(),
                })
            }
            None => Err("Parsing error".into()),
        }
    }
}

// D/ServiceManager(711ad700): Service MediaPlayer has been created in process main
// or time
// 01-01 00:54:21.129 V/u-blox  (  247): checkRecvInitReq: Serial not properly opened
parser!(MindroidFormat,
        r"^(\d\d-\d\d \d\d:\d\d:\d\d\.\d\d\d){0,1}\s*(\D)/([a-zA-Z0-9-_\{\}\[\]=\\/\. \+]*)\(\s*([0-9a-f]+)\):\s*(.*)");

impl Format for MindroidFormat {
    fn parse(&self, line: &str) -> Result<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Ok(Record {
                    timestamp: captures.at(1)
                        .and_then(|c| ::time::strptime(c.trim(), "%m-%d %H:%M:%S.%f").ok()),
                    level: Level::from(captures.at(2).unwrap_or("")),
                    tag: captures.at(3).unwrap_or("").trim().to_string(),
                    process: captures.at(4).unwrap_or("").trim().to_string(),
                    thread: "".to_string(),
                    message: captures.at(5).unwrap_or("").to_string().trim().to_string(),
                    raw: line.to_owned(),
                })
            }
            None => Err("Parsing error".into()),
        }
    }
}

// "11-05 19:55:27.791000000","ConnectivityService","798","1013","D","notifyType CAP_CHANGED for NetworkAgentInfo [MOBILE (UMTS) - 109]"
#[derive(PartialEq)]
struct CsvFormat;

impl CsvFormat {
    fn new() -> CsvFormat {
        CsvFormat {}
    }
}

impl Format for CsvFormat {
    fn parse(&self, line: &str) -> Result<Record> {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim().trim_matches('"')).collect();
        if parts.len() >= 6 {
            Ok(Record {
                timestamp: ::time::strptime(parts[0].trim(), "%m-%d %H:%M:%S.%f").ok(),
                level: Level::from(parts[4]),
                tag: parts[1].to_owned(),
                process: parts[2].to_owned(),
                thread: parts[3].to_owned(),
                message: parts[5..].iter().map(|s| s.to_string()).collect(),
                raw: line.to_owned(),
            })
        } else {
            Err("Parsing error".into())
        }
    }
}

pub struct Parser {
    format: Option<Box<Format + Send + Sync>>,
    parsers: Vec<Box<Format + Send + Sync>>,
}

impl Parser {
    pub fn new() -> Self {
        Parser {
            format: None,
            parsers: vec![Box::new(MindroidFormat::new()),
                          Box::new(PrintableFormat::new()),
                          Box::new(OldPrintableFormat::new()),
                          Box::new(TagFormat::new()),
                          Box::new(ThreadFormat::new()),
                          Box::new(CsvFormat::new())],
        }
    }

    fn detect(&mut self, record: &Record) -> Option<Box<Format + Send + Sync>> {
        for i in 0..self.parsers.len() {
            if self.parsers[i].parse(&record.raw).is_ok() {
                return Some(self.parsers.remove(i));
            }
        }
        None
    }
}

impl Actor for Parser {
    type Request = Message;
    type Response = Message;
    type Error = Error;
    type Future = RFuture<Message>;

    fn call(&mut self, message: Message) -> Self::Future {
        let r = match message {
            Message::Record(ref record) => {
                if self.format.is_none() {
                    self.format = self.detect(record);
                }
                match self.format {
                    Some(ref p) => {
                        Message::Record(p.parse(&record.raw).unwrap_or(Record {
                            message: record.raw.clone(),
                            raw: record.raw.clone(),
                            ..Default::default()
                        }))
                    }
                    None => {
                        Message::Record(Record {
                            message: record.raw.clone(),
                            raw: record.raw.clone(),
                            ..Default::default()
                        })
                    }
                }
            }
            _ => message,
        };
        future::ok(r).boxed()
    }
}

#[test]
fn test_printable() {
    assert!(PrintableFormat::new()
        .parse("03-01 02:19:45.207     0     0 I EXT4-fs (mmcblk3p8): mounted filesystem with \
                ordered data mode. Opts: (null)")
        .is_ok());
    assert!(PrintableFormat::new()
        .parse("03-01 02:19:42.868     0     0 I /soc/aips-bus@02100000/usdhc@0219c000: \
                voltage-ranges unspecified")
        .is_ok());
    assert!(PrintableFormat::new()
        .parse("11-06 13:58:53.582 31359 31420 I GStreamer+amc: 0:00:00.326067533 0xb8ef2a00 \
                gstamc.c:1526:scan_codecs Checking codec 'OMX.ffmpeg.flac.decoder")
        .is_ok());
    assert!(PrintableFormat::new()
        .parse("08-20 12:13:47.931 30786 30786 D EventBus: No subscribers registered for event \
                class com.runtastic.android.events.bolt.music.MusicStateChangedEvent")
        .is_ok());
    assert!(PrintableFormat::new()
        .parse("01-01 00:00:48.990   121   121 E Provisioner {XXXX-XXX-7}: 	at \
                coresaaaaaaa.provisioning.d.j(SourceFile:1352)")
        .is_ok());
}

#[test]
fn test_tag() {
    assert!(TagFormat::new().parse("V/Av+rcp   : isPlayStateTobeUpdated: device: null").is_ok());
}

#[test]
fn test_thread() {
    assert!(ThreadFormat::new()
        .parse("I(  801:  815) uid=1000(system) Binder_1 expire 3 lines")
        .is_ok());
}

#[test]
fn test_mindroid() {
    assert!(MindroidFormat::new()
        .parse("D/ServiceManager+(711ad700): Service MediaPlayer has been created in process \
                main")
        .is_ok());
    assert!(MindroidFormat::new()
        .parse("E/u-blox  (  247): connectReceiver: Failing to reopen the serial port")
        .is_ok());
    assert!(MindroidFormat::new()
        .parse("01-01 00:54:21.129 V/u-blox  (  247): checkRecvInitReq: Serial not properly \
                opened")
        .is_ok());
}

#[test]
fn test_csv() {
    assert!(CsvFormat::new()
        .parse("11-04 23:14:11.566000000\",\"vold\",\"181\",\"191\",\"D\",\"Waiting for FUSE to \
                spin up...")
        .is_ok());
    assert!(CsvFormat::new()
        .parse("11-04 23:14:37.171000000\",\"chatty\",\"798\",\"2107\",\"I\",\"uid=1000(s,,,,,,\
                ystem) Binder_C expire 12 lines")
        .is_ok());
}
