// Copyright © 2017 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use crate::utils;
use clap::{value_t, ArgMatches};
use failure::{format_err, Error};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Into;
use std::env::var;
use std::fs::File;
use std::io::Read;
use std::ops::AddAssign;
use std::path::PathBuf;
use toml::from_str;

/// Profile definition with filters and misc
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub comment: Option<String>,
    pub extends: Vec<String>,
    pub highlight: Vec<String>,
    pub message: Vec<String>,
    pub tag: Vec<String>,
}

/// Create a new Profiles instance from a give configuration file
/// and default if file is not present or readable
pub fn from_args(args: &ArgMatches) -> Result<Profile, Error> {
    let file = file(Some(args))?;
    if !file.exists() {
        Ok(Profile::default())
    } else {
        let mut config = String::new();
        File::open(file.clone())
            .map_err(|e| format_err!("Failed to open {}: {}", file.display(), e))?
            .read_to_string(&mut config)?;

        let mut config_file: ConfigurationFile = from_str(&config)
            .map_err(|e| format_err!("Failed to parse {}: {}", file.display(), e))?;

        let profiles: HashMap<String, Profile> = config_file
            .profile
            .drain()
            .map(|(k, v)| (k, v.into()))
            .collect();

        let mut profile = Profile::default();
        if let Some(n) = args.value_of("profile") {
            profile = profiles
                .get(n)
                .ok_or_else(|| format_err!("Unknown profile {}", n))?
                .clone();
            expand(n, &mut profile, &profiles)?;
        }

        Ok(profile)
    }
}

/// Expand a profile with file content
fn expand(n: &str, p: &mut Profile, a: &HashMap<String, Profile>) -> Result<(), Error> {
    let mut recursion_limit = 100;
    while !p.extends.is_empty() {
        let extends = p.extends.clone();
        p.extends.clear();
        for e in &extends {
            let f = a
                .get(e)
                .ok_or_else(|| format_err!("Unknown extend profile name {} used in {}", e, n))?;
            *p += f.clone();
        }

        recursion_limit -= 1;
        if recursion_limit == 0 {
            return Err(format_err!(
                "Reached recursion limit while resolving profile {} extends",
                n
            ));
        }
    }
    Ok(())
}

/// Return path to profile file by checking cli argument, env and default to configdir
fn file(args: Option<&ArgMatches>) -> Result<PathBuf, Error> {
    if let Some(args) = args {
        if args.is_present("profiles_path") {
            let f = PathBuf::from(value_t!(args, "profiles_path", String)?);
            if f.exists() {
                return Ok(f);
            } else {
                return Err(format_err!(
                    "Cannot find {}. Use --profiles_path to specify the path manually!",
                    f.display()
                ));
            }
        }
    }
    if let Ok(f) = var("ROGCAT_PROFILES").map(PathBuf::from) {
        if f.exists() {
            return Ok(f);
        } else {
            Err(format_err!(
                "Cannot find {} set in ROGCAT_PROFILES!",
                f.display()
            ))
        }
    } else {
        Ok(utils::config_dir().join("profiles.toml"))
    }
}

/// Configuration file
#[derive(Debug, Default, Deserialize, Serialize)]
struct ConfigurationFile {
    profile: HashMap<String, ProfileFile>,
}

/// Struct with exact layout as used in config file
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct ProfileFile {
    extends: Option<Vec<String>>,
    comment: Option<String>,
    highlight: Option<Vec<String>>,
    message: Option<Vec<String>>,
    tag: Option<Vec<String>>,
}

impl From<ProfileFile> for Profile {
    fn from(f: ProfileFile) -> Profile {
        Profile {
            comment: f.comment,
            extends: f.extends.unwrap_or_else(|| vec![]),
            highlight: f.highlight.unwrap_or_else(|| vec![]),
            message: f.message.unwrap_or_else(|| vec![]),
            tag: f.tag.unwrap_or_else(|| vec![]),
        }
    }
}

impl AddAssign for Profile {
    fn add_assign(&mut self, other: Profile) {
        macro_rules! vec_extend {
            ($x:expr, $y:expr) => {
                $x.extend($y);
                $x.sort();
                $x.dedup();
            };
        }

        vec_extend!(self.extends, other.extends);
        vec_extend!(self.highlight, other.highlight);
        vec_extend!(self.message, other.message);
        vec_extend!(self.tag, other.tag);
    }
}
