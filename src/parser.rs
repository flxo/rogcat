// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use regex::Regex;
use Level;
use Record;

trait Format {
    fn parse(&self, line: &str) -> Option<Record>;
}

macro_rules! parser {
    ($v:ident, $r:expr) => (
        struct $v {
            regex: Regex,
        }

        impl $v {
            fn new() -> $v {
                $v {
                    regex: Regex::new($r).unwrap(),
                }
            }
        }
    );
}

parser!(PrintableFormat,
        r"(\d\d-\d\d \d\d:\d\d:\d\d\.\d\d\d)\s+(\d+)\s+(\d+) (\D)\s([a-zA-Z0-9-_\{\}\[\]=\\/\.\+]*)\s*: (.*)");

impl Format for PrintableFormat {
    fn parse(&self, line: &str) -> Option<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Some(Record {
                    timestamp: match ::time::strptime(captures.at(1).unwrap_or("").trim(),
                                                      "%m-%d %H:%M:%S.%f") {
                        Ok(tm) => tm,
                        Err(_) => panic!("failed to parse timestamp"),
                    },
                    level: Level::from(captures.at(4).unwrap_or("")),
                    tag: captures.at(5).unwrap_or("").to_string().trim().to_string(),
                    process: captures.at(2).unwrap_or("").to_string(),
                    thread: captures.at(3).unwrap_or("").to_string(),
                    message: captures.at(6).unwrap_or("").to_string().trim().to_string(),
                })
            }
            None => None,
        }
    }
}

parser!(OldPrintableFormat,
        r"(\d\d-\d\d \d\d:\d\d:\d\d\.\d\d\d) \++\d\d\d\d (\D)/([a-zA-Z0-9-_\{\}\[\]=\\/\.\+]*)\(\s*(\d+)\): (.*)");

impl Format for OldPrintableFormat {
    fn parse(&self, line: &str) -> Option<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Some(Record {
                    timestamp: match ::time::strptime(captures.at(1).unwrap_or("").trim(),
                                                      "%m-%d %H:%M:%S.%f") {
                        Ok(tm) => tm,
                        Err(_) => panic!("failed to parse timestamp"),
                    },
                    level: Level::from(captures.at(2).unwrap_or("")),
                    tag: captures.at(3).unwrap_or("").to_string().trim().to_string(),
                    process: captures.at(4).unwrap_or("").to_string(),
                    thread: "".to_string(),
                    message: captures.at(5).unwrap_or("").to_string().trim().to_string(),
                })
            }
            None => None,
        }
    }
}

// D/ConnectivityService: notifyType CAP_CHANGED for NetworkAgentInfo [WIFI () - 145]
parser!(TagFormat, r"^(\D)/([a-zA-Z0-9-_\{\}\[\]=\\/\.\+]*)\s*: (.*)");

impl Format for TagFormat {
    fn parse(&self, line: &str) -> Option<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Some(Record {
                    timestamp: ::time::now(),
                    level: Level::from(captures.at(1).unwrap_or("")),
                    tag: captures.at(2).unwrap_or("").to_string().trim().to_string(),
                    process: "".to_string(),
                    thread: "".to_string(),
                    message: captures.at(3).unwrap_or("").to_string().trim().to_string(),
                })
            }
            None => None,
        }
    }
}

// I(  801:  815) uid=1000(system) Binder_1 expire 3 lines
parser!(ThreadFormat, r"(\D)\(\s*(\d+):\s*(\d+)\) (.*)");

impl Format for ThreadFormat {
    fn parse(&self, line: &str) -> Option<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Some(Record {
                    timestamp: ::time::now(),
                    level: Level::from(captures.at(1).unwrap_or("")),
                    tag: "".to_string(),
                    process: captures.at(2).unwrap_or("").to_string(),
                    thread: captures.at(3).unwrap_or("").to_string(),
                    message: captures.at(4).unwrap_or("").to_string().trim().to_string(),
                })
            }
            None => None,
        }
    }
}

// D/ServiceManager(711ad700): Service MediaPlayer has been created in process main
parser!(MindroidFormat,
        r"^(\D)/([a-zA-Z0-9-_\{\}\[\]=\\/\. \+]*)\(([0-9a-f]+)\): (.*)");

