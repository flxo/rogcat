[![Build Status](https://travis-ci.org/flxo/rogcat.png)](https://travis-ci.org/flxo/rogcat)
# rogcat


A ``adb logcat`` wrapper with colored output and filter capabilities.

![Screenshot](/screenshot.png)

## Usage

Without any argument rogcat starts up adb logcat and displays the output in a nice way. 
Not all errors are wrapped with a nice and readable explanation, so please be prepared to see something strange.

```
rogcat 0.2.1
Felix Obenhuber <felix@obenhuber.de>
A 'adb logcat' wrapper

USAGE:
    rogcat [FLAGS] [OPTIONS] [COMMAND] [SUBCOMMAND]

FLAGS:
    -S               Output statistics
    -c               Clear (flush) the entire log and exit
        --csv        Write csv format instead
    -g               Get the size of the log's ring buffer and exit
    -h, --help       Prints help information
        --restart    Restart command on exit
    -V, --version    Prints version information

OPTIONS:
    -i, --input <INPUT>      Read from file instead of command. Pass "stdin" to capture stdin
    -l, --level <level>      Minimum level [values: trace, debug, info, warn, error, fatal, assert, T, D, I, W, E, F, A]
    -m, --msg <MSG>...       Message filters in RE2
    -o, --output <OUTPUT>    Write to file and stdout
    -t, --tag <TAG>...       Tag filters in RE2

ARGS:
    <COMMAND>    Optional command to run and capture stdout. Pass "stdin" to capture stdin

SUBCOMMANDS:
    completions    Generates completion scripts for your shell
    help           Prints this message or the help of the given subcommand(s)
```
## Bugs

There are plenty. Report them on GitHub, or - even better - open a pull request.

## Licensing

Rogcat is open source software; see ``COPYING`` for details.

## Author

Rogcat was written by Felix Obenhuber
