use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Debug)]
pub struct ConnectionStatus {
    #[serde(alias = "type")]
    pub _type: Option<String>,
    pub rate_down: Option<i64>,
    pub bytes_up: Option<i64>,
    pub rate_up: Option<i64>,
    pub bandwidth_up: Option<i64>,
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub bandwidth_down: Option<i64>,
    pub state: Option<String>,
    pub bytes_down: Option<i64>,
    pub media: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ConnectionConfiguration {
    pub ping: Option<bool>,
    pub is_secure_pass: Option<bool>,
    pub remote_access_port: Option<u16>,
    pub remote_access: Option<bool>,
    pub wol: Option<bool>,
    pub adblock: Option<bool>,
    pub adblock_not_set: Option<bool>,
    pub api_remote_access: Option<bool>,
    pub allow_token_request: Option<bool>,
    pub remote_access_ip: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ConnectionIpv6Delegation {
    pub prefix: Option<String>,
    pub next_hop: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ConnectionIpv6Configuration {
    pub ipv6_enabled: Option<bool>,
    pub delegations: Option<Vec<ConnectionIpv6Delegation>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ConnectionFtth {
    pub sfp_has_power_report: Option<bool>,
    pub sfp_has_signal: Option<bool>,
    pub sfp_model: Option<String>,
    pub sfp_vendor: Option<String>,
    pub sfp_pwr_tx: Option<i64>,
    pub sfp_pwr_rx: Option<i64>,
    pub link: Option<bool>,
    pub sfp_alim_ok: Option<bool>,
    pub sfp_serial: Option<String>,
    pub sfp_present: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct XdslStatus {
    pub status: Option<String>,
    pub protocol : Option<String>,
    pub modulation: Option<String>,
    pub uptime: Option<u32>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct XdslInfo {
    pub status : Option<XdslStatus>,
    pub down: Option<XdslStats>,
    pub up: Option<XdslStats>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct XdslStats {
    pub maxrate: Option<i64>,
    pub rate: Option<i64>,
    pub snr: Option<i16>,
    pub attn: Option<i16>,
    pub fec: Option<i32>,
    pub crc: Option<i32>,
    pub hec: Option<i32>,
    pub es: Option<u32>,
    pub ses: Option<i32>,
    pub rxmt: Option<i32>,
    pub rxmt_uncorr: Option<i32>,
    pub rxmt_corr: Option<i32>,
    pub rtx_tx: Option<i32>,
    pub rtx_c: Option<i32>,
    pub rtx_uc: Option<i32>,
    // pub phyr: Option<bool>,
    // pub ginp: Option<bool>,
    // pub nitro: Option<bool>,
}