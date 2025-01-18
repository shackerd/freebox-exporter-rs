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
    pub names: Option<Vec<HostName>>,
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
pub struct ChannelSurveyHistory {
    pub timestamp: Option<u64>,
    pub busy_percent: Option<u8>,
    pub tx_percent: Option<u8>,
    pub rx_bss_percent: Option<u8>,
    pub rx_percent: Option<u8>,
}

pub struct WifiMetricMap {
    factory: AuthenticatedHttpClientFactory,
    busy_percent: IntGaugeVec,
    tx_percent: IntGaugeVec,
    rx_percent: IntGaugeVec,
    rx_bss_percent: IntGaugeVec,
    history_ttl: Duration,
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
            history_ttl,
        }
    }

    async fn set_channel_survey_history(
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

    pub async fn set_stations(
        &self,
        stations: &[Station],
    ) -> Result<(), Box<dyn std::error::Error>> {
        for station in stations.iter() {
            let mac = station.mac.as_ref().unwrap();
            let last_rx = station.last_rx.as_ref().unwrap();
            let last_tx = station.last_tx.as_ref().unwrap();
            let tx_bytes = station.tx_bytes.unwrap();
            let tx_rate = station.tx_rate.unwrap();
            let rx_bytes = station.rx_bytes.unwrap();
            let rx_rate = station.rx_rate.unwrap();
            let id = station.id.as_ref().unwrap();
            let bssid = station.bssid.as_ref().unwrap();
            let flags = station.flags.as_ref().unwrap();
            let host = station.host.as_ref().unwrap();
            let l2ident = host.l2ident.as_ref().unwrap();
            let names = host.to_owned().names.unwrap();

            println!(
                "mac: {}, last_rx: {:?}, last_tx: {:?}, tx_bytes: {}, tx_rate: {}, rx_bytes: {}, rx_rate: {}, id: {}, bssid: {}, flags: {:?}, host: {:?}, l2ident: {:?}, names: {:?}",
                mac, last_rx, last_tx, tx_bytes, tx_rate, rx_bytes, rx_rate, id, bssid, flags, host, l2ident, names
            );
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
            self.set_channel_survey_history(&ap).await?;

            let stations = self.get_stations(&ap).await;

            if let Err(e) = stations {
                return Err(e);
            }

            self.set_stations(&stations.unwrap()).await?;
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
