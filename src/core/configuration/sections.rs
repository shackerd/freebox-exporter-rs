use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct CoreConfiguration {
    pub data_directory: Option<String>,
    pub port: Option<u16>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ApiConfiguration {
    pub mode: Option<String>,
    pub refresh: Option<u64>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct MetricsConfiguration {
    pub connection: Option<bool>,
    pub system: Option<bool>,
    pub lan: Option<bool>,
    pub lan_browser: Option<bool>,
    pub switch: Option<bool>,
    pub wifi: Option<bool>,
    pub dhcp: Option<bool>,
    pub prefix: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LogConfiguration {
    pub level: Option<String>,
    pub retention: Option<usize>,
}
