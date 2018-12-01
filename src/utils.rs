// Copyright © 2016 Felix Obenhuber
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

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
