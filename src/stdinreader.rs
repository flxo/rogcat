// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use errors::*;
use futures::{future, Future};
use kabuki::Actor;
use std::io::stdin;
use super::Message;
use super::record::Record;

pub struct StdinReader;

impl Actor for StdinReader {
    type Request = ();
    type Response = Message;
    type Error = Error;
    type Future = RFuture<Message>;

    fn call(&mut self, _req: ()) -> Self::Future {
        let mut buffer = String::new();
        let record = match stdin().read_line(&mut buffer) {
            Ok(_) => {
                if buffer.is_empty() {
                    future::ok(Message::Finished)
                } else {
                    future::ok(Message::Record(Record {
                        timestamp: Some(::time::now()),
                        raw: buffer.trim().to_string(),
                        ..Default::default()
                    }))
                }
            }
            Err(_) => future::err("Failed to read stdin".into()),
        };
        record.boxed()
    }
}
