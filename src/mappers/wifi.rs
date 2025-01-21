use std::usize;

use async_trait::async_trait;
use chrono::Duration;
use prometheus_exporter::prometheus::{register_int_gauge_vec, IntGaugeVec};
use serde::{Deserialize, Serialize};

use crate::core::common::{AuthenticatedHttpClientFactory, FreeboxResponse, FreeboxResponseError};

use super::MetricMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WifiConfig {
    pub enabled: Option<bool>,
    pub mac_filter_state: Option<String>,
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

pub struct WifiMetricMap {
    factory: AuthenticatedHttpClientFactory,
    history_ttl: Duration,
    busy_percent_gauge: IntGaugeVec,
    tx_percent_gauge: IntGaugeVec,
    rx_percent_gauge: IntGaugeVec,
    rx_bss_percent: IntGaugeVec,
    station_active_gauge: IntGaugeVec,
    station_rx_bitrate_gauge: IntGaugeVec,
    station_rx_mcs_gauge: IntGaugeVec,
    station_rx_shortgi_gauge: IntGaugeVec,
    station_rx_vht_mcs_gauge: IntGaugeVec,
    station_rx_width_gauge: IntGaugeVec,
    station_rx_bytes_gauge: IntGaugeVec,
    station_rx_rate_gauge: IntGaugeVec,
    station_tx_bitrate_gauge: IntGaugeVec,
    station_tx_mcs_gauge: IntGaugeVec,
    station_tx_shortgi_gauge: IntGaugeVec,

    station_tx_vht_mcs_gauge: IntGaugeVec,
    station_tx_width_gauge: IntGaugeVec,
    station_tx_bytes_gauge: IntGaugeVec,
    station_tx_rate_gauge: IntGaugeVec,
    station_signal_gauge: IntGaugeVec,
    station_inactive_gauge: IntGaugeVec,
    station_state_gauge: IntGaugeVec,
    station_flags_gauge: IntGaugeVec,
    station_last_activity_gauge: IntGaugeVec,
    station_last_time_reachable_gauge: IntGaugeVec,
    neighbors_access_point_gauge: IntGaugeVec,
    channel_usage_gauge: IntGaugeVec,
}

impl WifiMetricMap {
    pub fn new(
        factory: AuthenticatedHttpClientFactory,
        prefix: String,
        history_ttl: Duration,
    ) -> Self {
        let prfx: String = format!("{prefix}_wifi");
        Self {
            factory,
            history_ttl,
            busy_percent_gauge: register_int_gauge_vec!(
                format!("{prfx}_busy_percent"),
                format!("{prfx}_busy_percent"),
                &["ap", "name", "band"]
            )
            .expect(&format!("cannot create {prfx}_busy_percent gauge")),
            rx_bss_percent: register_int_gauge_vec!(
                format!("{prfx}_rx_bss_percent"),
                format!("{prfx}_rx_bss_percent"),
                &["ap", "name", "band"]
            )
            .expect(&format!("cannot create {prfx}_rx_bss_percent gauge")),
            rx_percent_gauge: register_int_gauge_vec!(
                format!("{prfx}_rx_percent"),
                format!("{prfx}_rx_percent"),
                &["ap", "name", "band"]
            )
            .expect(&format!("cannot create {prfx}_rx_percent gauge")),
            tx_percent_gauge: register_int_gauge_vec!(
                format!("{prfx}_tx_percent"),
                format!("{prfx}_tx_percent"),
                &["ap", "name", "band"]
            )
            .expect(&format!("cannot create {prfx}_tx_percent gauge")),
            station_active_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_active"),
                format!("{prfx}_station_active 1 for active"),
                &[
                    "primary_name",
                    "ap_name",
                    "band",
                    "ap_id",
                    "mac",
                    "vendor_name"
                ]
            )
            .expect(&format!("cannot create {prfx}_station_mac gauge")),
            station_rx_bitrate_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_rx_bitrate"),
                format!("{prfx}_station_rx_bitrate"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_rx_bitrate gauge")),

            station_rx_mcs_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_rx_mcs"),
                format!("{prfx}_station_rx_mcs"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_rx_mcs gauge")),

            station_rx_shortgi_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_rx_shortgi"),
                format!("{prfx}_station_rx_shortgi 1 for shortgi enabled"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_rx_shortgi gauge")),

            station_rx_vht_mcs_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_rx_vht_mcs"),
                format!("{prfx}_station_rx_vht_mcs"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_rx_vht_mcs gauge")),

            station_rx_width_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_rx_width"),
                format!("{prfx}_station_rx_width"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_rx_width gauge")),

            station_rx_bytes_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_rx_bytes"),
                format!("{prfx}_station_rx_bytes"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_rx_bytes gauge")),

            station_rx_rate_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_rx_rate"),
                format!("{prfx}_station_rx_rate"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_rx_rate gauge")),

            station_tx_bitrate_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_tx_bitrate"),
                format!("{prfx}_station_tx_bitrate"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_tx_bitrate gauge")),

            station_tx_mcs_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_tx_mcs"),
                format!("{prfx}_station_tx_mcs"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_tx_mcs gauge")),

            station_tx_shortgi_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_tx_shortgi"),
                format!("{prfx}_station_tx_shortgi 1 for shortgi enabled"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_tx_shortgi gauge")),

            station_tx_vht_mcs_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_tx_vht_mcs"),
                format!("{prfx}_station_tx_vht_mcs"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_tx_vht_mcs gauge")),

            station_tx_width_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_tx_width"),
                format!("{prfx}_station_tx_width"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_tx_width gauge")),

            station_tx_bytes_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_tx_bytes"),
                format!("{prfx}_station_tx_bytes"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_tx_bytes gauge")),

            station_tx_rate_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_tx_rate"),
                format!("{prfx}_station_tx_rate"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_tx_rate gauge")),

            station_signal_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_signal"),
                format!("{prfx}_station_signal"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_signal gauge")),

            station_inactive_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_inactive"),
                format!("{prfx}_station_inactive"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_inactive gauge")),

            station_state_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_state"),
                format!("{prfx}_station_state"),
                &[
                    "primary_name",
                    "ipv4",
                    "ap_name",
                    "band",
                    "ap_id",
                    "mac",
                    "state"
                ]
            )
            .expect(&format!("cannot create {prfx}_station_state gauge")),

            station_flags_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_flags"),
                format!("{prfx}_station_flags"),
                &[
                    "primary_name",
                    "ipv4",
                    "ap_name",
                    "band",
                    "ap_id",
                    "mac",
                    "vht",
                    "legacy",
                    "authorized",
                    "ht"
                ]
            )
            .expect(&format!("cannot create {prfx}_station_vht gauge")),
            station_last_activity_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_last_activity"),
                format!("{prfx}_station_last_activity"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_last_activity gauge")),
            station_last_time_reachable_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_last_time_reachable"),
                format!("{prfx}_station_last_time_reachable"),
                &["primary_name", "ipv4", "ap_name", "band", "ap_id", "mac"]
            )
            .expect(&format!(
                "cannot create {prfx}_station_last_time_reachable gauge"
            )),
            neighbors_access_point_gauge: register_int_gauge_vec!(
                format!("{prfx}_neighbors_access_point"),
                format!("{prfx}_neighbors_access_point signal strength"),
                &[
                    "channel",
                    // "channel_width",
                    "ssid",
                    "bssid",
                    "band",
                    "vht",
                    "legacy",
                    "he",
                    "ht",
                    "eht",
                    "secondary_channel",
                ]
            )
            .expect(&format!(
                "cannot create {prfx}_neighbors_access_point gauge"
            )),
            channel_usage_gauge: register_int_gauge_vec!(
                format!("{prfx}_channel_usage"),
                format!("{prfx}_channel_usage noise level"),
                &["band", "channel", "rx_busy_percent"]
            )
            .expect(&format!("cannot create {prfx}_channel_usage gauge")),
        }
    }

    async fn set_channel_survey_history_gauges(
        &self,
        ap: &AccessPoint,
    ) -> Result<(), Box<dyn std::error::Error + Send>> {
        let client = self.factory.create_client().await?;
        let ts = chrono::offset::Local::now().timestamp();
        let root_url = &self.factory.api_url;
        let ap_id = ap.id.as_ref().unwrap().to_string();
        let band = ap.config.as_ref().unwrap().band.as_ref().unwrap();
        let ap_name = match ap.name.as_ref() {
            Some(n) => n,
            None => "unknown",
        };

        let res = client
            .get(format!(
                "{root_url}v4/wifi/ap/{ap_id}/channel_survey_history/{ts}"
            ))
            .send()
            .await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res
            .unwrap()
            .json::<FreeboxResponse<Vec<ChannelSurveyHistory>>>()
            .await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res.unwrap();

        if let None = res.result {
            return Err(Box::new(FreeboxResponseError::new(format!(
                "{root_url} was empty"
            ))));
        }

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        // represents the amount of entries we want to keep between each api call
        let recent = get_recent_channel_entries(
            res.result.as_ref().unwrap(),
            (self.history_ttl.num_milliseconds() / 100) as usize, // each array entry represents 100ms
        );

        let avg_history = calculate_avg_channel_survey_history(&recent);

        self.busy_percent_gauge
            .with_label_values(&[&ap_id, &ap_name, &band])
            .set(avg_history.busy_percent.unwrap() as i64);

        self.tx_percent_gauge
            .with_label_values(&[&ap_id, &ap_name, &band])
            .set(avg_history.tx_percent.unwrap() as i64);

        self.rx_bss_percent
            .with_label_values(&[&ap_id, &ap_name, &band])
            .set(avg_history.rx_bss_percent.unwrap() as i64);

        self.rx_percent_gauge
            .with_label_values(&[&ap_id, &ap_name, &band])
            .set(avg_history.rx_percent.unwrap() as i64);

        Ok(())
    }

    async fn get_stations(
        &self,
        ap: &AccessPoint,
    ) -> Result<Vec<Station>, Box<dyn std::error::Error + Send>> {
        let client = self.factory.create_client().await?;

        let res = client
            .get(format!(
                "{}v4/wifi/ap/{}/stations",
                self.factory.api_url,
                ap.id.unwrap()
            ))
            .send()
            .await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res.unwrap().json::<FreeboxResponse<Vec<Station>>>().await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res.unwrap();

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        if let None = res.result {
            return Ok(vec![]);
        }

        Ok(res.result.unwrap())
    }

    async fn get_channel_usage(
        &self,
        ap: &AccessPoint,
    ) -> Result<Vec<ChannelUsage>, Box<dyn std::error::Error + Send>> {
        let client = self.factory.create_client().await?;

        let res = client
            .get(format!(
                "{}v4/wifi/ap/{}/channel_usage",
                self.factory.api_url,
                ap.id.unwrap()
            ))
            .send()
            .await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res
            .unwrap()
            .json::<FreeboxResponse<Vec<ChannelUsage>>>()
            .await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res.unwrap();

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        if let None = res.result {
            return Ok(vec![]);
        }

        Ok(res.result.unwrap())
    }

    async fn get_neighbors_access_points(
        &self,
        ap: &AccessPoint,
    ) -> Result<Vec<NeighborsAccessPoint>, Box<dyn std::error::Error + Send>> {
        let client = self.factory.create_client().await?;

        let res = client
            .get(format!(
                "{}v4/wifi/ap/{}/neighbors",
                self.factory.api_url,
                ap.id.unwrap()
            ))
            .send()
            .await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res.unwrap();

        let res = res
            .json::<FreeboxResponse<Vec<NeighborsAccessPoint>>>()
            .await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res.unwrap();

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        if let None = res.result {
            return Ok(vec![]);
        }

        Ok(res.result.unwrap())
    }

    async fn get_access_point(
        &self,
    ) -> Result<Vec<AccessPoint>, Box<dyn std::error::Error + Send>> {
        let client = self.factory.create_client().await?;

        let res = client
            .get(format!("{}v4/wifi/ap", self.factory.api_url))
            .send()
            .await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res
            .unwrap()
            .json::<FreeboxResponse<Vec<AccessPoint>>>()
            .await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res.unwrap();

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        if let None = res.result {
            return Ok(vec![]);
        }

        Ok(res.result.unwrap())
    }

    pub async fn set_stations_gauges(
        &self,
        stations: &[Station],
        ap: &AccessPoint,
    ) -> Result<(), Box<dyn std::error::Error + Send>> {
        for station in stations.iter() {
            let last_rx = station.last_rx.as_ref().unwrap();
            let last_tx = station.last_tx.as_ref().unwrap();
            let flags = station.flags.as_ref().unwrap();
            let host = station.host.as_ref().unwrap();

            let mut l3s = host.l3connectivities.as_ref().unwrap().to_vec();
            l3s.sort_by(|a, b| {
                b.last_time_reachable
                    .unwrap()
                    .cmp(&a.last_time_reachable.unwrap())
            });

            let l3 = l3s
                .iter()
                .filter(|l| l.af.as_ref().unwrap_or(&"unknown".to_string()) == "ipv4")
                .next();

            if let None = l3 {
                return Err(Box::new(FreeboxResponseError::new(format!(
                    "no ipv4 address found for station {}",
                    station.mac.as_ref().unwrap()
                ))));
            }

            let l3 = l3.unwrap(); // take the most recent entry

            let mac = station.mac.to_owned().unwrap_or("unknown".to_string());
            let rx_bitrate = last_rx.bitrate.unwrap_or(0);
            let rx_mcs = last_rx.mcs.unwrap_or(0);
            let rx_shortgi = last_rx.shortgi.unwrap_or_default();
            let rx_vht_mcs = last_rx.vht_mcs.unwrap_or(0);
            let rx_width = last_rx.width.to_owned().unwrap_or("unknown".to_string());
            let rx_bytes = station.rx_bytes.unwrap_or(0);
            let rx_rate = station.rx_rate.unwrap_or(0);
            let tx_bitrate = last_tx.bitrate.unwrap_or(0);
            let tx_mcs = last_tx.mcs.unwrap_or(0);
            let tx_shortgi = last_tx.shortgi.unwrap_or_default();
            let tx_vht_mcs = last_tx.vht_mcs.unwrap_or(0);
            let tx_width = last_tx.to_owned().width.unwrap_or("unknown".to_string());
            let tx_bytes = station.tx_bytes.unwrap_or(0);
            let tx_rate = station.tx_rate.unwrap_or(0);
            let signal = station.signal.unwrap_or(i8::MIN);
            let inactive = station.inactive.unwrap_or(i64::MIN);
            let state = station.state.to_owned().unwrap_or("unknown".to_string());
            let vht = flags.vht.unwrap_or_default();
            let legacy = flags.legacy.unwrap_or_default();
            let authorized = flags.authorized.unwrap_or_default();
            let ht = flags.ht.unwrap_or_default();
            let active = host.to_owned().active.unwrap_or_default();
            let last_activity = host.to_owned().last_activity.unwrap_or(i64::MIN);
            let last_time_reachable = host.to_owned().last_time_reachable.unwrap_or(i64::MIN);
            let vendor_name = host.to_owned().vendor_name.unwrap_or("unknown".to_string());
            let primary_name = host
                .to_owned()
                .primary_name
                .unwrap_or("unknown".to_string());
            let addr = l3.addr.to_owned().unwrap_or("unknown".to_string());
            let ap_name = ap.name.to_owned().unwrap_or("unknown".to_string());
            let ap_id = ap.id.to_owned().map_or(i8::MIN, |i| i as i8).to_string();
            let band = ap
                .config
                .as_ref()
                .unwrap()
                .band
                .to_owned()
                .unwrap_or("unknown".to_string());

            self.station_active_gauge
                .with_label_values(&[&primary_name, &ap_name, &band, &ap_id, &mac, &vendor_name])
                .set(active.into());

            self.station_rx_bitrate_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(rx_bitrate as i64);

            self.station_rx_mcs_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(rx_mcs as i64);

            self.station_rx_shortgi_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(rx_shortgi.into());

            self.station_rx_vht_mcs_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(rx_vht_mcs as i64);

            self.station_rx_width_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(rx_width.parse::<i64>().unwrap_or(0));

            self.station_rx_bytes_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(rx_bytes as i64);

            self.station_rx_rate_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(rx_rate as i64);

            self.station_tx_bitrate_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(tx_bitrate as i64);

            self.station_tx_mcs_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(tx_mcs as i64);

            self.station_tx_shortgi_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(tx_shortgi.into());

            self.station_tx_vht_mcs_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(tx_vht_mcs as i64);

            self.station_tx_width_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(tx_width.parse::<i64>().unwrap_or(0));

            self.station_tx_bytes_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(tx_bytes as i64);

            self.station_tx_rate_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(tx_rate as i64);

            self.station_signal_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(signal as i64);

            self.station_inactive_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(inactive);

            self.station_state_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac, &state])
                .set(1);

            self.station_flags_gauge.with_label_values(&[
                &primary_name,
                &addr,
                &ap_name,
                &band,
                &ap_id,
                &mac,
                &vht.to_string(),
                &legacy.to_string(),
                &authorized.to_string(),
                &ht.to_string(),
            ]);

            self.station_last_activity_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(last_activity);

            self.station_last_time_reachable_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &band, &ap_id, &mac])
                .set(last_time_reachable);
        }

        Ok(())
    }

    pub fn reset_all(&self) {
        self.busy_percent_gauge.reset();
        self.tx_percent_gauge.reset();
        self.rx_percent_gauge.reset();
        self.rx_bss_percent.reset();
        self.station_active_gauge.reset();
        self.station_rx_bitrate_gauge.reset();
        self.station_rx_mcs_gauge.reset();
        self.station_rx_shortgi_gauge.reset();
        self.station_rx_vht_mcs_gauge.reset();
        self.station_rx_width_gauge.reset();
        self.station_rx_bytes_gauge.reset();
        self.station_rx_rate_gauge.reset();
        self.station_tx_bitrate_gauge.reset();
        self.station_tx_mcs_gauge.reset();
        self.station_tx_shortgi_gauge.reset();
        self.station_tx_vht_mcs_gauge.reset();
        self.station_tx_width_gauge.reset();
        self.station_tx_bytes_gauge.reset();
        self.station_tx_rate_gauge.reset();
        self.station_signal_gauge.reset();
        self.station_inactive_gauge.reset();
        self.station_state_gauge.reset();
        self.station_flags_gauge.reset();
        self.station_last_activity_gauge.reset();
        self.station_last_time_reachable_gauge.reset();
        self.neighbors_access_point_gauge.reset();
        self.channel_usage_gauge.reset();
    }

    pub fn set_neighbors_access_points(
        &self,
        neighbors: &[NeighborsAccessPoint],
    ) -> Result<(), Box<dyn std::error::Error + Send>> {
        for neighbor in neighbors.iter() {
            let capabilities = neighbor.capabilities.as_ref().unwrap();
            let channel = neighbor.channel.unwrap_or(0);
            // let channel_width = neighbor.channel_width.unwrap_or(0);

            let ssid = neighbor.ssid.to_owned().unwrap_or("unknown".to_string());
            let bssid = neighbor.bssid.to_owned().unwrap_or("unknown".to_string());
            let signal = neighbor.signal.unwrap_or(i8::MIN);
            let secondary_channel = neighbor.secondary_channel.unwrap_or(0);
            let band = neighbor.band.to_owned().unwrap_or("unknown".to_string());
            let vht = capabilities.vht.unwrap_or_default();
            let legacy = capabilities.legacy.unwrap_or_default();
            let he = capabilities.he.unwrap_or_default();
            let ht = capabilities.ht.unwrap_or_default();
            let eht = capabilities.eht.unwrap_or_default();

            self.neighbors_access_point_gauge
                .with_label_values(&[
                    &channel.to_string(),
                    // &channel_width.to_string(),
                    &ssid,
                    &bssid,
                    &band,
                    &vht.to_string(),
                    &legacy.to_string(),
                    &he.to_string(),
                    &ht.to_string(),
                    &eht.to_string(),
                    &secondary_channel.to_string(),
                ])
                .set(signal as i64);
        }

        Ok(())
    }

    fn set_channel_usage_gauges(
        &self,
        channel_usage: &[ChannelUsage],
    ) -> Result<(), Box<dyn std::error::Error + Send>> {
        for usage in channel_usage.iter() {
            let band = usage.band.to_owned().unwrap_or("unknown".to_string());
            let noise_level = usage.noise_level.unwrap_or(i8::MIN);
            let channel = usage.channel.unwrap_or(0);
            let rx_busy_percent = usage.rx_busy_percent.unwrap_or(0);

            self.channel_usage_gauge
                .with_label_values(&[&band, &channel.to_string(), &rx_busy_percent.to_string()])
                .set(noise_level as i64);
        }

        Ok(())
    }

    pub async fn set_all(&self) -> Result<(), Box<dyn std::error::Error + Send>> {
        self.reset_all();
        let aps = self.get_access_point().await;
        if let Err(e) = aps {
            return Err(e);
        }
        let aps = aps?;
        for ap in aps.iter() {
            if let Err(e) = self.set_channel_survey_history_gauges(&ap).await {
                return Err(e);
            }

            let channel_usage = self.get_channel_usage(&ap).await;

            if let Ok(e) = channel_usage {
                if let Err(e) = self.set_channel_usage_gauges(&e) {
                    return Err(e);
                }
            } else if let Err(e) = channel_usage {
                return Err(e);
            }

            let neighbors = self.get_neighbors_access_points(&ap).await;

            if let Ok(a) = neighbors {
                if let Err(e) = self.set_neighbors_access_points(&a) {
                    return Err(e);
                }
            } else if let Err(e) = neighbors {
                return Err(e);
            }

            let stations = self.get_stations(&ap).await;

            if let Err(e) = stations {
                return Err(e);
            }

            if let Err(e) = self.set_stations_gauges(&stations.unwrap(), &ap).await {
                return Err(e);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl MetricMap for WifiMetricMap {
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Err(e) = self.set_all().await {
            return Err(e);
        }

        Ok(())
    }
}

fn calculate_avg_channel_survey_history(
    histories: &[ChannelSurveyHistory],
) -> ChannelSurveyHistory {
    let len = histories.len() as u32;

    let busy_avg = histories
        .iter()
        .map(|f| f.busy_percent.unwrap_or(0) as u32)
        .sum::<u32>()
        / len;
    let tx_avg = histories
        .iter()
        .map(|f| f.tx_percent.unwrap_or(0) as u32)
        .sum::<u32>()
        / len;
    let rx_bss_avg = histories
        .iter()
        .map(|f| f.rx_bss_percent.unwrap_or(0) as u32)
        .sum::<u32>()
        / len;
    let rx_avg = histories
        .iter()
        .map(|f| f.rx_percent.unwrap_or(0) as u32)
        .sum::<u32>()
        / len;

    ChannelSurveyHistory {
        timestamp: Some(chrono::offset::Local::now().timestamp() as u64),
        busy_percent: Some(busy_avg as u8),
        tx_percent: Some(tx_avg as u8),
        rx_bss_percent: Some(rx_bss_avg as u8),
        rx_percent: Some(rx_avg as u8),
    }
}

fn get_recent_channel_entries(
    histories: &[ChannelSurveyHistory],
    max_range: usize,
) -> Vec<ChannelSurveyHistory> {
    let len = histories.len() as u32;

    match len {
        0 => vec![ChannelSurveyHistory {
            timestamp: Some(chrono::offset::Local::now().timestamp() as u64),
            busy_percent: Some(0),
            tx_percent: Some(0),
            rx_bss_percent: Some(0),
            rx_percent: Some(0),
        }],
        l => {
            if l <= max_range as u32 {
                histories.to_vec()
            } else {
                // take only the last x entries
                let mut hist = histories.to_vec();
                hist.sort_by(|a, b| b.timestamp.unwrap().cmp(&a.timestamp.unwrap()));
                hist.split_at(len as usize - max_range).1.to_vec()
            }
        }
    }
}

#[cfg(test)]
mod tests_deserialize {
    use serde_json::from_str;

    use crate::{core::common::FreeboxResponse, mappers::api_specs_provider::get_specs_data};

    use super::*;

    #[tokio::test]
    async fn deserialize_api_v2_wifi_config() {
        let json_data = get_specs_data("wifi", "api_v2_wifi_config-get")
            .await
            .unwrap();

        let data: Result<FreeboxResponse<WifiConfig>, _> = from_str(&json_data);

        if let Ok(e) = &data {
            println!("{:?}", e);
        }

        assert!(data.is_ok());
    }

    #[tokio::test]
    async fn deserialize_api_v2_wifi_ap_0_stations() {
        let json_data = get_specs_data("wifi", "api_v2_wifi_ap_0_stations-get")
            .await
            .unwrap();

        let data: Result<FreeboxResponse<Vec<Station>>, _> = from_str(&json_data);

        if let Ok(e) = &data {
            println!("{:?}", e);
        }

        assert!(data.is_ok());
    }

    #[tokio::test]
    async fn deserialize_api_latest_wifi_ap_0_channel_survey_history() {
        let json_data = get_specs_data("wifi", "api_latest_wifi_ap_0_channel_survey_history-get")
            .await
            .unwrap();

        let data: Result<FreeboxResponse<Vec<ChannelSurveyHistory>>, _> = from_str(&json_data);

        assert!(data.is_ok());

        let avg_history =
            calculate_avg_channel_survey_history(data.unwrap().result.as_ref().unwrap());

        assert_eq!(avg_history.busy_percent.unwrap(), 34);
        assert_eq!(avg_history.tx_percent.unwrap(), 1);
        assert_eq!(avg_history.rx_percent.unwrap(), 30);
        assert_eq!(avg_history.rx_bss_percent.unwrap(), 0);
    }

    #[tokio::test]
    async fn deserialize_api_latest_ap_neighbors() {
        let json_data = get_specs_data("wifi", "api_latest_wifi_ap_0_neighbors-get")
            .await
            .unwrap();

        let data: Result<FreeboxResponse<Vec<NeighborsAccessPoint>>, _> = from_str(&json_data);

        if let Ok(e) = &data {
            println!("{:?}", e);
        }

        assert!(data.is_ok());
    }

    #[tokio::test]
    async fn deserialize_api_latest_ap_channel_usage() {
        let json_data = get_specs_data("wifi", "api_latest_wifi_ap_0_channel_usage-get")
            .await
            .unwrap();

        let data: Result<FreeboxResponse<Vec<ChannelUsage>>, _> = from_str(&json_data);

        if let Ok(e) = &data {
            println!("{:?}", e);
        }

        assert!(data.is_ok());
    }
}
