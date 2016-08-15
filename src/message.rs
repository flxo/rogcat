// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

pub struct Message {
    pub timestamp: ::time::Tm,
    pub message: String,
    pub level: super::Level,
    pub tag: String,
    pub process: String,
    pub thread: String,
}

impl Message {
    pub fn to_csv(&self) -> String {
        let timestamp: String = ::time::strftime("%m-%d %H:%M:%S.%f", &self.timestamp)
            .unwrap()
            .chars()
            .take(18)
            .collect();
        format!("{},{},{},{},{},{}",
                timestamp,
                self.tag,
                self.process,
                self.thread,
                self.level,
                self.message)
    }
}
