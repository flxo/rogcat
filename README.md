[![CI](https://img.shields.io/github/actions/workflow/status/flxo/rogcat/ci.yml?branch=main)](https://github.com/flxo/rogcat/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/rogcat.svg)](https://crates.io/crates/rogcat)
[![Release](https://img.shields.io/github/release/flxo/rogcat.svg)](https://github.com/flxo/rogcat/releases)
[![License](https://img.shields.io/github/license/flxo/rogcat.svg)](https://github.com/flxo/rogcat/blob/master/LICENSE)

# rogcat

...is a `adb logcat` wrapper. The `Android Debugging Bridge` (`adb`) is the default way to interact with a Android
device during development. The `logcat` subcommand of `adb` allows access to `Androids` internal log buffers. `rogcat`
tries to give access to those logs in a convenient way including post processing capabilities. The main feature probably
is a painted and reformatted view. `rogcat` can read logs from

- running `adb logcat` (default)
- a custom command (`stdout`, `stderr`)
- one or multiple files
- `stdin`
- connect to TCP port
- A SocketCAN CAN device (Linux only)

The processing steps within a `rogcat` run include parsing of the input stream and applying filters (if provided).
`rogcat` comes with a set of implemented in and output formats:

- `csv:` Comma separated values
- `raw:` Record (line) as captured
- `html:` A static single page html with a static table. This option cannot be used as input format. The page layout needs some love...
- `human:` A human friendly colored column based format. See screenshot
- `json:` Single line JSON

Except the `human` and `html` format the output of `rogcat` is parseable by `rogcat`.

![Screenshot](/screenshot.png)

## Examples

The following examples show a subset of `rogcat's` features. _Please read `--help`!_

### Live

Capture logs from a connected device and display unconditionally. Unless configurated otherwise, `Rogcat` runs `adb logcat -b main -b events -b crash -b kernel`:

`rogcat`

Write captured logs to `testrun.log`:

`rogcat -o testrun.log`

Write captured logs to file `./trace/testrun-XXX.log` by starting a new file every 1000 lines. If `./trace` is not present
it is created:

`rogcat -o ./trace/testrun.log -n 1000` or `rogcat -o ./trace/testrun.log -n 1k`

### stdin

Process `stdout` and `stderr` of `command`:

`rogcat command` or `command | rogcat -`

### Filter

Display logs from `adb logcat` and filter on records where the tag matches `^ABC.*` along with _not_ `X` and the message includes `pattern`:

`rogcat -t "^ADB.*" -t \!X -m pattern`

The Read all files matching `trace*` in alphanumerical order and dump lines matching `hmmm` to `/tmp/filtered`:

`rogcat -i trace* -m hmmm  -o /tmp/filtered`

Check the `--message` and `--highlight` options in the helptext.

### TCP

To connect via TCP to some host run something like:

`rogcat tcp://traceserver:1234`

### SocketCAN

To open a SocketCAN device and read frames run:

`rogcat can://can0`

SocketCAN is a Linux only thing.

### Bugreport

Capture a `Android` bugreport. This only works for `Android` version prior 7:

`rogcat bugreport`

Capture a `Android` bugreport and write (zipped) to `bugreport.zip`:

`rogcat bugreport -z bugreport.zip`

### Log

Write message "some text" into the device log buffer (e.g annotations during manual testing):

`rogcat log "some text"`

Set level and tag or read from `stdin`:

```sh
rogcat-log
Add log message(s) log buffer

USAGE:
    rogcat log [OPTIONS] [MESSAGE]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -l, --level <LEVEL>    Log on level [values: trace, debug, info, warn, error, fatal, assert, T, D, I, W, E, F, A]
    -t, --tag <TAG>        Log tag

ARGS:
    <MESSAGE>    Log message. Pass "-" to capture from stdin'
```

## Fuchsia

`rogcat` can be used to read logs from a `Fuchsia` device. Use the `--fx` switch to run `ffx` instead of `adb logcat`.
The default `ffx` command is `ffx log --no-color --severity debug`.

Of course `ffx` can be invoked manually with eg. `ffx log --no-color | rogcat -` or `rogcat "ffx log --no-color"`.

## Installation

Building `rogcat` requires Rust 2018 edition:

```sh
cargo install --path .
```

or grab one of the [binary releases](https://github.com/flxo/rogcat/releases) on the GitHub page.

On Debian based systems the package `libudev-dev` (and it's dependencies) is required for building.

## Configuration

When `rogcat` runs without any command supplied it defaults to running `adb logcat -b all`. The following options
can be overwritten in the `rogcat` config file `config.toml`. The location of the config file is platform specific:

- MacOS: `$HOME/Library/Preferences/rogcat/config.toml`
- Linux: `$HOME/.config/rogcat/config.toml`
- Windows: `%HOME%/AppData/Roaming/rogcat/config.toml`

### Restart

By default `rogcat` restarts `adb logcat` when that one exits. This is intentional behavior to make `rogcat` reconnect
on device power cycles or disconnect/reconnects. A `Windows 7` bug prevents `rogcat` from restarting `adb`. Place
`restart = false` in the configuration file mentioned above to make `rogcat` exit when `adb` exits.

### Buffer

The default behavior of `rogcat` is to dump `all` logcat buffers. This can be overwritten by selecting specific buffers in
the `rogcat` configuration file. e.g:

```toml
buffer = ["main", "events"]
```

### Terminal settings

Some parameters of the `human` format are adjustable via the config file:

```sh
terminal_bright_colors = false
terminal_color = never
terminal_hide_timestamp = true
terminal_process_width_max = 16
terminal_thread_width_max = 16
terminal_no_dimm = true
terminal_show_date = false
terminal_tag_width = 20
```

## Profiles

Optionally `rogcat` reads a (`toml` formated) configuration file if present. This configuration may include tracing profiles
('-p') and settings. The possible options in the configuration file are a subset of the command line options. The configuration
file is read from the location set in the environment variable `ROGCAT_PROFILES` or a fixed pathes depending on your OS:

- MacOS: `$HOME/Library/Preferences/rogcat/profiles.toml`
- Linux: `$HOME/.config/rogcat/profiles.toml`
- Windows: `%HOME%/AppData/Roaming/rogcat/profiles.toml`

The environment variable overrules the default path. See `rogcat profiles --help` or `rogcat profiles --examples`.

Example:

```toml
[profile.a]
comment = "Messages starting with A or a"
message_case_insensitive = ["^A.*"]

[profile.B]
comment = "Messages starting with B"
message = ["^B.*"]

[profile.ABC]
extends = ["A", "B"]
comment = "Profiles A, B plus the following filter (^C.*)"
message = ["^C.*"]

[profile."Comments are optional"]
tag = ["rogcat"]

[profile.complex]
comment = "Profiles can be complex. This one is probably very useless."
highlight = ["blah"]
message = ["^R.*", "!^A.*", "!^A.*"]
tag = ["b*", "!adb"]

[profile."W hitespace"]
comment = "Profile names can contain whitespaces. Quote on command line..."

[profile.rogcat]
comment = "Only tag \"rogcat\""
tag = ["^rogcat$"]

[profile.default]
comment = "Default profile"
```

To check your setup, run `rogcat profiles --list` and select a profile for a run by passing the `-p/--profile` option.

You can create a special profile named `default` which will be used when no other profile is selected on the command line.

## Usage

```sh
rogcat 0.4.8-pre
Felix Obenhuber <felix@obenhuber.de>
A 'adb logcat' wrapper and log processor. Your config directory is "/home/felix/.config/rogcat".

USAGE:
    rogcat [FLAGS] [OPTIONS] [COMMAND] [SUBCOMMAND]

FLAGS:
        --bright-colors     Use intense colors in terminal output
    -d, --dump              Dump the log and then exit (don't block)
        --ffx               Use ffx log instead of adb logcat
        --help              Prints help information
        --hide-timestamp    Hide timestamp in terminal output
    -L, --last              Dump the logs prior to the last reboot
        --no-dimm           Use white as dimm color
        --overwrite         Overwrite output file if present
        --restart           Restart command on exit
        --show-date         Show month and day in terminal output
    -V, --version           Prints version information

OPTIONS:
    -b, --buffer <buffer>...
            Select specific logd buffers. Defaults to main, events, kernel and crash

        --color <color>                            Terminal coloring option [possible values: auto, always, never]
    -s, --serial <dev>                             Forwards the device selector to adb
    -a, --filename-format <filename-format>
            Select a format for output file names. By passing 'single' the filename provided with the '-o' option is
            used (default).'enumerate' appends a file sequence number after the filename passed with '-o' option
            whenever a new file is created (see 'records-per-file' option). 'date' will prefix the output filename with
            the current local date when a new file is created [possible values: single, enumerate, date]
    -f, --filter <filter>...                       Regex filter on tag, pid, thread and message.
        --format <format>
            Output format. Defaults to human on stdout and raw on file output [possible values: csv, html, human, json,
            raw]
    -H, --head <head>                              Read n records and exit
    -h, --highlight <highlight>...
            Highlight messages that match this pattern in RE2. The prefix '!' inverts the match

    -i, --input <input>...
            Read from file instead of command. Use 'serial://COM0@115200,8N1 or similiar for reading a serial port

    -l, --level <level>
            Minimum level [possible values: trace, debug, info, warn, error, fatal, assert, T, D, I, W, E, F, A]

    -m, --message <message>...                     Message filters in RE2. The prefix '!' inverts the match
    -M, --Message <message-case-insensitive>...    Same as -m/--message but case insensitive
    -o, --output <output>                          Write output to file
    -p, --profile <profile>                        Select profile
    -P, --profiles-path <profiles-path>            Manually specify profile file (overrules ROGCAT_PROFILES)
    -n, --records-per-file <records-per-file>      Write n records per file. Use k, M, G suffixes or a plain number
    -t, --tag <tag>...                             Tag filters in RE2. The prefix '!' inverts the match
    -T, --Tag <tag-case-insensitive>...            Same as -t/--tag but case insensitive
        --tail <tail>                              Dump only the most recent <COUNT> lines (implies --dump)

ARGS:
    <COMMAND>    Optional command to run and capture stdout and stdderr from. Pass "-" to d capture stdin'. If
                 omitted, rogcat will run "adb logcat -b all" and restarts this commmand if 'adb' terminates

SUBCOMMANDS:
    bugreport      Capture bugreport. This is only works for Android versions < 7.
    clear          Clear logd buffers
    completions    Generates completion scripts
    devices        List available devices
    help           Prints this message or the help of the given subcommand(s)
    log            Add log message(s) log buffer
```

## Licensing

See `LICENSE` for details.
