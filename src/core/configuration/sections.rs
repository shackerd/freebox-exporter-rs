use std::time::Duration;

use serde::Deserialize;

use crate::core::common::http_client_factory::Timeouts;

#[derive(Deserialize, Clone, Debug)]
pub struct CoreConfiguration {
    pub data_directory: Option<String>,
    pub port: Option<u16>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ApiConfiguration {
    pub refresh: Option<u64>,
    pub connect_timeout: Option<u64>,
    pub request_timeout: Option<u64>,
}

impl ApiConfiguration {
    /// Upper bounds of the requests to the box, in seconds in the configuration file
    pub fn timeouts(&self) -> Timeouts {
        let default = Timeouts::default();
        Timeouts {
            connect: self.connect_timeout.map_or(default.connect, Duration::from_secs),
            request: self.request_timeout.map_or(default.request, Duration::from_secs),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct PoliciesConfiguration {
    pub unresolved_station_hostnames: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CapabilitiesConfiguration {
    pub connection: Option<bool>,
    pub system: Option<bool>,
    pub lan: Option<bool>,
    pub lan_browser: Option<bool>,
    pub switch: Option<bool>,
    pub wifi: Option<bool>,
    pub dhcp: Option<bool>,
    pub nas: Option<bool>,
    pub download: Option<bool>,
    pub vm: Option<bool>,
    pub raid: Option<bool>,
    pub vpn: Option<bool>,
    pub ddns: Option<bool>,
    pub prefix: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LogConfiguration {
    pub level: Option<String>,
    pub retention: Option<usize>,
}
