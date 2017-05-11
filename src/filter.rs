// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use clap::ArgMatches;
use errors::*;
use super::record::Level;
use regex::Regex;
use futures::{future, Future};
use super::{Message, Node, RFuture};

pub struct Filter {
    level: Level,
    message: Vec<Regex>,
    message_negative: Vec<Regex>,
    tag: Vec<Regex>,
    tag_negative: Vec<Regex>,
}

impl<'a> Filter {
    pub fn new(args: &ArgMatches<'a>) -> Result<Self> {
        let filters = |k| {
            args.values_of(k)
                .map(|m| {
                    m.map(|f| f.to_owned())
                        .collect::<Vec<String>>()
                })
        };
        let (tag, tag_negative) = Self::init_filter(filters("tag"))?;
        let (message, message_negative) = Self::init_filter(filters("message"))?;
        Ok(Filter {
            level: Level::from(args.value_of("LEVEL").unwrap_or("")),
            message: message,
            message_negative: message_negative,
            tag: tag,
            tag_negative: tag_negative,
        })
    }

    /// Try to build regex from args
    fn init_filter(i: Option<Vec<String>>) -> Result<(Vec<Regex>, Vec<Regex>)> {
        let mut positive = vec![];
        let mut negative = vec![];
        for r in &i.unwrap_or_else(|| vec![]) {
            if r.starts_with("!") {
                negative.push(Regex::new(&r[1..])?)
            } else {
                positive.push(Regex::new(r)?)
            }
        }
        Ok((positive, negative))
    }
}

impl Node for Filter {
    type Input = Message;

    fn process(&mut self, message: Message) -> RFuture {
        if let Message::Record(ref record) = message {
            if record.level < self.level {
                return future::ok(Message::Drop).boxed();
            }

            if !self.message.is_empty() &&
               self.message
                .iter()
                .map(|r| if r.is_match(&record.message) { 1 } else { 0 })
                .sum::<usize>() == 0 {
                return future::ok(Message::Drop).boxed();
            }

            for m in &self.message_negative {
                if m.is_match(&record.message) {
                    return future::ok(Message::Drop).boxed();
                }
            }

            if !self.tag.is_empty() &&
               self.tag
                .iter()
                .map(|r| if r.is_match(&record.tag) { 1 } else { 0 })
                .sum::<usize>() == 0 {
                return future::ok(Message::Drop).boxed();
            }

            for t in &self.tag_negative {
                if t.is_match(&record.tag) {
                    return future::ok(Message::Drop).boxed();
                }
            }
        }
        future::ok(message).boxed()
    }
}


#[test]
fn filter_args() {
    assert!(Filter::init_filter(None).is_ok());
    assert!(Filter::init_filter(Some(vec!["".to_owned()])).is_ok());
    assert!(Filter::init_filter(Some(vec!["a".to_owned()])).is_ok());
    assert!(Filter::init_filter(Some(vec![".*".to_owned()])).is_ok());
    assert!(Filter::init_filter(Some(vec![".*".to_owned(), ".*".to_owned()])).is_ok());
    assert!(Filter::init_filter(Some(vec!["(".to_owned()])).is_err());
}
