// Copyright © 2017 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use appdirs::user_config_dir;
use clap::ArgMatches;
use errors::*;
use std::collections::HashMap;
use std::env::var;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;
use toml::{from_str, to_string};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Profile {
    comment: Option<String>,
    highlight: Option<Vec<String>>,
    message: Option<Vec<String>>,
    tag: Option<Vec<String>>,
}

/// Configuration file layout
#[derive(Debug, Default, Deserialize, Serialize)]
struct ConfigurationFile {
    profile: HashMap<String, Profile>,
}

impl Profile {
    pub fn comment(&self) -> Option<String> {
        self.comment.clone()
    }

    pub fn highlight(&self) -> Vec<String> {
        self.highlight.clone().unwrap_or(vec![])
    }

    pub fn message(&self) -> Vec<String> {
        self.message.clone().unwrap_or(vec![])
    }

    pub fn tag(&self) -> Vec<String> {
        self.tag.clone().unwrap_or(vec![])
    }
}

#[derive(Debug, Default)]
pub struct Configuration {
    file: PathBuf,
    profile: Profile,
    config_file: ConfigurationFile,
}

impl Configuration {
    pub fn new(args: &ArgMatches) -> Result<Self> {
        let file = Self::file(Some(args))?;
        if !file.exists() {
            Ok(Configuration {
                file,
                ..Default::default()
            })
        } else {
            let mut config = String::new();
            File::open(file.clone())
                .chain_err(|| format!("Failed to open {:?}", file))?
                .read_to_string(&mut config)?;

            let config_file: ConfigurationFile = from_str(&config).chain_err(|| {
                format!("Failed to parse \"{}\"", file.display())
            })?;
            let profile = if let Some(name) = args.value_of("profile") {
                if let Some(profile) = config_file.profile.get(name) {
                    profile.clone()
                } else {
                    return Err(format!("Unknown profile \"{}\"", name).into());
                }
            } else {
                Profile::default()
            };

            Ok(Configuration {
                file,
                profile,
                config_file,
            })
        }
    }

    pub fn profile(&self) -> Profile {
        self.profile.clone()
    }

    pub fn command_profiles(self, args: &ArgMatches) -> Result<i32> {
        if args.is_present("list") {
            if self.config_file.profile.is_empty() {
                println!("No profiles present in \"{}\".", self.file.display());
            } else {
                println!("Available profiles in \"{}\":", self.file.display());
                for (k, v) in self.config_file.profile {
                    println!(
                        " * {}{}",
                        k,
                        v.comment().map(|c| format!(": {}", c)).unwrap_or("".into())
                    );
                }
            }
            Ok(0)
        } else {
            Err("Missing option for profiles subcommand!".into())
        }
    }

    pub fn command_configuration(&self, args: &ArgMatches) -> Result<i32> {
        if args.is_present("example") {
            let mut example = ConfigurationFile::default();

            example.profile.insert(
                "W hitespace".into(),
                Profile {
                    comment: Some(
                        "Profile names can contain whitespaces. Quote on command line..."
                            .into(),
                    ),
                    ..Default::default()
                },
            );

            example.profile.insert(
                "rogcat".into(),
                Profile {
                    comment: Some("Only tag \"rogcat\"".into()),
                    tag: Some(vec!["^rogcat$".into()]),
                    ..Default::default()
                },
            );

            example.profile.insert(
                "Comments are optional".into(),
                Profile {
                    tag: Some(vec!["rogcat".into()]),
                    ..Default::default()
                },
            );

            example.profile.insert(
                "R".into(),
                Profile {
                    comment: Some("Messages starting with R".into()),
                    message: Some(vec!["^R.*".into()]),
                    ..Default::default()
                },
            );

            example.profile.insert(
                "complex".into(),
                Profile {
                    comment: Some(
                        "Profiles can be complex. This one is probably very useless.".into(),
                    ),
                    tag: Some(vec!["b*".into(), "!adb".into()]),
                    message: Some(vec!["^R.*".into(), "!^A.*".into(), "!^A.*".into()]),
                    highlight: Some(vec!["blah".into()]),
                    ..Default::default()
                },
            );

            to_string(&example)
                .map_err(|e| {
                    format!("Internal example serialization error: {}", e).into()
                })
                .map(|s| {
                    println!("Example configuration:");
                    println!("");
                    println!("{}", s);
                    0
                })
        } else {
            Err("Missing option for config subcommand!".into())
        }
    }

    pub fn file(args: Option<&ArgMatches>) -> Result<PathBuf> {
        if let Some(args) = args {
            if args.is_present("config") {
                let f = PathBuf::from(value_t!(args, "config", String)?);
                if f.exists() {
                    return Ok(f)
                } else {
                    return Err(
                        format!("Cannot find \"{}\" set --config!", f.display()).into(),
                    )
                }
            }
        } 
        if let Ok(f) = var("ROGCAT_CONFIG").map(|c| PathBuf::from(c)) {
            println!("env: {}", f.display());
            if f.exists() {
                return Ok(f);
            } else {
                Err(
                    format!("Cannot find \"{}\" set in ROGCAT_CONFIG!", f.display()).into(),
                )
            }
        } else if let Ok(mut f) = user_config_dir(Some("rogcat"), None, false) {
            f.push("config.toml");
            Ok(f)
        } else {
            Err("Failed to find config directory".into())
        }
    }
}
