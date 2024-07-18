# freebox-exporter-rs

[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2Fshackerd%2Ffreebox-exporter-rs.svg?type=shield)](https://app.fossa.com/projects/git%2Bgithub.com%2Fshackerd%2Ffreebox-exporter-rs?ref=badge_shield)

Yet another [Freebox API](https://dev.freebox.fr/sdk/os/) exporter for Prometheus! This project is actually under development, feel free to contribute!

**Disclaimer:** This project is unofficial and is **not affiliated with Illiad Group**

## Features

* Freebox API exposition (WIP)
* Caching & background update
* Customizable data directory
* Can specify configuration file

## Current API progress

* &#10134; Authentication
  * &#9989; Register: **100%**
  * &#9989; Login: **100%**
* &#10134; Configuration
  * &#10134; Connection
    * &#9989; Status: **100%**
    * &#9989; Configuration: **100%**
    * &#9989; Configuration IPV6: **100%**
    * &#10060; xDSL: 0%
    * &#9989; FFTH: **100%**
    * &#10060; DynDNS: 0%
  * &#10060; Lan: 0%
  * &#10060; Lan Browser: 0%
  * &#10060; Freeplug: 0%
  * &#10060; DHCP: 0%
  * &#10060; Ftp: 0%
  * &#10060; NAT: 0%
  * &#10060; Port Forwarding: 0%
  * &#10060; Incoming port configuration: 0%
  * &#10060; UPnP IGD: 0%
  * &#10060; LCD: 0%
  * &#10060; Network Share: 0%
  * &#10060; UPnP AV: 0%
  * &#10060; Switch: 0%
  * &#10060; Wi-Fi: 0%
  * &#10060; System: 0%
  * &#10060; VPN Server: 0%
  * &#10060; VPN Client: 0%

* &#10134; Download
  * &#10060; Stats: 0%
  * &#10060; Files: 0%
  * &#10060; Trackers: 0%
  * &#10060; Peers: 0%
  * &#10060; Pieces: 0%
  * &#10060; Blacklist: 0%
  * &#10060; Feeds: 0%
  * &#10060; Configuration: 0%

* &#10134; File
  * &#10060; System: 0%
  * &#10060; Sharing Link: 0%
  * &#10060; Upload: 0%

* &#10134; Air Media
  * &#10060; Configuration: 0%
  * &#10060; Receivers: 0%

* &#10060; Storage: 0%
* &#10060; Parental filter: 0%
* &#10134; PVR
  * &#10060; Programmed records: 0%
  * &#10060; Finished records: 0%
  * &#10060; Storage media: 0%

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

## Building, debugging, configuring project

### Clone project

``` bash
git clone https://github.com/shackerd/freebox-exporter-rs.git && cd freebox-exporter-rs
```

### Configuration

Actually `data_dir` folder is set to my home directory, you need to change it in configuration file (`conf.toml`) to get it working properly.

``` toml
[api]
host = "mafreebox.freebox.fr"
port = 443
refresh_interval_secs = 5
use_discovery = false
expose = { connection = true,  settings = true, contacts = true, calls = true, explorer = true, downloader = true, parental = true, pvr = true }

[core]
data_dir = "<your path>"
port = 9102
```

### Run debug configuration, assuming application is registered on Freebox host

``` bash
cargo run serve
```

### Register application if application is not registered on Freebox host

``` bash
cargo run register
```

### Running tests

This project uses [Mockoon](https://mockoon.com/) for API mocking, you need to install GUI or CLI and start it with `api-mock.json` file.

Then run the following command.

``` bash
cargo test
```

### Verify it works

If you changed port in `conf.toml`, update the command line below.

``` bash
curl http://localhost:9102/metrics
```

## License

[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2Fshackerd%2Ffreebox-exporter-rs.svg?type=large)](https://app.fossa.com/projects/git%2Bgithub.com%2Fshackerd%2Ffreebox-exporter-rs?ref=badge_large)

## Support this project

If you want to help, you can contribute or you can still [buy me a coffee](https://buymeacoffee.com/shackerd)!
