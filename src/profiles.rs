// Copyright © 2017 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use clap::ArgMatches;
use failure::{err_msg, Error};
use std::collections::HashMap;
use std::env::var;
use std::fs::File;
use std::io::prelude::*;
use std::ops::AddAssign;
use std::path::PathBuf;
use std::convert::Into;
use toml::{from_str, to_string};

const EXTEND_LIMIT: u32 = 1000;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ProfileFile {
    extends: Option<Vec<String>>,
    comment: Option<String>,
    highlight: Option<Vec<String>>,
    message: Option<Vec<String>>,
    tag: Option<Vec<String>>,
}

impl Into<Profile> for ProfileFile {
    fn into(self) -> Profile {
        Profile {
            comment: self.comment,
            extends: self.extends.unwrap_or_else(|| vec![]),
            highlight: self.highlight.unwrap_or_else(|| vec![]),
            message: self.message.unwrap_or_else(|| vec![]),
            tag: self.tag.unwrap_or_else(|| vec![]),
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ConfigurationFile {
    profile: HashMap<String, ProfileFile>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    comment: Option<String>,
    extends: Vec<String>,
    highlight: Vec<String>,
    message: Vec<String>,
    tag: Vec<String>,
}

impl Profile {
    pub fn comment(&self) -> &Option<String> {
        &self.comment
    }

    pub fn highlight(&self) -> &Vec<String> {
        &self.highlight
    }

    pub fn message(&self) -> &Vec<String> {
        &self.message
    }

    pub fn tag(&self) -> &Vec<String> {
        &self.tag
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

#[derive(Debug, Default)]
pub struct Profiles {
    file: PathBuf,
    profile: Profile,
    profiles: HashMap<String, Profile>,
}

impl Profiles {
    pub fn new(args: &ArgMatches) -> Result<Self, Error> {
        let file = Self::file(Some(args))?;
        if !file.exists() {
            Ok(Profiles {
                file,
                ..Default::default()
            })
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
                Self::expand(n, &mut profile, &profiles)?;
            }

            Ok(Profiles {
                file,
                profile,
                profiles,
            })
        }
    }

    fn expand(n: &str, p: &mut Profile, a: &HashMap<String, Profile>) -> Result<(), Error> {
        let mut loops = EXTEND_LIMIT;
        while !p.extends.is_empty() {
            let extends = p.extends.clone();
            p.extends.clear();
            for e in &extends {
                let f = a.get(e)
                    .ok_or_else(|| format_err!("Unknown extend profile name {} used in {}", e, n))?;
                *p += f.clone();
            }

            loops -= 1;
            if loops == 0 {
                return Err(format_err!(
                    "Reached recursion limit while resolving profile {} extends",
                    n
                ));
            }
        }
        Ok(())
    }

    pub fn profile(&self) -> Profile {
        self.profile.clone()
    }

    pub fn subcommand(self, args: &ArgMatches) -> Result<i32, Error> {
        if args.is_present("list") {
            if self.profiles.is_empty() {
                println!("No profiles present in \"{}\".", self.file.display());
            } else {
                println!("Available profiles in \"{}\":", self.file.display());
                for (k, v) in self.profiles {
                    println!(
                        " * {}{}",
                        k,
                        v.comment()
                            .clone()
                            .map(|c| format!(": {}", c))
                            .unwrap_or_else(|| "".into())
                    );
                }
            }
            Ok(0)
        } else if args.is_present("examples") {
            let mut example = ConfigurationFile::default();

            example.profile.insert(
                "W hitespace".into(),
                ProfileFile {
                    comment: Some(
                        "Profile names can contain whitespaces. Quote on command line...".into(),
                    ),
                    ..Default::default()
                },
            );

            example.profile.insert(
                "rogcat".into(),
                ProfileFile {
                    comment: Some("Only tag \"rogcat\"".into()),
                    tag: Some(vec!["^rogcat$".into()]),
                    ..Default::default()
                },
            );

            example.profile.insert(
                "Comments are optional".into(),
                ProfileFile {
                    tag: Some(vec!["rogcat".into()]),
                    ..Default::default()
                },
            );

            example.profile.insert(
                "A".into(),
                ProfileFile {
                    comment: Some("Messages starting with A".into()),
                    message: Some(vec!["^A.*".into()]),
                    ..Default::default()
                },
            );

            example.profile.insert(
                "B".into(),
                ProfileFile {
                    comment: Some("Messages starting with B".into()),
                    message: Some(vec!["^B.*".into()]),
                    ..Default::default()
                },
            );

            example.profile.insert(
                "ABC".into(),
                ProfileFile {
                    extends: Some(vec!["A".into(), "B".into()]),
                    comment: Some("Profiles A, B plus the following filter (^C.*)".into()),
                    message: Some(vec!["^C.*".into()]),
                    ..Default::default()
                },
            );

            example.profile.insert(
                "complex".into(),
                ProfileFile {
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
                .map_err(|e| format_err!("Internal example serialization error: {}", e))
                .map(|s| {
                    println!("Example profiles:");
                    println!();
                    println!("{}", s);
                    0
                })
        } else {
            Err(err_msg("Missing option for profiles subcommand!"))
        }
    }

    pub fn file(args: Option<&ArgMatches>) -> Result<PathBuf, Error> {
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
            Ok(::config_dir()?.join("profiles.toml"))
        }
    }
}
