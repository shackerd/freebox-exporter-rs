use std::error::Error;
use std::usize;

use async_trait::async_trait;
use chrono::Duration;
use log::{debug, info};
use models::{AccessPoint, ChannelSurveyHistory, ChannelUsage, NeighborsAccessPoint, Station};
use prometheus_exporter::prometheus::{register_int_gauge_vec, IntGaugeVec};
use reqwest::Client;
use utils::{calculate_avg_channel_survey_history, get_recent_channel_entries};

use crate::{
    core::common::{
        http_client_factory::{AuthenticatedHttpClientFactory, ManagedHttpClient},
        transport::{FreeboxResponse, FreeboxResponseError},
    },
    diagnostics::DryRunnable,
    mappers::wifi::models::WifiConfig,
};

use super::MetricMap;
use crate::diagnostics::DryRunOutputWriter;

pub mod models;
pub mod unittests;
pub mod utils;

pub struct WifiMetricMap<'a> {
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    managed_client: Option<ManagedHttpClient>,
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

impl<'a> WifiMetricMap<'a> {
    pub fn new(
        factory: &'a AuthenticatedHttpClientFactory<'a>,
        prefix: String,
        history_ttl: Duration,
    ) -> Self {
        let prfx: String = format!("{prefix}_wifi");
        Self {
            factory,
            managed_client: None,
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
                    "ssid",
                    "bssid",
                    "band",
                    "vht",
                    "legacy",
                    "he",
                    "ht",
                    "eht",
                    "secondary_channel"
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

    async fn get_managed_client(
        &mut self,
    ) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
        if self.managed_client.is_none() {
            debug!("creating managed client");
            self.managed_client = Some(self.factory.create_managed_client().await?);
        }

        match self.managed_client.as_ref().unwrap().get() {
            Ok(client) => Ok(client),
            Err(_) => {
                debug!("renewing managed client");
                self.managed_client = Some(self.factory.create_managed_client().await?);
                self.managed_client.as_ref().unwrap().get()
            }
        }
    }

    async fn set_channel_survey_history_gauges(
        &mut self,
        ap: &AccessPoint,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!(
            "fetching channel survey history for access point {}",
            ap.id.as_ref().unwrap()
        );

        let client = self.get_managed_client().await?;
        let ts = chrono::offset::Local::now().timestamp();
        let root_url = &self.factory.api_url;
        let ap_id = ap.id.as_ref().unwrap().to_string();
        let band = ap.config.as_ref().unwrap().band.as_ref().unwrap();
        let ap_name = ap.name.as_deref().unwrap_or("unknown");
        let url = format!("{root_url}v4/wifi/ap/{ap_id}/channel_survey_history/{ts}");

        let res = client
            .get(url.to_owned())
            .send()
            .await?
            .json::<FreeboxResponse<Vec<ChannelSurveyHistory>>>()
            .await?;

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        if res.result.is_none() {
            info!("channel survey history is empty, wifi might be disabled on freebox");
            return Ok(());
        }

        let result = res.result.unwrap();

        let recent = get_recent_channel_entries(
            &result,
            (self.history_ttl.num_milliseconds() / 100) as usize,
        );
        let avg_history = calculate_avg_channel_survey_history(&recent);

        self.busy_percent_gauge
            .with_label_values(&[&ap_id, &ap_name, &band])
            .set(avg_history.busy_percent.unwrap_or(0) as i64);

        self.tx_percent_gauge
            .with_label_values(&[&ap_id, &ap_name, &band])
            .set(avg_history.tx_percent.unwrap_or(0) as i64);

        self.rx_bss_percent
            .with_label_values(&[&ap_id, &ap_name, &band])
            .set(avg_history.rx_bss_percent.unwrap_or(0) as i64);

        self.rx_percent_gauge
            .with_label_values(&[&ap_id, &ap_name, &band])
            .set(avg_history.rx_percent.unwrap_or(0) as i64);

        Ok(())
    }

    async fn get_stations(
        &mut self,
        ap: &AccessPoint,
    ) -> Result<Vec<Station>, Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching wifi stations");

        let client = self.get_managed_client().await?;
        let res = client
            .get(format!(
                "{}v4/wifi/ap/{}/stations",
                self.factory.api_url,
                ap.id.unwrap()
            ))
            .send()
            .await?
            .json::<FreeboxResponse<Vec<Station>>>()
            .await?;

        if res.success.unwrap_or(false) {
            Ok(res.result.unwrap_or_default())
        } else {
            Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )))
        }
    }

    async fn get_channel_usage(
        &mut self,
        ap: &AccessPoint,
    ) -> Result<Vec<ChannelUsage>, Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching channel usage for access point {}", ap.id.unwrap());
        let client = self.get_managed_client().await?;
        let res = client
            .get(format!(
                "{}v4/wifi/ap/{}/channel_usage",
                self.factory.api_url,
                ap.id.unwrap()
            ))
            .send()
            .await?
            .json::<FreeboxResponse<Vec<ChannelUsage>>>()
            .await?;

        if res.success.unwrap_or(false) {
            Ok(res.result.unwrap_or_default())
        } else {
            Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )))
        }
    }

    async fn get_neighbors_access_points(
        &mut self,
        ap: &AccessPoint,
    ) -> Result<Vec<NeighborsAccessPoint>, Box<dyn std::error::Error + Send + Sync>> {
        debug!(
            "fetching neighbors access points for access point {}",
            ap.id.unwrap()
        );
        let client = self.get_managed_client().await?;

        let res = client
            .get(format!(
                "{}v4/wifi/ap/{}/neighbors",
                self.factory.api_url,
                ap.id.unwrap()
            ))
            .send()
            .await?
            .json::<FreeboxResponse<Vec<NeighborsAccessPoint>>>()
            .await?;

        if res.success.unwrap_or(false) {
            Ok(res.result.unwrap_or_default())
        } else {
            Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )))
        }
    }

    async fn get_access_points(
        &mut self,
    ) -> Result<Vec<AccessPoint>, Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching access points");
        let client = self.get_managed_client().await?;

        let res = client
            .get(format!("{}v4/wifi/ap", self.factory.api_url))
            .send()
            .await?
            .json::<FreeboxResponse<Vec<AccessPoint>>>()
            .await?;

        if res.success.unwrap_or(false) {
            Ok(res.result.unwrap_or_default())
        } else {
            Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )))
        }
    }

    pub async fn set_stations_gauges(
        &self,
        stations: &[Station],
        ap: &AccessPoint,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for neighbor in neighbors.iter() {
            let capabilities = neighbor.capabilities.as_ref().unwrap();
            let channel = neighbor.channel.unwrap_or(0);
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
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

    async fn get_wifi_config(
        &mut self,
    ) -> Result<WifiConfig, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_managed_client().await?;
        let response = client
            .get(format!("{}v4/wifi/config", self.factory.api_url))
            .send()
            .await?
            .json::<FreeboxResponse<WifiConfig>>()
            .await?;

        if response.success.unwrap_or(false) {
            Ok(response.result.unwrap())
        } else {
            Err(Box::new(FreeboxResponseError::new(
                response.msg.unwrap_or_default(),
            )))
        }
    }

    async fn get_access_point(
        &mut self,
        phy_id: &i16,
    ) -> Result<AccessPoint, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_managed_client().await?;
        let response = client
            .get(format!("{}v4/wifi/ap/{}", self.factory.api_url, phy_id))
            .send()
            .await?
            .json::<FreeboxResponse<AccessPoint>>()
            .await?;

        if response.success.unwrap_or(false) {
            Ok(response.result.unwrap())
        } else {
            Err(Box::new(FreeboxResponseError::new(
                response.msg.unwrap_or_default(),
            )))
        }
    }

    pub async fn set_all(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.reset_all();
        let aps = self.get_access_points().await?;

        let aps = match aps.len() {
            0 => {
                info!("no access points found in /wifi/ap endpoint, fallbacking to /wifi/config");
                let config = self.get_wifi_config().await?;
                if config.expected_phys.is_none() {
                    info!("no expected_phys found in /wifi/config endpoint, wifi might be disabled on freebox or no device connected");
                    return Ok(());
                }

                let phys = config.expected_phys.unwrap();
                let mut aps: Vec<AccessPoint> = Vec::with_capacity(phys.len());
                for phy in phys.iter() {
                    let id = phy.phy_id.unwrap();
                    if let Ok(ap) = self.get_access_point(&id).await {
                        debug!(
                            "found access point {} from phy_id {}",
                            ap.name.as_deref().unwrap_or("unknown"),
                            id
                        );
                        aps.push(ap);
                    }
                }
                debug!("{} access points found", aps.len());
                aps
            }
            _ => {
                debug!("{} access points found", aps.len());
                aps
            }
        };

        for ap in aps.iter() {
            self.set_channel_survey_history_gauges(&ap).await?;

            if let Ok(channel_usage) = self.get_channel_usage(&ap).await {
                self.set_channel_usage_gauges(&channel_usage)?;
            }

            if let Ok(neighbors) = self.get_neighbors_access_points(&ap).await {
                self.set_neighbors_access_points(&neighbors)?;
            }

            let stations = self.get_stations(&ap).await?;
            self.set_stations_gauges(&stations, &ap).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl<'a> MetricMap<'a> for WifiMetricMap<'a> {
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.set_all().await
    }
}

#[async_trait]
impl DryRunnable for WifiMetricMap<'_> {
    fn get_name(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        Ok("wifi".to_string())
    }

    async fn dry_run(
        &mut self,
        _writer: &mut dyn DryRunOutputWriter,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }

    fn as_dry_runnable(&mut self) -> &mut dyn DryRunnable {
        self
    }
}
