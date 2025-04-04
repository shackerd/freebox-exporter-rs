use async_trait::async_trait;
use lazy_static::lazy_static;
use log::debug;
use prometheus_exporter::prometheus::{register_int_gauge_vec, IntGaugeVec};
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;

use crate::{core::common::{
    http_client_factory::{AuthenticatedHttpClientFactory, ManagedHttpClient},
    transport::{FreeboxResponse, FreeboxResponseError},
}, diagnostics::DryRunnable};

use super::MetricMap;

#[derive(Deserialize, Clone, Debug)]
pub struct SwitchPortStatus {
    id: Option<i16>,
    link: Option<String>,
    speed: Option<String>,
    mac_list: Option<Vec<SwitchPortHost>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SwitchPortStats {
    rx_packets_rate: Option<i64>,
    rx_good_bytes: Option<i64>,
    rx_oversize_packets: Option<i64>,
    rx_unicast_packets: Option<i64>,
    tx_bytes_rate: Option<i64>,
    tx_unicast_packets: Option<i64>,
    rx_bytes_rate: Option<i64>,
    tx_packets: Option<i64>,
    tx_collisions: Option<i64>,
    tx_packets_rate: Option<i64>,
    tx_fcs: Option<i64>,
    tx_bytes: Option<i64>,
    rx_jabber_packets: Option<i64>,
    tx_single: Option<i64>,
    tx_excessive: Option<i64>,
    rx_pause: Option<i64>,
    rx_multicast_packets: Option<i64>,
    tx_pause: Option<i64>,
    rx_good_packets: Option<i64>,
    rx_broadcast_packets: Option<i64>,
    tx_multiple: Option<i64>,
    tx_deferred: Option<i64>,
    tx_late: Option<i64>,
    tx_multicast_packets: Option<i64>,
    rx_fcs_packets: Option<i64>,
    tx_broadcast_packets: Option<i64>,
    rx_err_packets: Option<i64>,
    rx_fragments_packets: Option<i64>,
    rx_bad_bytes: Option<i64>,
    rx_undersize_packets: Option<i64>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SwitchPortHost {
    mac: Option<String>,
    hostname: Option<String>,
}

pub struct SwitchMetricMap<'a> {
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    managed_client: Option<ManagedHttpClient>,
    rx_packets_rate_gauge: IntGaugeVec,
    rx_good_bytes_gauge: IntGaugeVec,
    rx_oversize_packets_gauge: IntGaugeVec,
    rx_unicast_packets_gauge: IntGaugeVec,
    tx_bytes_rate_gauge: IntGaugeVec,
    tx_unicast_packets_gauge: IntGaugeVec,
    rx_bytes_rate_gauge: IntGaugeVec,
    tx_packets_gauge: IntGaugeVec,
    tx_collisions_gauge: IntGaugeVec,
    tx_packets_rate_gauge: IntGaugeVec,
    tx_fcs_gauge: IntGaugeVec,
    tx_bytes_gauge: IntGaugeVec,
    rx_jabber_packets_gauge: IntGaugeVec,
    tx_single_gauge: IntGaugeVec,
    tx_excessive_gauge: IntGaugeVec,
    rx_pause_gauge: IntGaugeVec,
    rx_multicast_packets_gauge: IntGaugeVec,
    tx_pause_gauge: IntGaugeVec,
    rx_good_packets_gauge: IntGaugeVec,
    rx_broadcast_packets_gauge: IntGaugeVec,
    tx_multiple_gauge: IntGaugeVec,
    tx_deferred_gauge: IntGaugeVec,
    tx_late_gauge: IntGaugeVec,
    tx_multicast_packets_gauge: IntGaugeVec,
    rx_fcs_packets_gauge: IntGaugeVec,
    tx_broadcast_packets_gauge: IntGaugeVec,
    rx_err_packets_gauge: IntGaugeVec,
    rx_fragments_packets_gauge: IntGaugeVec,
    rx_bad_bytes_gauge: IntGaugeVec,
    rx_undersize_packets_gauge: IntGaugeVec,
    port_status_gauge: IntGaugeVec,
    port_speed_gauge: IntGaugeVec,
    port_mac_list_gauge: IntGaugeVec,
}

impl<'a> SwitchMetricMap<'a> {
    pub fn new(factory: &'a AuthenticatedHttpClientFactory<'a>, prefix: String) -> Self {
        let prfx: String = format!("{prefix}_switch");
        let stats_prfx: String = format!("{prfx}_stats");

        Self {
            factory,
            managed_client: None,
            rx_packets_rate_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_rx_packets_rate"),
                "rx packet rate",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_rx_packet_rate gauge")),
            rx_good_bytes_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_rx_good_bytes"),
                "rx good bytes",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_rx_good_bytes gauge")),
            rx_oversize_packets_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_rx_oversize_packets"),
                "rx oversize packets",
                &["port"]
            )
            .expect(&format!(
                "cannot create {stats_prfx}_rx_oversize_packets gauge"
            )),
            rx_unicast_packets_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_rx_unicast_packets"),
                "rx unicast packets",
                &["port"]
            )
            .expect(&format!(
                "cannot create {stats_prfx}_rx_unicast_packets gauge"
            )),
            tx_bytes_rate_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_tx_bytes_rate"),
                "tx bytes rate",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_tx_bytes_rate gauge")),
            tx_unicast_packets_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_tx_unicast_packets"),
                "tx unicast packets",
                &["port"]
            )
            .expect(&format!(
                "cannot create {stats_prfx}_tx_unicast_packets gauge"
            )),
            rx_bytes_rate_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_rx_bytes_rate"),
                "rx bytes rate",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_rx_bytes_rate gauge")),
            tx_packets_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_tx_packets"),
                "tx packets",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_tx_packets gauge")),
            tx_collisions_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_tx_collisions"),
                "tx collisions",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_tx_collisions gauge")),
            tx_packets_rate_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_tx_packets_rate"),
                "tx packets rate",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_tx_packets_rate gauge")),
            tx_fcs_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_tx_fcs"),
                "tx fcs",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_tx_fcs gauge")),
            tx_bytes_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_tx_bytes"),
                "tx bytes",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_tx_bytes gauge")),
            rx_jabber_packets_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_rx_jabber_packets"),
                "rx jabber packets",
                &["port"]
            )
            .expect(&format!(
                "cannot create {stats_prfx}_rx_jabber_packets gauge"
            )),
            tx_single_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_tx_single"),
                "tx single",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_tx_single gauge")),
            tx_excessive_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_tx_excessive"),
                "tx excessive",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_tx_excessive gauge")),
            rx_pause_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_rx_pause"),
                "rx pause",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_rx_pause gauge")),
            rx_multicast_packets_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_rx_multicast_packets"),
                "rx multicast packets",
                &["port"]
            )
            .expect(&format!(
                "cannot create {stats_prfx}_rx_multicast_packets gauge"
            )),
            tx_pause_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_tx_pause"),
                "tx pause",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_tx_pause gauge")),
            rx_good_packets_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_rx_good_packets"),
                "tx good packets",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_rx_good_packets gauge")),
            rx_broadcast_packets_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_rx_broadcast_packets"),
                "rx broadcast packets",
                &["port"]
            )
            .expect(&format!(
                "cannot create {stats_prfx}_rx_broadcast_packets gauge"
            )),
            tx_multiple_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_tx_multiple"),
                "tx multiple",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_tx_multiple gauge")),
            tx_deferred_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_tx_deferred"),
                "tx deferred",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_tx_deferred gauge")),
            tx_late_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_tx_late"),
                "tx late",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_tx_late gauge")),
            tx_multicast_packets_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_tx_multicast_packets"),
                "tx multicast packets",
                &["port"]
            )
            .expect(&format!(
                "cannot create {stats_prfx}_tx_multicast_packets gauge"
            )),
            rx_fcs_packets_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_rx_fcs_packets"),
                "rx fcs packets",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_rx_fcs_packets gauge")),
            tx_broadcast_packets_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_tx_broadcast_packets"),
                "tx broadcast packets",
                &["port"]
            )
            .expect(&format!(
                "cannot create {stats_prfx}_tx_broadcast_packets gauge"
            )),
            rx_err_packets_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_rx_err_packets"),
                "rx err packets",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_rx_err_packets gauge")),
            rx_fragments_packets_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_rx_fragments_packets"),
                "rx fragments packets",
                &["port"]
            )
            .expect(&format!(
                "cannot create {stats_prfx}_rx_fragments_packets gauge"
            )),
            rx_bad_bytes_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_rx_bad_bytes"),
                "rx bad bytes",
                &["port"]
            )
            .expect(&format!("cannot create {stats_prfx}_rx_bad_bytes gauge")),
            rx_undersize_packets_gauge: register_int_gauge_vec!(
                format!("{stats_prfx}_rx_undersize_packets"),
                "rx undersize packets",
                &["port"]
            )
            .expect(&format!(
                "cannot create {stats_prfx}_rx_undersize_packets gauge"
            )),
            port_status_gauge: register_int_gauge_vec!(
                format!("{prfx}_port_status"),
                "port status, 1 for link up",
                &["port"]
            )
            .expect(&format!("cannot create {prfx}_port_status gauge")),
            port_speed_gauge: register_int_gauge_vec!(
                format!("{prfx}_port_speed"),
                "port status speed",
                &["port"]
            )
            .expect(&format!("cannot create {prfx}_port_speed gauge")),
            port_mac_list_gauge: register_int_gauge_vec!(
                format!("{prfx}_port_mac_list"),
                "port mac list, always 1",
                &["port", "mac", "hostname"]
            )
            .expect(&format!("cannot create {prfx}_port_mac_list gauge")),
        }
    }

    async fn get_managed_client(
        &mut self,
    ) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
        if self.managed_client.as_ref().is_none() {
            debug!("creating managed client");

            let res = self.factory.create_managed_client().await;

            if res.is_err() {
                debug!("cannot create managed client");

                return Err(res.err().unwrap());
            }

            self.managed_client = Some(res.unwrap());
        }

        let client = self.managed_client.as_ref().clone().unwrap();
        let res = client.get();

        if res.is_ok() {
            return Ok(res.unwrap());
        } else {
            debug!("renewing managed client");

            let client = self.factory.create_managed_client().await;
            self.managed_client = Some(client.unwrap());

            return self.managed_client.as_ref().unwrap().get();
        }
    }

    async fn get_ports_status_json(
        &mut self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching switch ports statuses");

        let body = self
            .get_managed_client()
            .await
            .unwrap()
            .get(format!("{}v4/switch/status/", self.factory.api_url))
            .send()
            .await?
            .text()
            .await?;

        Ok(body)
    }

    async fn get_ports_status(
        &mut self,
        body: &str
    ) -> Result<Vec<SwitchPortStatus>, Box<dyn std::error::Error + Send + Sync>> {      

        let fixed_body = SwitchMetricMap::handle_malformed_mac_list(&body)?;

        let res = match serde_json::from_str::<FreeboxResponse<Vec<SwitchPortStatus>>>(&fixed_body)
        {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        let statuses = match res.result {
            None => {
                return Err(Box::new(FreeboxResponseError::new(
                    "v4/switch/status/ response was empty".to_string(),
                )))
            }
            Some(r) => r,
        };

        Ok(statuses)
    }

    fn handle_malformed_mac_list(
        res: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let fixed_results = REG_MAC.replace_all(res, r#""mac_list":[]"#).to_string();
        Ok(fixed_results)
    }

    async fn get_port_stats_json(
        &mut self,
        port_status: &SwitchPortStatus,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching switch ports stats");

        let port_id = port_status.id.unwrap_or_default();

        let body = self
            .get_managed_client()
            .await
            .unwrap()
            .get(format!(
                "{}v4/switch/port/{}/stats",
                self.factory.api_url, port_id
            ))
            .send()
            .await?
            .text()
            .await?;

        Ok(body)
    }

    async fn get_port_stats(
        &mut self,
        body: &str, 
        port_id: &i16,
    ) -> Result<SwitchPortStats, Box<dyn std::error::Error + Send + Sync>> {

        let res = match serde_json::from_str::<FreeboxResponse<SwitchPortStats>>(body) {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        match res.result {
            None => {
                return Err(Box::new(FreeboxResponseError::new(format!(
                    "v4/switch/port/{}/stats response was empty",
                    port_id
                ))))
            }
            Some(r) => Ok(r),
        }
    }

    fn reset_all(&self) {
        self.rx_packets_rate_gauge.reset();
        self.rx_good_bytes_gauge.reset();
        self.rx_oversize_packets_gauge.reset();
        self.rx_unicast_packets_gauge.reset();
        self.tx_bytes_rate_gauge.reset();
        self.tx_unicast_packets_gauge.reset();
        self.rx_bytes_rate_gauge.reset();
        self.tx_packets_gauge.reset();
        self.tx_collisions_gauge.reset();
        self.tx_packets_rate_gauge.reset();
        self.tx_fcs_gauge.reset();
        self.tx_bytes_gauge.reset();
        self.rx_jabber_packets_gauge.reset();
        self.tx_single_gauge.reset();
        self.tx_excessive_gauge.reset();
        self.rx_pause_gauge.reset();
        self.rx_multicast_packets_gauge.reset();
        self.tx_pause_gauge.reset();
        self.rx_good_packets_gauge.reset();
        self.rx_broadcast_packets_gauge.reset();
        self.tx_multiple_gauge.reset();
        self.tx_deferred_gauge.reset();
        self.tx_late_gauge.reset();
        self.tx_multicast_packets_gauge.reset();
        self.rx_fcs_packets_gauge.reset();
        self.tx_broadcast_packets_gauge.reset();
        self.rx_err_packets_gauge.reset();
        self.rx_fragments_packets_gauge.reset();
        self.rx_bad_bytes_gauge.reset();
        self.rx_undersize_packets_gauge.reset();
        self.port_status_gauge.reset();
        self.port_speed_gauge.reset();
        self.port_mac_list_gauge.reset();
    }

    async fn set_all(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.reset_all();

        let body_status = self.get_ports_status_json().await;

        if body_status.is_err() {
            return Err(Box::new(FreeboxResponseError::new(
                "v4/switch/status/ failed".to_string(),
            )));
        }

        let body_status = body_status.unwrap(); 

        let port_statuses = match self.get_ports_status(&body_status).await {
            Err(e) => return Err(e),
            Ok(r) => r,
        };

        for port_status in port_statuses {
            
            let body_stats = self.get_port_stats_json(&port_status)
                .await;
            
            if body_stats.is_err() {
                return Err(Box::new(FreeboxResponseError::new(
                    "v4/switch/port/{}/stats failed".to_string(),
                )));
            }

            let body_stats = body_stats.unwrap();

            let stats = match self.get_port_stats(&body_stats, port_status.id.as_ref().unwrap()).await {
                Err(e) => return Err(e),
                Ok(r) => r,
            };

            self.rx_packets_rate_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.rx_packets_rate.unwrap_or_default());

            self.rx_good_bytes_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.rx_good_bytes.unwrap_or_default());

            self.rx_oversize_packets_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.rx_oversize_packets.unwrap_or_default());

            self.rx_unicast_packets_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.rx_unicast_packets.unwrap_or_default());

            self.tx_bytes_rate_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.tx_bytes_rate.unwrap_or_default());

            self.tx_unicast_packets_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.tx_unicast_packets.unwrap_or_default());

            self.rx_bytes_rate_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.rx_bytes_rate.unwrap_or_default());

            self.tx_packets_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.tx_packets.unwrap_or_default());

            self.tx_collisions_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.tx_collisions.unwrap_or_default());

            self.tx_packets_rate_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.tx_packets_rate.unwrap_or_default());

            self.tx_fcs_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.tx_fcs.unwrap_or_default());

            self.tx_bytes_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.tx_bytes.unwrap_or_default());

            self.rx_jabber_packets_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.rx_jabber_packets.unwrap_or_default());

            self.tx_single_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.tx_single.unwrap_or_default());

            self.tx_excessive_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.tx_excessive.unwrap_or_default());

            self.rx_pause_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.rx_pause.unwrap_or_default());

            self.rx_multicast_packets_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.rx_multicast_packets.unwrap_or_default());

            self.tx_pause_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.tx_pause.unwrap_or_default());

            self.rx_good_packets_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.rx_good_packets.unwrap_or_default());

            self.rx_broadcast_packets_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.rx_broadcast_packets.unwrap_or_default());

            self.tx_multiple_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.tx_multiple.unwrap_or_default());

            self.tx_deferred_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.tx_deferred.unwrap_or_default());

            self.tx_late_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.tx_late.unwrap_or_default());

            self.tx_multicast_packets_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.tx_multicast_packets.unwrap_or_default());

            self.rx_fcs_packets_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.rx_fcs_packets.unwrap_or_default());

            self.tx_broadcast_packets_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.tx_broadcast_packets.unwrap_or_default());

            self.rx_err_packets_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.rx_err_packets.unwrap_or_default());

            self.rx_fragments_packets_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.rx_fragments_packets.unwrap_or_default());

            self.rx_bad_bytes_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.rx_bad_bytes.unwrap_or_default());

            self.rx_undersize_packets_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(stats.rx_undersize_packets.unwrap_or_default());

            self.port_status_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set((port_status.link.unwrap_or_default() == "up").into());

            self.port_speed_gauge
                .with_label_values(&[&port_status.id.unwrap_or_default().to_string()])
                .set(
                    port_status
                        .speed
                        .unwrap_or("0".to_string())
                        .parse::<i64>()
                        .unwrap_or(0),
                );

            for host in port_status.mac_list.to_owned().unwrap_or_default() {
                self.port_mac_list_gauge
                    .with_label_values(&[
                        &port_status.id.unwrap_or_default().to_string(),
                        &host.mac.unwrap_or_default(),
                        &host.hostname.unwrap_or_default(),
                    ])
                    .set(1);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl<'a> MetricMap<'a> for SwitchMetricMap<'a> {
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Err(e) = self.set_all().await {
            return Err(e);
        }

        Ok(())
    }
}