impl Format for MindroidFormat {
    fn parse(&self, line: &str) -> Option<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Some(Record {
                    timestamp: ::time::now(),
                    level: Level::from(captures.at(1).unwrap_or("")),
                    tag: captures.at(2).unwrap_or("").to_string(),
                    process: captures.at(3).unwrap_or("").to_string(),
                    thread: "".to_string(),
                    message: captures.at(4).unwrap_or("").to_string().trim().to_string(),
                })
            }
            None => None,
        }
    }
}

// Nov  5 10:22:34 flap kernel: [ 1262.374536] usb 2-2: Manufacturer: motorola
parser!(SyslogFormat,
        r"(\S+\s+\d\s\d\d:\d\d:\d\d) ([_0-9a-zA-Z\.\[\]]+ [_0-9a-zA-Z\.\[\]]+): (.*)");

impl Format for SyslogFormat {
    fn parse(&self, line: &str) -> Option<Record> {
        match self.regex.captures(line) {
            Some(captures) => {
                Some(Record {
                    timestamp: ::time::now(),
                    level: Level::Debug,
                    tag: captures.at(2).unwrap_or("").to_string(),
                    process: "".to_string(),
                    thread: "".to_string(),
                    message: captures.at(3).unwrap_or("").to_string().trim().to_string(),
                })
            }
            None => None,
        }
    }
}

pub struct Parser {
    parser: Option<Box<Format>>,
}


impl Parser {
    pub fn new() -> Parser {
        Parser { parser: None }
    }

    fn default_record(line: &str) -> Record {
        Record {
            timestamp: ::time::now(),
            level: Level::Debug,
            tag: "".to_string(),
            process: "".to_string(),
            thread: "".to_string(),
            message: line.to_string().trim().to_string(),
        }
    }

    fn detect_format(line: &str) -> Option<Box<Format>> {
        let parsers = vec![Box::new(MindroidFormat::new()) as Box<Format>,
                           Box::new(PrintableFormat::new()) as Box<Format>,
                           Box::new(OldPrintableFormat::new()) as Box<Format>,
                           Box::new(ThreadFormat::new()) as Box<Format>,
                           Box::new(TagFormat::new()) as Box<Format>,
                           Box::new(SyslogFormat::new()) as Box<Format>];

        for p in parsers {
            if p.parse(line).is_some() {
                return Some(p);
            }
        }
        None
    }

    pub fn parse(&mut self, line: &str) -> Record {
        if self.parser.is_none() {
            self.parser = Self::detect_format(line);
        }

        match self.parser {
            Some(ref p) => (*p).parse(line).unwrap_or_else(|| Self::default_record(line)),
            None => Self::default_record(line),
        }
    }
}

#[test]
fn test_printable() {
 
    let lines = [ "11-06 13:58:53.582 31359 31420 I GStreamer+amc: 0:00:00.326067533 0xb8ef2a00 gstamc.c:1526:scan_codecs Checking codec 'OMX.ffmpeg.flac.decoder",
                  "08-20 12:13:47.931 30786 30786 D EventBus: No subscribers registered for event class com.runtastic.android.events.bolt.music.MusicStateChangedEvent" ];
    for line in &lines {
        assert!(PrintableFormat::new().parse(line).is_some());
    }
}

#[test]
fn test_tag() {
    let line = "V/Av+rcp   : isPlayStateTobeUpdated: device: null";
    assert!(TagFormat::new().parse(line).is_some());
}

#[test]
fn test_thread() {
    let line = "I(  801:  815) uid=1000(system) Binder_1 expire 3 lines";
    assert!(ThreadFormat::new().parse(line).is_some());
}

#[test]
fn test_mindroid() {
    let line = "D/ServiceManager+(711ad700): Service MediaPlayer has been created in process main";
    assert!(MindroidFormat::new().parse(line).is_some());
}

#[test]
fn test_syslog() {
    let lines = ["Nov  5 10:22:34 flap kernel: [ 1262.374536] usb 2-2: Manufacturer: motorola",
                 "Nov  5 11:08:34 flap wpa_supplicant[1342]: wlp2s0: WPA: Group rekeying completed with 00:11:22:33:44:55 [GTK=CCMP]"];
    for l in &lines {
        assert!(SyslogFormat::new().parse(l).is_some());
    }
}
