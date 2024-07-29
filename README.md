# freebox-exporter-rs

[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2Fshackerd%2Ffreebox-exporter-rs.svg?type=shield)](https://app.fossa.com/projects/git%2Bgithub.com%2Fshackerd%2Ffreebox-exporter-rs?ref=badge_shield)

Yet another [Freebox API](https://dev.freebox.fr/sdk/os/) exporter for Prometheus! This project is actually under development, feel free to contribute!

> [!IMPORTANT]
> **Disclaimer:** This project is unofficial and is **not affiliated with Illiad Group**

## Features

* Freebox API exposition (WIP)
* Caching & background update
* Customizable data directory
* Can specify configuration file
* File/console logging

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
  -v, --verbosity <VERBOSITY>
  -h, --help                                     Print help
  -V, --version                                  Print version
```

## Building, debugging, configuring project

### Clone project

``` bash
git clone https://github.com/shackerd/freebox-exporter-rs.git && cd freebox-exporter-rs
```

### Configuration

``` toml
[api]
# Acceptable values: "router" or "bridge"
# These values will determine whether use discovery or not, see: https://github.com/shackerd/freebox-exporter-rs/issues/2#issuecomment-2234856496
# * discovery on:
#   * Traffic will be using host like xxxxxxxx.fbxos.fr
#   * FQDN resolves to your public IP address.
#   * However, you do not need to activate remote_access from local network to get API working.
# * discovery off:
#   * Traffic will be using host mafreebox.freebox.fr
#   * FQDN resolves to a public IP address (not yours), which allows you to reach your freebox API even if it's set to bridge mode.
# Remark: setting bridge as value works for both freebox mode router & bridge
mode = "bridge"

# Refresh wait interval in seconds, application will send requests to the freebox host on each refresh iteration
# This does not affect prometheus scrap agents, application will use cached values between calls
# Remark:
#   more you set API exposition (c.f: [publish] section) more requests will be sent,
#   setting a too low interval between refreshs could lead to request rate limiting from freebox host
refresh = 5

[publish]
# Exposes connection API
connection = true
# Exposes settings API
settings = false
# Exposes contacts API
contacts = true
# Exposes calls API
calls = true
# Exposes explorer API
explorer = true
# Exposes downloader API
downloader = true
# Exposes parental API
parental = true
# Exposes pvr API
pvr = true

[core]
# Specify where to store data for exporter such as APP_TOKEN, logs, etc.
data_directory = "."
# Specify which TCP port to listen to, for the /metrics HTTP endpoint
port = 9102

[log]
# Specify which log level to use
# Acceptable values :
#   * "Off"     : A level lower than all log levels
#   * "Error"   : Corresponds to the `Error` log level
#   * "Warn"    : Corresponds to the `Warn` log level
#   * "Info"    : Corresponds to the `Info` log level
#   * "Debug"   : Corresponds to the `Debug` log level
#   * "Trace"   : Corresponds to the `Trace` log level
level = "Info"
# Specify how long application should keep compressed log files, value is in days
retention = 31
```

### Run debug configuration, assuming application is registered on Freebox host

``` bash
cargo run serve
```

> [!TIP]
> You can change output log level by specifying verbosity, such as `cargo run -- -v Debug serve`

### Register application if application is not registered on Freebox host

``` bash
cargo run register
```

### Running tests

> [!TIP]
> This project uses [Mockoon](https://mockoon.com/) for API mocking, you need to install GUI or CLI and start it with `api-mock.json` file.

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
