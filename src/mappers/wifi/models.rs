use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WifiConfig {
    pub enabled: Option<bool>,
    pub power_saving: Option<bool>,
    pub expected_phys: Option<Vec<ExpectedPhy>>,
    pub mac_filter_state: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExpectedPhy {
    pub band: Option<String>,
    pub phy_id: Option<i16>,
    pub detected: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Station {
    pub mac: Option<String>,
    pub last_rx: Option<LastRxTx>,
    pub last_tx: Option<LastRxTx>,
    pub tx_bytes: Option<u64>,
    pub tx_rate: Option<u64>,
    pub rx_bytes: Option<u64>,
    pub rx_rate: Option<u64>,
    pub id: Option<String>,
    pub bssid: Option<String>,
    pub flags: Option<Flags>,
    pub host: Option<Host>,
    pub signal: Option<i8>,
    pub inactive: Option<i64>,
    pub state: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccessPointCapabilities {
    pub band: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccessPoint {
    pub name: Option<String>,
    pub id: Option<u8>,
    pub config: Option<AccessPointCapabilities>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LastRxTx {
    pub bitrate: Option<u64>,
    pub mcs: Option<i64>,
    pub shortgi: Option<bool>,
    pub vht_mcs: Option<i64>,
    pub width: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Flags {
    pub vht: Option<bool>,
    pub legacy: Option<bool>,
    pub authorized: Option<bool>,
    pub ht: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Host {
    pub l2ident: Option<L2Ident>,
    pub l3connectivities: Option<Vec<L3Connectivities>>,
    pub names: Option<Vec<HostName>>,
    pub active: Option<bool>,
    pub last_activity: Option<i64>,
    pub last_time_reachable: Option<i64>,
    pub vendor_name: Option<String>,
    pub primary_name: Option<String>,
    pub primary_name_manual: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HostName {
    pub name: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct L2Ident {
    pub id: Option<String>,
    pub r#type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct L3Connectivities {
    pub addr: Option<String>,
    pub af: Option<String>,
    pub active: Option<bool>,
    pub reachable: Option<bool>,
    pub last_activity: Option<i64>,
    pub last_time_reachable: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChannelSurveyHistory {
    pub timestamp: Option<u64>,
    pub busy_percent: Option<u8>,
    pub tx_percent: Option<u8>,
    pub rx_bss_percent: Option<u8>,
    pub rx_percent: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NeighborsAccessPoint {
    pub capabilities: Option<NeighborsAccessPointFlags>,
    pub channel: Option<u8>,
    // pub channel_width: Option<u8>,
    pub ssid: Option<String>,
    pub bssid: Option<String>,
    pub signal: Option<i8>,
    pub secondary_channel: Option<u8>,
    pub band: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NeighborsAccessPointFlags {
    pub vht: Option<bool>,
    pub legacy: Option<bool>,
    pub he: Option<bool>,
    pub ht: Option<bool>,
    pub eht: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChannelUsage {
    pub band: Option<String>,
    pub noise_level: Option<i8>,
    pub channel: Option<u8>,
    pub rx_busy_percent: Option<u8>,
}
