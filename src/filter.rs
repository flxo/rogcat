// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use clap::ArgMatches;
use errors::*;
use profiles::*;
use record::{Level, Record};
use regex::Regex;

pub struct Filter {
    level: Level,
    message: Vec<Regex>,
    message_negative: Vec<Regex>,
    tag: Vec<Regex>,
    tag_negative: Vec<Regex>,
}

impl<'a> Filter {
    pub fn new(args: &ArgMatches<'a>, profile: &Profile) -> Result<Self> {
        let mut tag_filter = args.values_of("tag")
            .map(|m| m.map(|f| f.to_owned()).collect::<Vec<String>>())
            .unwrap_or_else(|| vec![]);
        tag_filter.extend(profile.tag().clone());
        let mut message_filter = args.values_of("message")
            .map(|m| m.map(|f| f.to_owned()).collect::<Vec<String>>())
            .unwrap_or_else(|| vec![]);
        message_filter.extend(profile.message().clone());

        let (tag, tag_negative) = Self::init_filter(tag_filter.clone())?;
        let (message, message_negative) = Self::init_filter(message_filter)?;

        Ok(Filter {
            level: Level::from(args.value_of("level").unwrap_or("")),
            message: message,
            message_negative: message_negative,
            tag: tag,
            tag_negative: tag_negative,
        })
    }

    fn init_filter(i: Vec<String>) -> Result<(Vec<Regex>, Vec<Regex>)> {
        let mut positive = vec![];
        let mut negative = vec![];
        for r in &i {
            if r.starts_with('!') {
                let r = &r[1..];
                negative.push(Regex::new(r).chain_err(
                    || format!("Invalid regex string: \"{}\"", r),
                )?)
            } else {
                positive.push(Regex::new(r).chain_err(
                    || format!("Invalid regex string: \"{}\"", r),
                )?)
            }
        }
        Ok((positive, negative))
    }

    pub fn filter(&mut self, record: &Option<Record>) -> bool {
        if let Some(ref record) = *record {
            if record.level < self.level {
                return false;
            }

            if !self.message.is_empty() &&
                !self.message.iter().any(|m| m.is_match(&record.message))
            {
                return false;
            }

            if self.message_negative.iter().any(
                |m| m.is_match(&record.message),
            )
            {
                return false;
            }

            if !self.tag.is_empty() && !self.tag.iter().any(|m| m.is_match(&record.tag)) {
                return false;
            }

            if self.tag_negative.iter().any(|m| m.is_match(&record.tag)) {
                return false;
            }

            true
        } else {
            true
        }
    }
}

#[test]
fn filter_args() {
    assert!(Filter::init_filter(vec![]).is_ok());
    assert!(Filter::init_filter(vec!["".to_owned()]).is_ok());
    assert!(Filter::init_filter(vec!["a".to_owned()]).is_ok());
    assert!(Filter::init_filter(vec![".*".to_owned()]).is_ok());
    assert!(Filter::init_filter(vec![".*".to_owned(), ".*".to_owned()]).is_ok());
    assert!(Filter::init_filter(vec!["(".to_owned()]).is_err());
}
