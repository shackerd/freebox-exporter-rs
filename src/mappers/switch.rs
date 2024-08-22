use async_trait::async_trait;
use log::debug;
use prometheus_exporter::prometheus::{register_int_gauge_vec, IntGaugeVec};
use serde::Deserialize;

use crate::core::common::{AuthenticatedHttpClientFactory, FreeboxResponse, FreeboxResponseError};

use super::MetricMap;

#[derive(Deserialize, Clone, Debug)]
pub struct SwitchPortStatus {
    id: Option<i16>,
    link: Option<String>,
    speed: Option<String>,
    mac_list: Option<Vec<SwitchPortHost>>
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
    rx_undersize_packets: Option<i64>
}

#[derive(Deserialize, Clone, Debug)]
pub struct SwitchPortHost {
    mac: Option<String>,
    hostname: Option<String>
}

pub struct SwitchMetricMap {
    factory: AuthenticatedHttpClientFactory,
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
    port_mac_list_gauge: IntGaugeVec
}

impl SwitchMetricMap {
    pub fn new(factory: AuthenticatedHttpClientFactory, prefix: String) -> Self {
        let prfx: String = format!("{prefix}_switch");
        let stats_prfx: String = format!("{prfx}_stats");

        Self {
            factory,
            rx_packets_rate_gauge: register_int_gauge_vec!(format!("{stats_prfx}_rx_packets_rate"), "rx packet rate", &["port"]).expect(&format!("cannot create {stats_prfx}_rx_packet_rate gauge")),
            rx_good_bytes_gauge: register_int_gauge_vec!(format!("{stats_prfx}_rx_good_bytes"), "rx good bytes", &["port"]).expect(&format!("cannot create {stats_prfx}_rx_good_bytes gauge")),
            rx_oversize_packets_gauge: register_int_gauge_vec!(format!("{stats_prfx}_rx_oversize_packets"), "rx oversize packets", &["port"]).expect(&format!("cannot create {stats_prfx}_rx_oversize_packets gauge")),
            rx_unicast_packets_gauge: register_int_gauge_vec!(format!("{stats_prfx}_rx_unicast_packets"), "rx unicast packets", &["port"]).expect(&format!("cannot create {stats_prfx}_rx_unicast_packets gauge")),
            tx_bytes_rate_gauge: register_int_gauge_vec!(format!("{stats_prfx}_tx_bytes_rate"), "tx bytes rate", &["port"]).expect(&format!("cannot create {stats_prfx}_tx_bytes_rate gauge")),
            tx_unicast_packets_gauge: register_int_gauge_vec!(format!("{stats_prfx}_tx_unicast_packets"), "tx unicast packets", &["port"]).expect(&format!("cannot create {stats_prfx}_tx_unicast_packets gauge")),
            rx_bytes_rate_gauge: register_int_gauge_vec!(format!("{stats_prfx}_rx_bytes_rate"), "rx bytes rate", &["port"]).expect(&format!("cannot create {stats_prfx}_rx_bytes_rate gauge")),
            tx_packets_gauge: register_int_gauge_vec!(format!("{stats_prfx}_tx_packets"), "tx packets", &["port"]).expect(&format!("cannot create {stats_prfx}_tx_packets gauge")),
            tx_collisions_gauge: register_int_gauge_vec!(format!("{stats_prfx}_tx_collisions"), "tx collisions", &["port"]).expect(&format!("cannot create {stats_prfx}_tx_collisions gauge")),
            tx_packets_rate_gauge: register_int_gauge_vec!(format!("{stats_prfx}_tx_packets_rate"), "tx packets rate", &["port"]).expect(&format!("cannot create {stats_prfx}_tx_packets_rate gauge")),
            tx_fcs_gauge: register_int_gauge_vec!(format!("{stats_prfx}_tx_fcs"), "tx fcs", &["port"]).expect(&format!("cannot create {stats_prfx}_tx_fcs gauge")),
            tx_bytes_gauge: register_int_gauge_vec!(format!("{stats_prfx}_tx_bytes"), "tx bytes", &["port"]).expect(&format!("cannot create {stats_prfx}_tx_bytes gauge")),
            rx_jabber_packets_gauge: register_int_gauge_vec!(format!("{stats_prfx}_rx_jabber_packets"), "rx jabber packets", &["port"]).expect(&format!("cannot create {stats_prfx}_rx_jabber_packets gauge")),
            tx_single_gauge: register_int_gauge_vec!(format!("{stats_prfx}_tx_single"), "tx single", &["port"]).expect(&format!("cannot create {stats_prfx}_tx_single gauge")),
            tx_excessive_gauge: register_int_gauge_vec!(format!("{stats_prfx}_tx_excessive"), "tx excessive", &["port"]).expect(&format!("cannot create {stats_prfx}_tx_excessive gauge")),
            rx_pause_gauge: register_int_gauge_vec!(format!("{stats_prfx}_rx_pause"), "rx pause", &["port"]).expect(&format!("cannot create {stats_prfx}_rx_pause gauge")),
            rx_multicast_packets_gauge: register_int_gauge_vec!(format!("{stats_prfx}_rx_multicast_packets"), "rx multicast packets", &["port"]).expect(&format!("cannot create {stats_prfx}_rx_multicast_packets gauge")),
            tx_pause_gauge: register_int_gauge_vec!(format!("{stats_prfx}_tx_pause"), "tx pause", &["port"]).expect(&format!("cannot create {stats_prfx}_tx_pause gauge")),
            rx_good_packets_gauge: register_int_gauge_vec!(format!("{stats_prfx}_rx_good_packets"), "tx good packets", &["port"]).expect(&format!("cannot create {stats_prfx}_rx_good_packets gauge")),
            rx_broadcast_packets_gauge: register_int_gauge_vec!(format!("{stats_prfx}_rx_broadcast_packets"), "rx broadcast packets", &["port"]).expect(&format!("cannot create {stats_prfx}_rx_broadcast_packets gauge")),
            tx_multiple_gauge: register_int_gauge_vec!(format!("{stats_prfx}_tx_multiple"), "tx multiple", &["port"]).expect(&format!("cannot create {stats_prfx}_tx_multiple gauge")),
            tx_deferred_gauge: register_int_gauge_vec!(format!("{stats_prfx}_tx_deferred"), "tx deferred", &["port"]).expect(&format!("cannot create {stats_prfx}_tx_deferred gauge")),
            tx_late_gauge: register_int_gauge_vec!(format!("{stats_prfx}_tx_late"), "tx late", &["port"]).expect(&format!("cannot create {stats_prfx}_tx_late gauge")),
            tx_multicast_packets_gauge: register_int_gauge_vec!(format!("{stats_prfx}_tx_multicast_packets"), "tx multicast packets", &["port"]).expect(&format!("cannot create {stats_prfx}_tx_multicast_packets gauge")),
            rx_fcs_packets_gauge: register_int_gauge_vec!(format!("{stats_prfx}_rx_fcs_packets"), "rx fcs packets", &["port"]).expect(&format!("cannot create {stats_prfx}_rx_fcs_packets gauge")),
            tx_broadcast_packets_gauge: register_int_gauge_vec!(format!("{stats_prfx}_tx_broadcast_packets"), "tx broadcast packets", &["port"]).expect(&format!("cannot create {stats_prfx}_tx_broadcast_packets gauge")),
            rx_err_packets_gauge: register_int_gauge_vec!(format!("{stats_prfx}_rx_err_packets"), "rx err packets", &["port"]).expect(&format!("cannot create {stats_prfx}_rx_err_packets gauge")),
            rx_fragments_packets_gauge: register_int_gauge_vec!(format!("{stats_prfx}_rx_fragments_packets"), "rx fragments packets", &["port"]).expect(&format!("cannot create {stats_prfx}_rx_fragments_packets gauge")),
            rx_bad_bytes_gauge: register_int_gauge_vec!(format!("{stats_prfx}_rx_bad_bytes"), "rx bad bytes", &["port"]).expect(&format!("cannot create {stats_prfx}_rx_bad_bytes gauge")),
            rx_undersize_packets_gauge: register_int_gauge_vec!(format!("{stats_prfx}_rx_undersize_packets"), "rx undersize packets", &["port"]).expect(&format!("cannot create {stats_prfx}_rx_undersize_packets gauge")),
            port_status_gauge: register_int_gauge_vec!(format!("{prfx}_port_status"), "port status, 1 for link up", &["port"]).expect(&format!("cannot create {prfx}_port_status gauge")),
            port_speed_gauge: register_int_gauge_vec!(format!("{prfx}_port_speed"), "port status speed", &["port"]).expect(&format!("cannot create {prfx}_port_speed gauge")),
            port_mac_list_gauge: register_int_gauge_vec!(format!("{prfx}_port_mac_list"), "port mac list, always 1", &["port", "mac", "hostname"]).expect(&format!("cannot create {prfx}_port_mac_list gauge")),
        }
    }

