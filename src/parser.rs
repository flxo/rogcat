// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use regex::Regex;
use super::node::Node;
use super::record::{Level, Record};

trait Format {
    fn parse(&self, line: &str) -> Option<Record>;
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
        r"(\d\d-\d\d \d\d:\d\d:\d\d\.\d\d\d)\s+(\d+)\s+(\d+) (\D)\s([a-zA-Z0-9-_\{\}\[\]=\\/\.\+\s]*)\s*:\s*(.*)");

impl Format for PrintableFormat {
    fn parse(&self, line: &str) -> Option<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Some(Record {
                    timestamp: captures.at(1)
                        .and_then(|c| ::time::strptime(c.trim(), "%m-%d %H:%M:%S.%f").ok()),
                    level: Level::from(captures.at(4).unwrap_or("")),
                    tag: captures.at(5).unwrap_or("").to_string().trim().to_string(),
                    process: captures.at(2).unwrap_or("").to_string(),
                    thread: captures.at(3).unwrap_or("").to_string(),
                    message: captures.at(6).unwrap_or("").to_string().trim().to_string(),
                    raw: line.to_owned(),
                })
            }
            None => None,
        }
    }
}

parser!(OldPrintableFormat,
        r"(\d\d-\d\d \d\d:\d\d:\d\d\.\d\d\d) \++\d\d\d\d (\D)/([a-zA-Z0-9-_\{\}\[\]=\\/\.\+\s]*)\(\s*(\d+)\):\s*(.*)");

impl Format for OldPrintableFormat {
    fn parse(&self, line: &str) -> Option<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Some(Record {
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
            None => None,
        }
    }
}

// D/ConnectivityService: notifyType CAP_CHANGED for NetworkAgentInfo [WIFI () - 145]
parser!(TagFormat,
        r"^(\D)/([a-zA-Z0-9-_\{\}\[\]=\\/\.\+]*)\s*:\s*(.*)");

impl Format for TagFormat {
    fn parse(&self, line: &str) -> Option<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Some(Record {
                    timestamp: None,
                    level: Level::from(captures.at(1).unwrap_or("")),
                    tag: captures.at(2).unwrap_or("").to_string().trim().to_string(),
                    process: "".to_string(),
                    thread: "".to_string(),
                    message: captures.at(3).unwrap_or("").to_string().trim().to_string(),
                    raw: line.to_owned(),
                })
            }
            None => None,
        }
    }
}

// I(  801:  815) uid=1000(system) Binder_1 expire 3 lines
parser!(ThreadFormat, r"(\D)\(\s*(\d+):\s*(\d+)\)\s*(.*)");

impl Format for ThreadFormat {
    fn parse(&self, line: &str) -> Option<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Some(Record {
                    timestamp: None,
                    level: Level::from(captures.at(1).unwrap_or("")),
                    tag: "".to_string(),
                    process: captures.at(2).unwrap_or("").to_string(),
                    thread: captures.at(3).unwrap_or("").to_string(),
                    message: captures.at(4).unwrap_or("").to_string().trim().to_string(),
                    raw: line.to_owned(),
                })
            }
            None => None,
        }
    }
}

// D/ServiceManager(711ad700): Service MediaPlayer has been created in process main
parser!(MindroidFormat,
        r"^(\D)/([a-zA-Z0-9-_\{\}\[\]=\\/\. \+]*)\(([0-9a-f]+)\):\s*(.*)");

impl Format for MindroidFormat {
    fn parse(&self, line: &str) -> Option<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Some(Record {
                    timestamp: None,
                    level: Level::from(captures.at(1).unwrap_or("")),
                    tag: captures.at(2).unwrap_or("").to_string(),
                    process: captures.at(3).unwrap_or("").to_string(),
                    thread: "".to_string(),
                    message: captures.at(4).unwrap_or("").to_string().trim().to_string(),
                    raw: line.to_owned(),
                })
            }
            None => None,
        }
    }
}

// Nov  5 10:22:34 flap kernel: [ 1262.374536] usb 2-2: Manufacturer: motorola
parser!(SyslogFormat,
        r"(\S+\s+\d\s\d\d:\d\d:\d\d) ([_0-9a-zA-Z\.\[\]]+ [_0-9a-zA-Z\.\[\]]+):\s*(.*)");