#[async_trait]
impl DryRunnable for SwitchMetricMap<'_> {

    fn get_name(&self) -> Result<String,Box<dyn std::error::Error> >  {
        Ok("switch".to_string())
    }

    async fn dry_run(&mut self) -> Result<String,Box<dyn std::error::Error>>{
        
        let statuses = self.get_ports_status_json().await;

        if statuses.is_err() {
            return Err(Box::new(FreeboxResponseError::new(
                "v4/switch/status/ failed".to_string(),
            )));
        }

        let statuses = statuses.unwrap();

        let mut result = String::new();
        result.push_str("{");
        result.push_str("\"status\":");        
        result.push_str(&statuses);
        result.push_str(",");
        result.push_str("\"stats\":[");
        
        let port_statuses = match self.get_ports_status(&statuses).await {
            Err(e) => return Err(e),
            Ok(r) => r,
        };

        let mut i = 0;
        let len = port_statuses.len();

        for port_status in port_statuses {
            let body_stats = self.get_port_stats_json(&port_status)
                .await
                .unwrap();

            result.push_str(body_stats.as_str());

            i += 1;
            
            if i < len {
                result.push_str(",");
            }
        }
        result.push_str("]");

        result.push_str("}");

        
        Ok(result)
    }

    fn coerce(&mut self) ->  &mut dyn DryRunnable {
        self
    }
}

