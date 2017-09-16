// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use clap::ArgMatches;
use crc::{crc32, Hasher32};
use errors::*;
use futures::{Sink, StartSend, Async, AsyncSink, Poll};
use handlebars::{Handlebars, RenderContext, RenderError, Helper, JsonRender, to_json};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use serde_json::value::{Value as Json, Map};
use std::fs::{DirBuilder, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::str;
use record::{Format, Record};
use time::{now, strftime};

/// Interface for a output file format
trait Writer {
    fn new(filename: &PathBuf, format: &Format) -> Result<Box<Self>>
    where
        Self: Sized;
    fn write(&mut self, record: &Record, index: usize) -> Result<()>;
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

const TEMPLATE: &str = r#"
<!doctype HTML>
<title>Rogcat</title>
<link href='http://fonts.googleapis.com/css?family=Source+Code+Pro' rel='stylesheet' type='text/css'>
<style>
body {background: black; color: #BBBBBB; font-family: 'Source Code Pro', Monaco, monospace; font-size: 12px}
.green, .I {color: #A8FF60}
.white {color: #EEEEEE}
.red, .E, .A, .F {color: #FF6C60}
.yellow, .W {color: #FFFFB6}
.black {color: #4F4F4F}
.blue {color: #96CBFE}
.cyan {color: #C6C5FE}
.magenta {color: #FF73FD}
tr.hover { background: #260041 }
table {
    border-spacing: 0;
    width: 100%;
}
td {
    vertical-align: top;
    padding-bottom: 0;
    padding-left: 2ex;
    padding-right: 2ex;
    white-space: nowrap;
}
tr:hover {
    color: yellow;
}
td.level-D {
	color: white;
	background: #555;
}
td.level-I {
    color: black;
	background: #A8FF60;
}
td.level-W {
    color: black;
	background: #FFFFB6;
}
td.level-E {
    color: black;
	background: #FF6C60;
}
td.level-A {
    color: black;
	background: #FF6C60;
}
td.level-F {
    color: black;
	background: #FF6C60;
}
table tr td:first-child + td + td {
    text-align: right
}
table tr td:first-child + td + td + td {
}
table tr td:first-child + td + td + td + td {
    text-align: right
}
table tr td:first-child + td + td + td + td + td {
}
</style>

<table>

{{#each records as |t| ~}}
    <tr>
    <td>{{t.index}}</td>
    <td>{{t.record.timestamp}}</td>
    <td><a>{{color t.record.tag}}</a></td>
    <td>{{color t.record.process}}</td>
    <td>{{color t.record.thread}}</td>
    <td class="level-{{t.record.level}}">{{t.record.level}}</td>
    <td>{{t.record.message}}</td>
    </tr>
{{/each~}}

</table>
"#;

#[derive(Serialize)]
struct HtmlRecord {
    index: usize,
    record: Record,
}

/// Simple static html file
#[derive(Default)]
struct Html {
    filename: PathBuf,
    records: Vec<HtmlRecord>,
}

impl Html {
    // TODO: ensure readability
    fn hash_color(value: &str) -> String {
        let mut digest = crc32::Digest::new(crc32::IEEE);
        digest.write(value.as_bytes());
        let h = digest.sum32();
        let r = h & 0xFF;
        let g = (h & 0xFF00) >> 8;
        let b = (h & 0xFF0000) >> 16;
        format!("#{:02x}{:02x}{:02x}", r, g, b)
    }

    fn color_helper(
        h: &Helper,
        _: &Handlebars,
        rc: &mut RenderContext,
    ) -> ::std::result::Result<(), RenderError> {
        let param = h.param(0).ok_or_else(|| {
            RenderError::new("Param 0 is required for format helper.")
        })?;
        let value = param.value().render();
        let rendered = if value.is_empty() || value == "0" {
            format!("<span style=\"color:grey\">{}</span>", value)
        } else {
            format!(
                "<span style=\"color:{}\">{}</span>",
                Self::hash_color(&value),
                value
            )
        };
        rc.writer.write_all(rendered.into_bytes().as_ref())?;
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        let mut hb = Handlebars::new();
        let mut data: Map<String, Json> = Map::new();
        data.insert("records".to_owned(), to_json(&self.records));
        let mut output_file = File::create(&self.filename)?;
        hb.register_helper("color", Box::new(Self::color_helper));
        hb.register_template_string("t1", TEMPLATE).ok();
        hb.renderw("t1", &data, &mut output_file)?;
        Ok(())
    }
}

impl Writer for Html {
    fn new(filename: &PathBuf, _: &Format) -> Result<Box<Self>> {
        let html = Html {
            filename: filename.clone(),
            records: Vec::new(),
        };
        Ok(Box::new(html))
    }

    fn write(&mut self, record: &Record, index: usize) -> Result<()> {
        let r = HtmlRecord {
            index: index,
            record: record.clone(),
        };
        Ok(self.records.push(r))
    }
}

impl Drop for Html {
    fn drop(&mut self) {
        self.flush().ok();
    }
}

/// Textfile with format
struct Textfile {
    file: File,
    format: Format,
}

impl Writer for Textfile {
    fn new(filename: &PathBuf, format: &Format) -> Result<Box<Self>> {
        let file = File::create(filename).chain_err(|| {
            format!("Failed to create output file: {:?}", filename.display())
        })?;
        let textfile = Textfile {
            file: file,
            format: format.clone(),
        };
        Ok(Box::new(textfile))
    }

    fn write(&mut self, record: &Record, _index: usize) -> Result<()> {
        self.file
            .write(record.format(&self.format)?.as_bytes())
            .chain_err(|| "Failed to write")?;
        self.file.write(b"\n").chain_err(|| "Failed to write")?;
        Ok(())
    }
}

#[derive(Clone)]
enum FilenameFormat {
    Date(bool, u64),
    Enumerate(bool, u64),
    Single(bool),
}

pub struct FileWriter {
    current_filename: PathBuf,
    file_size: u64,
    filename: PathBuf,
    filename_format: FilenameFormat,
    format: Format,
    index: usize,
    progress: ProgressBar,
    writer: Option<Box<Writer>>,
}

impl<'a> FileWriter {
    pub fn new(args: &ArgMatches<'a>) -> Result<Self> {
        let filename = args.value_of("output")
            .and_then(|f| Some(PathBuf::from(f)))
            .ok_or("Invalid output filename!")?;

        let records_per_file = args.value_of("records_per_file").and_then(|l| {
            Regex::new(r"^(\d+)([kMG])$")
                .unwrap()
                .captures(l)
                .and_then(|caps| {
                    caps.at(1)
                        .and_then(|size| u64::from_str(size).ok())
                        .and_then(|size| Some((size, caps.at(2))))
                })
                .and_then(|(size, suffix)| match suffix {
                    Some("k") => Some(1_000 * size),
                    Some("M") => Some(1_000_000 * size),
                    Some("G") => Some(1_000_000_000 * size),
                    _ => None,
                })
                .or_else(|| u64::from_str(l).ok())
        });

        let format = args.value_of("format")
            .and_then(|f| Format::from_str(f).ok())
            .unwrap_or(Format::Raw);
        if format == Format::Human {
            return Err("Human format is unsupported when writing to files".into());
        }

        let overwrite = args.is_present("overwrite");

        let records = records_per_file.unwrap_or(::std::u64::MAX);
        let filename_format = match args.value_of("filename_format") {
            Some("enumerate") => FilenameFormat::Enumerate(overwrite, records),
            Some("date") => FilenameFormat::Date(overwrite, records),
            // If records per file is set, default to enumerated even if
            // no file format argument is supplied.
            Some(_) | None => {
                if let Some(n) = records_per_file {
                    FilenameFormat::Enumerate(overwrite, n)
                } else {
                    FilenameFormat::Single(overwrite)
                }
            }
        };

        let progress = {
            let (pb, chars, template) = if let Some(n) = records_per_file {
                (
                    ProgressBar::new(n),
                    "•• ",
                    "{spinner:.yellow} Writing {msg:.dim.bold} {pos:>7.dim}/{len:.dim} {elapsed_precise:.dim} [{bar:40.yellow/green}] ({eta:.dim})",
                )
            } else {
                (
                    ProgressBar::new(::std::u64::MAX),
                    " • ",
                    "{spinner:.yellow} Writing {msg:.dim.bold} {pos:>7.dim} {elapsed_precise:.dim}",
                )
            };
            pb.set_style(
                ProgressStyle::default_bar()
                    .template(template)
                    .progress_chars(chars),
            );
            pb
        };

        Ok(FileWriter {
            current_filename: filename.clone(),
            file_size: 0,
            filename: filename,
            filename_format: filename_format,
            format: format,
            index: 0,
            progress: progress,
            writer: None,
        })
    }

    fn next_file(&self) -> Result<PathBuf> {
        match self.filename_format {
            FilenameFormat::Single(overwrite) => {
                if self.filename.exists() && !overwrite {
                    Err(
                        format!("{:?} exists. Use overwrite flag to force!", self.filename).into(),
                    )
                } else {
                    Ok(self.filename.clone())
                }
            }
            FilenameFormat::Enumerate(_overwrite, _) => {
                if self.filename.as_path().is_dir() {
                    return Err(
                        format!("Output file {:?} is a directory", self.filename).into(),
                    );
                }

                let dir = self.filename.parent().unwrap_or_else(|| Path::new(""));
                if !dir.is_dir() {
                    DirBuilder::new().recursive(true).create(dir).chain_err(
                        || "Failed to create outfile parent directory",
                    )?
                }

                let next = |index| -> Result<PathBuf> {
                    let mut name = self.filename.clone();
                    name = PathBuf::from(format!(
                        "{}-{:03}",
                        name.file_stem().ok_or("Invalid path")?.to_str().ok_or(
                            "Invalid path",
                        )?,
                        index
                    ));
                    if let Some(extension) = self.filename.extension() {
                        name.set_extension(extension);
                    }
                    Ok(dir.join(name))
                };

                for index in 0.. {
                    let n = next(index)?;
                    if !n.exists() {
                        return Ok(n);
                    }
                }

                unreachable!("Could not find a file - this is proably a bug here...")
            }
            FilenameFormat::Date(overwrite, _) => {
                // If the overwrite flag is set the files are
                // enumerated from the first one to get nice
                // aligned filenames.
                let mut e: Option<u32> = if overwrite { None } else { Some(0) };

                loop {
                    let dir = self.filename.parent().unwrap_or_else(|| Path::new(""));
                    if !dir.is_dir() {
                        DirBuilder::new().recursive(true).create(dir).chain_err(
                            || {
                                format!(
                                    "Failed to create outfile parent directory: {}",
                                    dir.display()
                                )
                            },
                        )?
                    }

                    #[cfg(not(windows))]
                    let now = strftime("%F-%T", &now())?;
                    #[cfg(windows)]
                    let now = strftime("%F-%H_%M_%S", &now())?;
                    let enumeration = e.map(|a| format!("-{:03}", a)).unwrap_or_else(
                        || "".to_owned(),
                    );
                    let filename = self.filename
                        .file_name()
                        .ok_or("Invalid path")?
                        .to_str()
                        .ok_or("Invalid path")?;
                    let candidate = PathBuf::from(format!("{}{}_{}", now, enumeration, filename));
                    let candidate = dir.join(candidate);
                    if !overwrite && candidate.exists() {
                        e = Some(e.unwrap_or(0) + 1);
                        continue;
                    } else {
                        return Ok(candidate);
                    }
                }
            }
        }
    }

    fn write(&mut self, record: &Record) -> Result<()> {
        match self.writer {
            Some(ref mut writer) => {
                writer.write(record, self.index)?;
                self.index += 1;
            }
            None => {
                self.current_filename = self.next_file()?;
                let mut writer = match self.format {
                    Format::Csv | Format::Json | Format::Raw => {
                        Textfile::new(&self.current_filename, &self.format)? as Box<Writer>
                    }
                    Format::Html => Html::new(&self.current_filename, &self.format)? as Box<Writer>,
                    Format::Human => panic!("Unsupported format human in output file"),
                };
                let message = format!("Writing {}", self.current_filename.display());
                self.progress.set_message(&message);
                writer.write(record, self.index)?;
                self.index += 1;
                self.writer = Some(writer);
            }
        }

        self.file_size += 1;
        self.progress.set_position(self.file_size);

        match self.filename_format {
            FilenameFormat::Enumerate(_, n) |
            FilenameFormat::Date(_, n) => {
                if self.file_size >= n {
                    self.flush()
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }

    fn flush(&mut self) -> Result<()> {
        if let Some(ref mut writer) = self.writer {
            writer.flush()?;
        }
        self.progress.set_style(
            ProgressStyle::default_bar().template(
                "{msg:.dim.bold}",
            ),
        );
        self.progress.finish_with_message(&format!(
            "Finished dumping {} records.",
            self.index
        ));
        self.file_size = 0;
        self.writer = None;
        Ok(())
    }
}

impl Sink for FileWriter {
    type SinkItem = Option<Record>;
    type SinkError = Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        if let Some(record) = item {
            if let Err(e) = self.write(&record) {
                return Err(e);
            }
        }
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}
