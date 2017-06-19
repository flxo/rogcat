// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use serde::{Serialize, Serializer};
use serde::ser::SerializeMap;
use std::str::FromStr;
use super::errors::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Format {
    Csv,
    Html,
    Human,
    Raw,
}

impl FromStr for Format {
    type Err = &'static str;
    fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
        match s {
            "csv" => Ok(Format::Csv),
            "html" => Ok(Format::Html),
            "human" => Ok(Format::Human),
            "raw" => Ok(Format::Raw),
            _ => Err("Format parsing error"),
        }
    }
}

const LEVEL_VALUES: &'static[&'static str] = 
        &[
            "trace",
            "debug",
            "info",
            "warn",
            "error",
            "fatal",
            "assert",
            "T",
            "D",
            "I",
            "W",
            "E",
            "F",
            "A",
        ];

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum Level {
    None,
    Trace,
    Verbose,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
    Assert,
}

impl ::std::fmt::Display for Level {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                Level::None => "-",
                Level::Trace => "T",
                Level::Verbose => "V",
                Level::Debug => "D",
                Level::Info => "I",
                Level::Warn => "W",
                Level::Error => "E",
                Level::Fatal => "F",
                Level::Assert => "A",
            }
        )
    }
}

impl Default for Level {
    fn default() -> Level {
        Level::None
    }
}

impl<'a> From<&'a str> for Level {
    fn from(s: &str) -> Self {
        match s {
            "T" | "trace" => Level::Trace,
            "V" | "verbose" => Level::Verbose,
            "D" | "debug" => Level::Debug,
            "I" | "info" => Level::Info,
            "W" | "warn" => Level::Warn,
            "E" | "error" => Level::Error,
            "F" | "fatal" => Level::Fatal,
            "A" | "assert" => Level::Assert,
            _ => Level::None,
        }
    }
}

impl Level {
    pub fn values() -> &'static [&'static str] {
        LEVEL_VALUES
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    pub timestamp: Option<::time::Tm>,
    pub message: String,
    pub level: Level,
    pub tag: String,
    pub process: String,
    pub thread: String,
    pub raw: String,
}

impl Serialize for Record {
    fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(
            6 + if self.timestamp.is_some() { 1 } else { 0 },
        ))?;
        if let Some(timestamp) = self.timestamp {
            let t = ::time::strftime("%m-%d %H:%M:%S.%f", &timestamp).unwrap_or("".to_owned());
            let t = &t[..(t.len() - 6)];
            map.serialize_entry("timestamp", &t)?;
        } else {
            map.serialize_entry("timestamp", "")?;
        };
        map.serialize_entry("message", &self.message)?;
        map.serialize_entry("level", &format!("{}", self.level))?;
        map.serialize_entry("tag", &self.tag)?;
        map.serialize_entry("process", &self.process)?;
        map.serialize_entry("thread", &self.thread)?;
        map.serialize_entry("raw", &self.raw)?;
        map.end()
    }
}

impl Record {
    pub fn format(&self, format: Format) -> Result<String> {
        Ok(match format {
            // TODO: refactor
            Format::Csv => {
                format!(
                    "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"",
                    self.timestamp
                        .and_then(|ts| ::time::strftime("%m-%d %H:%M:%S.%f", &ts).ok())
                        .unwrap_or("".to_owned()),
                    self.tag,
                    self.process,
                    self.thread,
                    self.level,
                    self.message
                )
            }
            Format::Raw => self.raw.clone(),
            Format::Human | Format::Html => panic!("Unimplemented"),
        })
    }
}
