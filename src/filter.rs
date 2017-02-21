// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use errors::*;
use super::node::Node;
use super::record::{Level, Record};
use regex::Regex;

pub struct Args {
    pub level: Level,
    pub msg: Option<Vec<String>>,
    pub tag: Option<Vec<String>>,
}

// TODO: Add time range
pub struct Filter {
    level: Level,
    msg: Vec<Regex>,
    tag: Vec<Regex>,
}

impl Filter {
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

impl Node<Record, Args> for Filter {
    fn new(arg: Args) -> Result<Box<Self>> {
        Ok(Box::new(Filter {
            level: arg.level,
            msg: Self::init_filter(arg.msg)?,
            tag: Self::init_filter(arg.tag)?,
        }))
    }

    fn message(&mut self, record: Record) -> Result<Option<Record>> {
        if record.level < self.level {
            return Ok(None);
        }

        for r in &self.msg {
            if !r.is_match(&record.message) {
                return Ok(None);
            }
        }

        for r in &self.tag {
            if !r.is_match(&record.tag) {
                return Ok(None);
            }
        }

        Ok(Some(record))
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

    assert!(Filter::new(Args {
            level: Level::None,
            msg: None,
            tag: None,
        })
        .is_ok());

    assert!(Filter::new(Args {
            level: Level::None,
            msg: Some(vec!["(".to_owned()]),
            tag: None,
        })
        .is_err());
}

#[test]
fn filter() {
    let mut filter = Filter::new(Args {
            level: Level::Debug,
            msg: Some(vec!["test.*".to_owned()]),
            tag: Some(vec!["test.*".to_owned()]),
        })
        .unwrap();

    let t = Record { tag: "test".to_owned(), ..Default::default() };
    assert_eq!(filter.message(t.clone()).unwrap(), None);

    let t = Record { message: "test".to_owned(), ..Default::default() };
    assert_eq!(filter.message(t.clone()).unwrap(), None);

    let t = Record { level: Level::None, ..Default::default() };
    assert_eq!(filter.message(t).unwrap(), None);

    let t = Record {
        level: Level::Warn,
        message: "testasdf".to_owned(),
        tag: "test123".to_owned(),
        ..Default::default()
    };
    assert_eq!(filter.message(t.clone()).unwrap(), Some(t));
}