    async fn get_ports_status(&self) -> Result<Vec<SwitchPortStatus>, Box<dyn std::error::Error>> {

        debug!("fetching switch ports statuses");

        let body =
            self.factory.create_client().await.unwrap().get(format!("{}v4/switch/status/", self.factory.api_url))
            .send().await?
            .text().await?;

        let res = match serde_json::from_str::<FreeboxResponse<Vec<SwitchPortStatus>>>(&body)
            { Err(e) => return Err(Box::new(e)), Ok(r) => r };

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(res.msg.unwrap_or_default())));
        }

        let statuses = match res.result
            { None => return Err(Box::new(FreeboxResponseError::new("v4/switch/status/ response was empty".to_string()))), Some(r) => r};

        Ok(statuses)
    }

    async fn get_port_stats(&self, port_status: &SwitchPortStatus) -> Result<SwitchPortStats, Box<dyn std::error::Error>> {

        debug!("fetching switch ports stats");

        let port_id = port_status.id.unwrap_or_default();

        let body =
            self.factory.create_client().await.unwrap().get(format!("{}v4/switch/port/{}/stats", self.factory.api_url, port_id))
            .send().await?
            .text().await?;

        let res = match serde_json::from_str::<FreeboxResponse<SwitchPortStats>>(&body)
            { Err(e) => return Err(Box::new(e)), Ok(r) => r };

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(res.msg.unwrap_or_default())));
        }

        match res.result {
            None => return Err(Box::new(FreeboxResponseError::new(format!("v4/switch/port/{}/stats response was empty", port_id)))),
            Some(r) =>  Ok(r)
        }
    }

    async fn set_all(&self) -> Result<(), Box<dyn std::error::Error>> {

        let port_statuses = match self.get_ports_status().await
            { Err(e) => return Err(e), Ok(r) => r };

        for port_status in port_statuses {
            let stats = match self.get_port_stats(&port_status).await
                { Err(e) => return Err(e), Ok(r) => r };

            self.rx_packets_rate_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.rx_packets_rate.unwrap_or_default());
            self.rx_good_bytes_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.rx_good_bytes.unwrap_or_default());
            self.rx_oversize_packets_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.rx_oversize_packets.unwrap_or_default());
            self.rx_unicast_packets_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.rx_unicast_packets.unwrap_or_default());
            self.tx_bytes_rate_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.tx_bytes_rate.unwrap_or_default());
            self.tx_unicast_packets_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.tx_unicast_packets.unwrap_or_default());
            self.rx_bytes_rate_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.rx_bytes_rate.unwrap_or_default());
            self.tx_packets_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.tx_packets.unwrap_or_default());
            self.tx_collisions_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.tx_collisions.unwrap_or_default());
            self.tx_packets_rate_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.tx_packets_rate.unwrap_or_default());
            self.tx_fcs_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.tx_fcs.unwrap_or_default());
            self.tx_bytes_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.tx_bytes.unwrap_or_default());
            self.rx_jabber_packets_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.rx_jabber_packets.unwrap_or_default());
            self.tx_single_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.tx_single.unwrap_or_default());
            self.tx_excessive_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.tx_excessive.unwrap_or_default());
            self.rx_pause_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.rx_pause.unwrap_or_default());
            self.rx_multicast_packets_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.rx_multicast_packets.unwrap_or_default());
            self.tx_pause_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.tx_pause.unwrap_or_default());
            self.rx_good_packets_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.rx_good_packets.unwrap_or_default());
            self.rx_broadcast_packets_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.rx_broadcast_packets.unwrap_or_default());
            self.tx_multiple_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.tx_multiple.unwrap_or_default());
            self.tx_deferred_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.tx_deferred.unwrap_or_default());
            self.tx_late_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.tx_late.unwrap_or_default());
            self.tx_multicast_packets_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.tx_multicast_packets.unwrap_or_default());
            self.rx_fcs_packets_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.rx_fcs_packets.unwrap_or_default());
            self.tx_broadcast_packets_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.tx_broadcast_packets.unwrap_or_default());
            self.rx_err_packets_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.rx_err_packets.unwrap_or_default());
            self.rx_fragments_packets_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.rx_fragments_packets.unwrap_or_default());
            self.rx_bad_bytes_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.rx_bad_bytes.unwrap_or_default());
            self.rx_undersize_packets_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(stats.rx_undersize_packets.unwrap_or_default());

            self.port_status_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set((port_status.link.unwrap_or_default() == "up").into());
            self.port_speed_gauge.with_label_values(&[&port_status.id.unwrap_or_default().to_string()]).set(i64::from_str_radix(&port_status.speed.unwrap_or("0".to_string()), 10).unwrap());

            for host in port_status.mac_list.to_owned().unwrap_or_default() {
                self.port_mac_list_gauge.with_label_values(
                    &[&port_status.id.unwrap_or_default().to_string(), &host.mac.unwrap_or_default(), &host.hostname.unwrap_or_default()]
                ).set(1);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl MetricMap for SwitchMetricMap {

    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }

    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.set_all().await { Err(e) => return Err(e), _ => {} };
        Ok(())
    }
}