impl Format for SyslogFormat {
    fn parse(&self, line: &str) -> Option<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Some(Record {
                    timestamp: None, // TODO
                    level: Level::Debug,
                    tag: captures.at(2).unwrap_or("").to_string(),
                    process: "".to_string(),
                    thread: "".to_string(),
                    message: captures.at(3).unwrap_or("").to_string().trim().to_string(),
                    raw: line.to_owned(),
                })
            }
            None => None,
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
    fn parse(&self, line: &str) -> Option<Record> {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim().trim_matches('"')).collect();
        if parts.len() >= 6 {
            Some(Record {
                timestamp: ::time::strptime(parts[0].trim(), "%m-%d %H:%M:%S.%f").ok(),
                level: Level::from(parts[4]),
                tag: parts[1].to_owned(),
                process: parts[2].to_owned(),
                thread: parts[3].to_owned(),
                message: parts[5..].iter().map(|s| s.to_string()).collect(),
                raw: line.to_owned(),
            })
        } else {
            None
        }
    }
}

pub struct Parser {
    format: Option<Box<Format + Send + Sync>>,
    parsers: Vec<Box<Format + Send + Sync>>,
}

impl Parser {
    fn detect(&mut self, record: &Record) -> Option<Box<Format + Send + Sync>> {
        for i in 0..self.parsers.len() {
            if self.parsers[i].parse(&record.raw).is_some() {
                let p = self.parsers.remove(i);
                return Some(p);
            }
        }
        None
    }
}

impl Default for Parser {
    fn default() -> Parser {
        Parser {
            format: None,
            parsers: vec![Box::new(MindroidFormat::new()),
                          Box::new(PrintableFormat::new()),
                          Box::new(OldPrintableFormat::new()),
                          Box::new(TagFormat::new()),
                          Box::new(ThreadFormat::new()),
                          Box::new(CsvFormat::new()),
                          Box::new(SyslogFormat::new())],
        }
    }
}

impl Node<Record, ()> for Parser {
    fn new(_: ()) -> Result<Box<Self>, String> {
        Ok(Box::new(Parser::default()))
    }

    fn message(&mut self, record: Record) -> Result<Option<Record>, String> {
        if self.format.is_none() {
            self.format = self.detect(&record);
        }
        match self.format {
            Some(ref p) => Ok(Some(p.parse(&record.raw).unwrap_or(Record::new(&record.raw)))),
            None => Ok(Some(Record::new(&record.raw))), 
        }
    }
}

#[test]
fn test_printable() {
    assert!(PrintableFormat::new()
        .parse("11-06 13:58:53.582 31359 31420 I GStreamer+amc: 0:00:00.326067533 0xb8ef2a00 \
                gstamc.c:1526:scan_codecs Checking codec 'OMX.ffmpeg.flac.decoder")
        .is_some());
    assert!(PrintableFormat::new()
        .parse("08-20 12:13:47.931 30786 30786 D EventBus: No subscribers registered for event \
                class com.runtastic.android.events.bolt.music.MusicStateChangedEvent")
        .is_some());
    assert!(PrintableFormat::new()
        .parse("01-01 00:00:48.990   121   121 E Provisioner {XXXX-XXX-7}: 	at \
                coresaaaaaaa.provisioning.d.j(SourceFile:1352)")
        .is_some());
}

#[test]
fn test_tag() {
    assert!(TagFormat::new().parse("V/Av+rcp   : isPlayStateTobeUpdated: device: null").is_some());
}

#[test]
fn test_thread() {
    assert!(ThreadFormat::new()
        .parse("I(  801:  815) uid=1000(system) Binder_1 expire 3 lines")
        .is_some());
}

#[test]
fn test_mindroid() {
    assert!(MindroidFormat::new()
        .parse("D/ServiceManager+(711ad700): Service MediaPlayer has been created in process \
                main")
        .is_some());
}

#[test]
fn test_syslog() {
    assert!(SyslogFormat::new()
        .parse("Nov  5 10:22:34 flap kernel: [ 1262.374536] usb 2-2: Manufacturer: motorola")
        .is_some());
    assert!(SyslogFormat::new()
        .parse("Nov  5 11:08:34 flap wpa_supplicant[1342]: wlp2s0: WPA: Group rekeying \
                completed with 00:11:22:33:44:55 [GTK=CCMP]")
        .is_some());
}

#[test]
fn test_csv() {
    assert!(CsvFormat::new()
        .parse("11-04 23:14:11.566000000\",\"vold\",\"181\",\"191\",\"D\",\"Waiting for FUSE to \
                spin up...")
        .is_some());
    assert!(CsvFormat::new()
        .parse("11-04 23:14:37.171000000\",\"chatty\",\"798\",\"2107\",\"I\",\"uid=1000(s,,,,,,\
                ystem) Binder_C expire 12 lines")
        .is_some());
}
