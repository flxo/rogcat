[![Build Status](https://travis-ci.org/flxo/rogcat.svg)](https://travis-ci.org/flxo/rogcat)
[![Build status](https://ci.appveyor.com/api/projects/status/ng8npy7ym6l8lsy0?svg=true)](https://ci.appveyor.com/project/flxo/rogcat)
# rogcat


A ``adb logcat`` wrapper with colors and filter and output options written in `Rust`

![Screenshot](/screenshot.png)

## Usage

```
rogcat 0.2.5-pre
Felix Obenhuber <felix@obenhuber.de>
A 'adb logcat' wrapper

USAGE:
    rogcat [FLAGS] [OPTIONS] [COMMAND] [SUBCOMMAND]

FLAGS:
        --no-timestamp           No timestamp in terminal output
        --overwrite              Overwrite output file if present
        --shorten-tags           Shorten tag by removing vovels if too long
        --show-date              Show date in terminal output
        --show-time-diff         Show time diff of tags in terminal output
    -c, --clear                  Clear (flush) the entire log and exit
    -g, --get-ringbuffer-size    Get the size of the log's ring buffer and exit
        --help                   Prints help information
    -S, --output-statistics      Output statistics
    -r, --restart                Restart command on exit
    -V, --version                Prints version information

OPTIONS:
    -a, --filename-format <FILENAME_FORMAT>      Select format for output file names. By passing 'single' the filename provided with the '-o'
                                                 option is used. 'enumerate' will append a file sequence number after the filename passed
                                                 with '-o' option whenever a new file is created (see 'records-per-file' option). 'date' will
                                                 prefix the output filename with the current local date when a new file is created [values:
                                                 single, enumerate, date]
    -f, --file-format <FILE_FORMAT>              Select format for output files [values: csv, html, raw]
    -l, --level <LEVEL>                          Minimum level [values: trace, debug, info, warn, error, fatal, assert, T, D, I, W, E, F, A]
    -n, --records-per-file <RECORDS_PER_FILE>    Write n records per file. Use k, M, G suffixes or a plain number
    -e, --terminal-format <TERMINAL_FORMAT>      Select format for stdout [default: human]  [values: human, raw, csv]
    -h, --highlight <HIGHLIGHT>...               Highlight pattern in RE
    -i, --input <INPUT>...                       Read from file instead of command. Use 'serial://COM0@11520,8N1 or similiar for reading a
                                                 serial port
    -m, --message <MSG>...                       Message filters in RE2. The prefix ! inverts the match
    -o, --output <OUTPUT>                        Write to file and stdout
    -t, --tag <TAG>...                           Tag filters in RE2. The prefix ! inverts the match

ARGS:
    <COMMAND>    Optional command to run and capture stdout from. Pass "-" to capture stdin'. If omitted, rogcat will run "adb logcat -b
                 all

SUBCOMMANDS:
    bugreport      Capture bugreport
    completions    Generates completion scripts for your shell
    devices        Show list of available devices
    help           Prints this message or the help of the given subcommand(s)
    log            Add log message log buffer
```


```
rogcat-log 
Add log message log buffer

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
rogcat-bugreport 
Capture bugreport

USAGE:
    rogcat bugreport [FLAGS] [FILE]

FLAGS:
        --overwrite    Overwrite report file if present
    -z, --zip          Zip report
    -h, --help         Prints help information
    -V, --version      Prints version information

ARGS:
    <FILE>    Output file name - defaults to <date>-bugreport

```

```
rogcat-bugreport 
Capture bugreport

USAGE:
    rogcat bugreport [FLAGS] [FILE]

FLAGS:
        --overwrite    Overwrite report file if present
    -z, --zip          Zip report
    -h, --help         Prints help information
    -V, --version      Prints version information

ARGS:
    <FILE>    Output file name - defaults to <date>-bugreport
```

## Bugs

There are plenty. Report them on GitHub, or - even better - open a pull request.

## Licensing

Rogcat is open source software; see ``COPYING`` for details.
