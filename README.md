# freebox-export-rs

Yet another [Freebox API](https://dev.freebox.fr/sdk/os/) exporter for Prometheus! This project is actually under development, feel free to contribute!

## Features

* Freebox API exposition (WIP)
* Caching & background update
* Customizable data directory
* Can specify configuration file

## Roadmap

* Expose all Freebox API
* Loggin
* Provide systemd registration
* Provide container support
* Publish to crates.io (cargo install)

## Usage

This project uses `clap` crate you will find usage by using the following command `freebox-exporter-rs -h`

``` text
Usage: freebox-exporter-rs [OPTIONS] <COMMAND>

Commands:
  register
  serve
  revoke
  help      Print this message or the help of the given subcommand(s)

Options:
  -c, --configuration-file <CONFIGURATION_FILE>
  -h, --help                                     Print help
  -V, --version                                  Print version
```
