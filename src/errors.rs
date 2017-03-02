// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use std::error;
use futures::Future;

error_chain! {
    foreign_links {
        Io(::std::io::Error);
        Regex(::regex::Error);
    }
}

pub type RFuture<T> = Box<Future<Item = T, Error = Error>>;

pub trait FutureChainErr<T> {
    fn chain_err<F, E>(self, callback: F) -> RFuture<T>
        where F: FnOnce() -> E + 'static,
              E: Into<ErrorKind>;
}

impl<F> FutureChainErr<F::Item> for F
    where F: Future + 'static,
          F::Error: error::Error + Send + 'static
{
    fn chain_err<C, E>(self, callback: C) -> RFuture<F::Item>
        where C: FnOnce() -> E + 'static,
              E: Into<ErrorKind>
    {
        Box::new(self.then(|r| r.chain_err(callback)))
    }
}
