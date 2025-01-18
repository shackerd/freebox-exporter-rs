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
pub struct AccessPoint {
    pub name: Option<String>,
    pub id: Option<u8>,
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

pub struct WifiMetricMap {
    factory: AuthenticatedHttpClientFactory,
    history_ttl: Duration,
    busy_percent: IntGaugeVec,
    tx_percent: IntGaugeVec,
    rx_percent: IntGaugeVec,
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
            busy_percent: register_int_gauge_vec!(
                format!("{prfx}_busy_percent"),
                format!("{prfx}_busy_percent"),
                &["ap", "name"]
            )
            .expect(&format!("cannot create {prfx}_busy_percent gauge")),
            rx_bss_percent: register_int_gauge_vec!(
                format!("{prfx}_rx_bss_percent"),
                format!("{prfx}_rx_bss_percent"),
                &["ap", "name"]
            )
            .expect(&format!("cannot create {prfx}_rx_bss_percent gauge")),
            rx_percent: register_int_gauge_vec!(
                format!("{prfx}_rx_percent"),
                format!("{prfx}_rx_percent"),
                &["ap", "name"]
            )
            .expect(&format!("cannot create {prfx}_rx_percent gauge")),
            tx_percent: register_int_gauge_vec!(
                format!("{prfx}_tx_percent"),
                format!("{prfx}_tx_percent"),
                &["ap", "name"]
            )
            .expect(&format!("cannot create {prfx}_tx_percent gauge")),
            station_active_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_active"),
                format!("{prfx}_station_active 1 for active"),
                &["primary_name", "ap_name", "ap_id", "mac", "vendor_name"]
            )
            .expect(&format!("cannot create {prfx}_station_mac gauge")),
            station_rx_bitrate_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_rx_bitrate"),
                format!("{prfx}_station_rx_bitrate"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_rx_bitrate gauge")),

            station_rx_mcs_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_rx_mcs"),
                format!("{prfx}_station_rx_mcs"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_rx_mcs gauge")),

            station_rx_shortgi_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_rx_shortgi"),
                format!("{prfx}_station_rx_shortgi 1 for shortgi enabled"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_rx_shortgi gauge")),

            station_rx_vht_mcs_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_rx_vht_mcs"),
                format!("{prfx}_station_rx_vht_mcs"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_rx_vht_mcs gauge")),

            station_rx_width_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_rx_width"),
                format!("{prfx}_station_rx_width"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_rx_width gauge")),

            station_rx_bytes_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_rx_bytes"),
                format!("{prfx}_station_rx_bytes"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_rx_bytes gauge")),

            station_rx_rate_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_rx_rate"),
                format!("{prfx}_station_rx_rate"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_rx_rate gauge")),

            station_tx_bitrate_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_tx_bitrate"),
                format!("{prfx}_station_tx_bitrate"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_tx_bitrate gauge")),

            station_tx_mcs_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_tx_mcs"),
                format!("{prfx}_station_tx_mcs"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_tx_mcs gauge")),

            station_tx_shortgi_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_tx_shortgi"),
                format!("{prfx}_station_tx_shortgi 1 for shortgi enabled"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_tx_shortgi gauge")),

            station_tx_vht_mcs_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_tx_vht_mcs"),
                format!("{prfx}_station_tx_vht_mcs"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_tx_vht_mcs gauge")),

            station_tx_width_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_tx_width"),
                format!("{prfx}_station_tx_width"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_tx_width gauge")),

            station_tx_bytes_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_tx_bytes"),
                format!("{prfx}_station_tx_bytes"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_tx_bytes gauge")),

            station_tx_rate_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_tx_rate"),
                format!("{prfx}_station_tx_rate"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_tx_rate gauge")),

            station_signal_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_signal"),
                format!("{prfx}_station_signal"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_signal gauge")),

            station_inactive_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_inactive"),
                format!("{prfx}_station_inactive"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_inactive gauge")),

            station_state_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_state"),
                format!("{prfx}_station_state"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac", "state"]
            )
            .expect(&format!("cannot create {prfx}_station_state gauge")),

            station_flags_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_flags"),
                format!("{prfx}_station_flags"),
                &[
                    "primary_name",
                    "ipv4",
                    "ap_name",
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
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!("cannot create {prfx}_station_last_activity gauge")),
            station_last_time_reachable_gauge: register_int_gauge_vec!(
                format!("{prfx}_station_last_time_reachable"),
                format!("{prfx}_station_last_time_reachable"),
                &["primary_name", "ipv4", "ap_name", "ap_id", "mac"]
            )
            .expect(&format!(
                "cannot create {prfx}_station_last_time_reachable gauge"
            )),
        }
    }

    async fn set_channel_survey_history_gauges(
        &self,
        ap: &AccessPoint,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let client = self.factory.create_client().await?;
        let ts = chrono::offset::Local::now().timestamp();
        let root_url = &self.factory.api_url;
        let ap_id = ap.id.as_ref().unwrap().to_string();
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

        let res = res?
            .json::<FreeboxResponse<Vec<ChannelSurveyHistory>>>()
            .await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res?;

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

        self.busy_percent
            .with_label_values(&[&ap_id, &ap_name])
            .set(avg_history.busy_percent.unwrap() as i64);

        self.tx_percent
            .with_label_values(&[&ap_id, &ap_name])
            .set(avg_history.tx_percent.unwrap() as i64);

        self.rx_bss_percent
            .with_label_values(&[&ap_id, &ap_name])
            .set(avg_history.rx_bss_percent.unwrap() as i64);

        self.rx_percent
            .with_label_values(&[&ap_id, &ap_name])
            .set(avg_history.rx_percent.unwrap() as i64);

        Ok(())
    }

    async fn get_stations(
        &self,
        ap: &AccessPoint,
    ) -> Result<Vec<Station>, Box<dyn std::error::Error>> {
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

        let res = res?.json::<FreeboxResponse<Vec<Station>>>().await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res?;

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

    async fn get_access_point(&self) -> Result<Vec<AccessPoint>, Box<dyn std::error::Error>> {
        let client = self.factory.create_client().await?;

        let res = client
            .get(format!("{}v4/wifi/ap", self.factory.api_url))
            .send()
            .await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res?.json::<FreeboxResponse<Vec<AccessPoint>>>().await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res?;

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
    ) -> Result<(), Box<dyn std::error::Error>> {
        for station in stations.iter() {
            let last_rx = station.last_rx.as_ref().unwrap();
            let last_tx = station.last_tx.as_ref().unwrap();
            let flags = station.flags.as_ref().unwrap();
            let host = station.host.as_ref().unwrap();
            // let l2ident = host.l2ident.as_ref().unwrap();
            // let hostnames = host.to_owned().names.unwrap();
            let mut l3s = host.l3connectivities.as_ref().unwrap().to_vec();
            l3s.sort_by(|a, b| {
                b.last_time_reachable
                    .unwrap()
                    .cmp(&a.last_time_reachable.unwrap())
            });
            let l3 = l3s.first().unwrap(); // take the most recent entry

            let mac = station.mac.to_owned().unwrap();
            let rx_bitrate = last_rx.bitrate.unwrap();
            let rx_mcs = last_rx.mcs.unwrap();
            let rx_shortgi = last_rx.shortgi.unwrap();
            let rx_vht_mcs = last_rx.vht_mcs.unwrap();
            let rx_width = last_rx.width.to_owned().unwrap();
            let rx_bytes = station.rx_bytes.unwrap();
            let rx_rate = station.rx_rate.unwrap();
            let tx_bitrate = last_tx.bitrate.unwrap();
            let tx_mcs = last_tx.mcs.unwrap();
            let tx_shortgi = last_tx.shortgi.unwrap();
            let tx_vht_mcs = last_tx.vht_mcs.unwrap();
            let tx_width = last_tx.to_owned().width.unwrap();
            let tx_bytes = station.tx_bytes.unwrap();
            let tx_rate = station.tx_rate.unwrap();
            // let id = station.id.to_owned().unwrap();
            // let bssid = station.bssid.to_owned().unwrap();
            let signal = station.signal.unwrap();
            let inactive = station.inactive.unwrap();
            let state = station.state.to_owned().unwrap();
            let vht = flags.vht.unwrap();
            let legacy = flags.legacy.unwrap();
            let authorized = flags.authorized.unwrap();
            let ht = flags.ht.unwrap();
            let active = host.to_owned().active.unwrap();
            let last_activity = host.to_owned().last_activity.unwrap();
            let last_time_reachable = host.to_owned().last_time_reachable.unwrap();
            let vendor_name = host.to_owned().vendor_name.unwrap();
            let primary_name = host.to_owned().primary_name.unwrap();
            // let primary_name_manual = host.to_owned().primary_name_manual.unwrap();
            // let id = l2ident.id.to_owned().unwrap();
            // let r#type = l2ident.r#type.to_owned().unwrap();
            let addr = l3.addr.to_owned().unwrap();
            // let af = l3.af.to_owned().unwrap();
            // let active = l3.active.unwrap();
            // let reachable = l3.reachable.unwrap();
            // let last_activity = l3.last_activity.unwrap();
            // let last_time_reachable = l3.last_time_reachable.unwrap();
            let ap_name = ap.name.to_owned().unwrap();
            let ap_id = ap.id.to_owned().unwrap().to_string();

            self.station_active_gauge
                .with_label_values(&[&primary_name, &ap_name, &ap_id, &mac, &vendor_name])
                .set(active.into());

            self.station_rx_bitrate_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(rx_bitrate as i64);

            self.station_rx_mcs_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(rx_mcs as i64);

            self.station_rx_shortgi_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(rx_shortgi.into());

            self.station_rx_vht_mcs_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(rx_vht_mcs as i64);

            self.station_rx_width_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(rx_width.parse::<i64>().unwrap_or(0));

            self.station_rx_bytes_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(rx_bytes as i64);

            self.station_rx_rate_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(rx_rate as i64);

            self.station_tx_bitrate_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(tx_bitrate as i64);

            self.station_tx_mcs_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(tx_mcs as i64);

            self.station_tx_shortgi_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(tx_shortgi.into());

            self.station_tx_vht_mcs_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(tx_vht_mcs as i64);

            self.station_tx_width_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(tx_width.parse::<i64>().unwrap_or(0));

            self.station_tx_bytes_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(tx_bytes as i64);

            self.station_tx_rate_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(tx_rate as i64);

            self.station_signal_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(signal as i64);

            self.station_inactive_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(inactive);

            self.station_state_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac, &state])
                .set(1);

            self.station_flags_gauge.with_label_values(&[
                &primary_name,
                &addr,
                &ap_name,
                &ap_id,
                &mac,
                &vht.to_string(),
                &legacy.to_string(),
                &authorized.to_string(),
                &ht.to_string(),
            ]);

            self.station_last_activity_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(last_activity);

            self.station_last_time_reachable_gauge
                .with_label_values(&[&primary_name, &addr, &ap_name, &ap_id, &mac])
                .set(last_time_reachable);

            // println!("{:?}", station);
        }

        Ok(())
    }

    pub async fn set_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        let aps = self.get_access_point().await;
        if let Err(e) = aps {
            return Err(e);
        }
        let aps = aps?;
        for ap in aps.iter() {
            self.set_channel_survey_history_gauges(&ap).await?;

            let stations = self.get_stations(&ap).await;

            if let Err(e) = stations {
                return Err(e);
            }

            self.set_stations_gauges(&stations.unwrap(), &ap).await?;
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
}