lazy_static! {
    // for performance reasons, we compile the regex only once
    static ref REG_MAC: Regex = Regex::new(r#""mac_list"[^\[]+\{\s{0,}}"#).unwrap();
}

#[cfg(test)]
mod non_reg_tests {
    use regex::Regex;

    use super::*;

    // https://github.com/shackerd/freebox-exporter-rs/issues/90
    #[test]
    fn poc_malformed_mac_list() {
        // The output error described in the issue shows that their is a panic when trying to deserialize a sequence, the only sequence in the payload is the mac_list field

        // this is a payload with a malformed mac_list field, it contains an empty object {} instead of an array [] as it should be in the response
        // c.f. https://dev.freebox.fr/sdk/os/switch/#SwitchPortStatus
        let payload = r#"{"success":true,"result":[{"duplex":"full","mac_list":[{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"}],"name":"Ethernet 1","link":"up","id":1,"mode":"100BaseTX-FD","speed":"100","rrd_id":"1"},{"duplex":"full","mac_list":[{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"}],"name":"Ethernet 2","link":"up","id":2,"mode":"100BaseTX-FD","speed":"100","rrd_id":"2"},{"duplex":"full","mac_list":[{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"},{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"},{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"},{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"},{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"}],"name":"Ethernet 3","link":"up","id":3,"mode":"1000BaseT-FD","speed":"1000","rrd_id":"3"},{"duplex":"full","mac_list":[{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"}],"name":"Ethernet 4","link":"up","id":4,"mode":"100BaseTX-FD","speed":"100","rrd_id":"4"},{"duplex":"half","name":"Freeplug","link":"down","id":5,"mode":"10BaseT-HD","speed":"10","rrd_id":"freeplug"},{"duplex":"auto","mac_list":{},"name":"Sfp lan","link":"down","id":6,"mode":"1000BaseT-FD","speed":"1000","rrd_id":"sfp_lan"}]}"#;

        let regex = Regex::new(r#""mac_list"[^\[]+\{\s{0,}}"#).unwrap();
        let fixed_results = regex.replace_all(payload, r#""mac_list":[]"#).to_string();

        let res =
            match serde_json::from_str::<FreeboxResponse<Vec<SwitchPortStatus>>>(&fixed_results) {
                Err(e) => {
                    println!("{:?}", e);
                    panic!()
                }
                Ok(r) => r,
            };

        if !res.success.unwrap_or(false) {
            panic!()
        }

        match res.result {
            None => panic!(),
            Some(r) => {
                assert!(!r
                    .last()
                    .unwrap()
                    .to_owned()
                    .mac_list
                    .unwrap()
                    .iter()
                    .any(|x| x.mac.is_some()));
            }
        }
    }

    #[test]
    fn should_handle_malformed_mac_list_test() {
        let payload = r#"{"success":true,"result":[{"duplex":"full","mac_list":[{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"}],"name":"Ethernet 1","link":"up","id":1,"mode":"100BaseTX-FD","speed":"100","rrd_id":"1"},{"duplex":"full","mac_list":[{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"}],"name":"Ethernet 2","link":"up","id":2,"mode":"100BaseTX-FD","speed":"100","rrd_id":"2"},{"duplex":"full","mac_list":[{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"},{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"},{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"},{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"},{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"}],"name":"Ethernet 3","link":"up","id":3,"mode":"1000BaseT-FD","speed":"1000","rrd_id":"3"},{"duplex":"full","mac_list":[{"mac":"xx:xx:xx:xx:xx:xx","hostname":"some device :)"}],"name":"Ethernet 4","link":"up","id":4,"mode":"100BaseTX-FD","speed":"100","rrd_id":"4"},{"duplex":"half","name":"Freeplug","link":"down","id":5,"mode":"10BaseT-HD","speed":"10","rrd_id":"freeplug"},{"duplex":"auto","mac_list":{},"name":"Sfp lan","link":"down","id":6,"mode":"1000BaseT-FD","speed":"1000","rrd_id":"sfp_lan"}]}"#;
        let res = SwitchMetricMap::handle_malformed_mac_list(payload);
        assert!(res.is_ok());

        // check is the replacement is done correctly
        let reg = Regex::new(r#""mac_list".+\[\s{0,}\]"#).unwrap();
        assert!(reg.is_match(&res.unwrap()));
    }
}
