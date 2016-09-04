// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use regex::Regex;

trait Format {
    fn name(&self) -> &'static str;
    fn parse(&self, line: &str) -> Option<super::message::Message>;
}

pub struct Parser {
    parser: Option<Box<Format>>,
}

struct PrintableFormat {
    regex: Regex,
    old_regex: Regex,
}

impl PrintableFormat {
    fn new() -> PrintableFormat {
        PrintableFormat {
            regex: Regex::new(r"(\d\d-\d\d \d\d:\d\d:\d\d\.\d\d\d)\s+(\d+)\s+(\d+) (\D)\s([a-zA-Z0-9-_\{\}\[\]=\\/\.]*)\s*: (.*)").unwrap(),
            old_regex: Regex::new(r"(\d\d-\d\d \d\d:\d\d:\d\d\.\d\d\d) \++\d\d\d\d (\D)/([a-zA-Z0-9-_\{\}\[\]=\\/\.]*)\(\s*(\d+)\): (.*)").unwrap(),
        }
    }
}

impl Format for PrintableFormat {
    fn name(&self) -> &'static str {
        "printable"
    }

    fn parse(&self, line: &str) -> Option<super::message::Message> {
        if self.regex.is_match(line) {
            match self.regex.captures(line) {
                Some(captures) => {
                    Some(super::message::Message {
                        timestamp: match ::time::strptime(captures.at(1).unwrap_or("").trim(),
                                                          "%m-%d %H:%M:%S.%f") {
                            Ok(tm) => tm,
                            Err(_) => panic!("failed to parse timestamp"),
                        },
                        level: super::Level::from(captures.at(4).unwrap_or("")),
                        tag: captures.at(5).unwrap_or("").to_string().trim().to_string(),
                        process: captures.at(2).unwrap_or("").to_string(),
                        thread: captures.at(3).unwrap_or("").to_string(),
                        message: captures.at(6).unwrap_or("").to_string().trim().to_string(),
                    })
                }
                None => None,
            }
        } else {
            match self.old_regex.captures(line) {
                Some(captures) => {
                    Some(super::message::Message {
                        timestamp: match ::time::strptime(captures.at(1).unwrap_or("").trim(),
                                                          "%m-%d %H:%M:%S.%f") {
                            Ok(tm) => tm,
                            Err(_) => panic!("failed to parse timestamp"),
                        },
                        level: super::Level::from(captures.at(2).unwrap_or("")),
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
}

struct TagFormat {
    regex: Regex,
}

impl TagFormat {
    fn new() -> TagFormat {
        TagFormat {
            // D/ConnectivityService: notifyType CAP_CHANGED for NetworkAgentInfo [WIFI () - 145]
            regex: Regex::new(r"^(\D)/([a-zA-Z0-9-_\{\}\[\]=\\/\.]*)\s*: (.*)").unwrap(),
        }
    }
}

impl Format for TagFormat {
    fn name(&self) -> &'static str {
        "printable"
    }

    fn parse(&self, line: &str) -> Option<super::message::Message> {
        match self.regex.captures(line) {
            Some(captures) => {
                Some(super::message::Message {
                    timestamp: ::time::now(),
                    level: super::Level::from(captures.at(1).unwrap_or("")),
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

struct ThreadFormat {
    regex: Regex,
}

impl ThreadFormat {
    fn new() -> ThreadFormat {
        ThreadFormat {
            // I(  801:  815) uid=1000(system) Binder_1 expire 3 lines
            regex: Regex::new(r"(\D)\(\s*(\d+):\s*(\d+)\) (.*)").unwrap(),
        }
    }
}

impl Format for ThreadFormat {
    fn name(&self) -> &'static str {
        "thread"
    }

    fn parse(&self, line: &str) -> Option<super::message::Message> {
        match self.regex.captures(line) {
            Some(captures) => {
                Some(super::message::Message {
                    timestamp: ::time::now(),
                    level: super::Level::from(captures.at(1).unwrap_or("")),
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

struct MindroidFormat {
    regex: Regex,
}

impl MindroidFormat {
    fn new() -> MindroidFormat {
        MindroidFormat {
            // D/ServiceManager(711ad700): Service MediaPlayer has been created in process main
            regex: Regex::new(r"^(\D)/([a-zA-Z0-9-_\{\}\[\]=\\/\.]*)\(([0-9a-f]+)\): (.*)")
                .unwrap(),
        }
    }
}

impl Format for MindroidFormat {
    fn name(&self) -> &'static str {
        "mindroid"
    }

    fn parse(&self, line: &str) -> Option<super::message::Message> {
        match self.regex.captures(line) {
            Some(captures) => {
                Some(super::message::Message {
                    timestamp: ::time::now(),
                    level: super::Level::from(captures.at(1).unwrap_or("")),
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




impl Parser {
    pub fn new() -> Parser {
        Parser { parser: None }
    }

    fn default_message(line: &str) -> super::message::Message {
        super::message::Message {
            timestamp: ::time::now(),
            level: super::Level::Debug,
            tag: "".to_string(),
            process: "".to_string(),
            thread: "".to_string(),
            message: line.to_string().trim().to_string(),
        }
    }

    fn detect_format(line: &str) -> Option<Box<Format>> {
        let parsers = vec![Box::new(MindroidFormat::new()) as Box<Format>,
                           Box::new(PrintableFormat::new()) as Box<Format>,
                           Box::new(ThreadFormat::new()) as Box<Format>,
                           Box::new(TagFormat::new()) as Box<Format>];

        for p in parsers {
            if p.parse(line).is_some() {
                println!("Matched parser: {}", p.name());
                return Some(p);
            }
        }
        None
    }

    pub fn parse(&mut self, line: &str) -> super::message::Message {
        if self.parser.is_none() {
            self.parser = Self::detect_format(line);
        }

        match self.parser {
            Some(ref p) => (*p).parse(line).unwrap_or_else(|| Self::default_message(line)),
            None => Self::default_message(line),
        }
    }
}

#[test]
fn test_printable() {
    let line = "08-20 12:13:47.931 30786 30786 D EventBus: No subscribers registered for event \
                class com.runtastic.android.events.bolt.music.MusicStateChangedEvent";
    assert!(PrintableFormat::new().parse(line).is_some());
}

#[test]
fn test_tag() {
    let line = "V/Avrcp   : isPlayStateTobeUpdated: device: null";
    assert!(TagFormat::new().parse(line).is_some());
}

#[test]
fn test_thread() {
    let line = "I(  801:  815) uid=1000(system) Binder_1 expire 3 lines";
    assert!(ThreadFormat::new().parse(line).is_some());
}

#[test]
fn test_mindroid() {
    let line = "D/ServiceManager(711ad700): Service MediaPlayer has been created in process main";
    assert!(MindroidFormat::new().parse(line).is_some());
}
