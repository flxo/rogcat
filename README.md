[![Build Status](https://travis-ci.org/flxo/rogcat.png)](https://travis-ci.org/flxo/rogcat)
# rogcat


A ``adb logcat`` wrapper with colored output and filter capabilities.

![Screenshot](/screenshot.png)

## Usage

Without any argument rogcat starts up adb logcat and displays the output in a nice way. 
Not all errors are wrapped with a nice and readable explanation, so please be prepared to see something strange.

```
rogcat 0.2.0
Felix Obenhuber <felix@obenhuber.de>
A logcat (and others) wrapper

USAGE:
    rogcat [FLAGS] [OPTIONS] [COMMAND] [SUBCOMMAND]

FLAGS:
        --no-color             Monochrome output
        --no-tag-shortening    Disable shortening of tag
        --no-time-diff         Disable tag time difference
    -S                         Output statistics
        --show-date            Disable month and day display
    -c                         Clear (flush) the entire log and exit
        --csv                  Write csv like format instead of raw
    -g                         Get the size of the log's ring buffer and exit
    -h, --help                 Prints help information
        --restart              Restart command on exit
    -V, --version              Prints version information

OPTIONS:
    -a, --adb <ADB BINARY>    Path to adb
    -i, --input <INPUT>       Read from file instead of command
    -l, --level <LEVEL>       Minumum level
    -m, --msg <FILTER>...     Message filters in RE2
    -o, --output <OUTPUT>     Write to file instead to stdout
    -t, --tag <FILTER>...     Tag filters in RE2

ARGS:
    <COMMAND>    Optional command to run and capture stdout. Use -- to read stdin.

SUBCOMMANDS:
    completions    Generates completion scripts for your shell
    help           Prints this message or the help of the given subcommand(s)
```

## Todos

* Configurable terminal
* Error messages instead of raw panics
* Extension of in and output formats
* Optimization of format implementation
* Ring buffer like file output
* ...

## Bugs

There are plenty. Report them on GitHub, or - even better - open a pull request.

## Licensing

Rogcat is open source software; see ``COPYING`` for details.

## Author

Rogcat was written by Felix Obenhuber
