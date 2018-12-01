// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use config::Config;
use failure::Error;
use lazy_static::lazy_static;
use serde::Deserialize;
use std::env;
use std::path::PathBuf;
use std::sync::RwLock;
use which::which_in;

lazy_static! {
    static ref CONFIG: RwLock<Config> = RwLock::new(Config::default());
}

/// Find adb binary
pub fn adb() -> Result<PathBuf, Error> {
    which_in("adb", env::var_os("PATH"), env::current_dir()?).map_err(|e| e.into())
}

pub fn terminal_width() -> Option<usize> {
    match term_size::dimensions() {
        Some((width, _)) => Some(width),
        None => env::var("COLUMNS")
            .ok()
            .and_then(|e| e.parse::<usize>().ok()),
    }
}

/// Detect configuration directory
pub fn config_dir() -> PathBuf {
    directories::BaseDirs::new()
        .unwrap()
        .config_dir()
        .join("rogcat")
}

/// Read a value from the configuration file
/// `config_dir/config.toml`
pub fn config_get<'a, T: Deserialize<'a>>(key: &'a str) -> Option<T> {
    CONFIG.read().ok().and_then(|c| c.get::<T>(key).ok())
}

pub fn config_init() {
    let config_file = config_dir().join("config.toml");
    CONFIG
        .write()
        .expect("Failed to get config lock")
        .merge(config::File::from(config_file))
        .ok();
}
