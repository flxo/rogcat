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
use kabuki::Actor;
use super::Message;
use super::RFuture;

pub struct Filter {
    level: Level,
    msg: Vec<Regex>,
    tag: Vec<Regex>,
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
        Ok(Filter {
            level: Level::from(args.value_of("level").unwrap_or("")),
            msg: Self::init_filter(filters("msg"))?,
            tag: Self::init_filter(filters("tag"))?,
        })
    }

    /// Try to build regex from args
    fn init_filter(i: Option<Vec<String>>) -> Result<Vec<Regex>> {
        let mut result = vec![];
        for r in &i.unwrap_or(vec![]) {
            match Regex::new(r) {
                Ok(re) => result.push(re),
                Err(e) => return Err(e.into()),
            }
        }
        Ok(result)
    }
}

impl Actor for Filter {
    type Request = Message;
    type Response = Message;
    type Error = Error;
    type Future = RFuture<Message>;

    fn call(&mut self, message: Message) -> Self::Future {
        match message {
            Message::Record(ref record) => {
                if record.level < self.level {
                    return future::ok(Message::Drop).boxed();
                }

                for r in &self.msg {
                    if !r.is_match(&record.message) {
                        return future::ok(Message::Drop).boxed();
                    }
                }

                for r in &self.tag {
                    if !r.is_match(&record.tag) {
                        return future::ok(Message::Drop).boxed();
                    }
                }
            }
            _ => (),
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

// #[test]
// fn filter() {
//     let mut filter = Filter::new(Args {
//             level: Level::Debug,
//             msg: Some(vec!["test.*".to_owned()]),
//             tag: Some(vec!["test.*".to_owned()]),
//         })
//         .unwrap();

//     let t = Record { tag: "test".to_owned(), ..Default::default() };
//     assert_eq!(filter.message(t.clone()).unwrap(), None);

//     let t = Record { message: "test".to_owned(), ..Default::default() };
//     assert_eq!(filter.message(t.clone()).unwrap(), None);

//     let t = Record { level: Level::None, ..Default::default() };
//     assert_eq!(filter.message(t).unwrap(), None);

//     let t = Record {
//         level: Level::Warn,
//         message: "testasdf".to_owned(),
//         tag: "test123".to_owned(),
//         ..Default::default()
//     };
//     assert_eq!(filter.message(t.clone()).unwrap(), Some(t));
// }
