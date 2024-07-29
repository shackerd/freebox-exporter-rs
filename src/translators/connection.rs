use async_trait::async_trait;
use log::debug;
use prometheus_exporter::prometheus::{register_int_gauge, register_int_gauge_vec, IntGauge, IntGaugeVec};
use serde::Deserialize;

use crate::core::common::{AuthenticatedHttpClientFactory, FreeboxResponse};

use super::TranslatorMetricTap;

#[derive(Deserialize, Debug)]
pub struct ConnectionStatus {
    #[serde(alias="type")]
    _type: Option<String>,
    rate_down: Option<i64>,
    bytes_up: Option<i64>,
    rate_up: Option<i64>,
    bandwidth_up: Option<i64>,
    ipv4: Option<String>,
    ipv6: Option<String>,
    bandwidth_down: Option<i64>,
    state: Option<String>,
    bytes_down: Option<i64>,
    media: Option<String>
}

#[derive(Deserialize, Debug)]
pub struct ConnectionConfiguration {
    ping: Option<bool>,
    is_secure_pass: Option<bool>,
    remote_access_port: Option<u16>,
    remote_access: Option<bool>,
    wol: Option<bool>,
    adblock: Option<bool>,
    adblock_not_set: Option<bool>,
    api_remote_access: Option<bool>,
    allow_token_request: Option<bool>,
    remote_access_ip: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ConnectionIpv6Delegation {
    prefix: Option<String>,
    next_hop: Option<String>
}

#[derive(Deserialize, Debug)]
pub struct ConnectionIpv6Configuration {
    ipv6_enabled: Option<bool>,
    delegations: Option<Vec<ConnectionIpv6Delegation>>
}

#[derive(Deserialize, Debug)]
pub struct ConnectionFtth {
    sfp_has_power_report: Option<bool>,
    sfp_has_signal: Option<bool>,
    sfp_model: Option<String>,
    sfp_vendor: Option<String>,
    sfp_pwr_tx: Option<i64>,
    sfp_pwr_rx: Option<i64>,
    link: Option<bool>,
    sfp_alim_ok: Option<bool>,
    sfp_serial: Option<String>,
    sfp_present: Option<bool>
}

pub struct ConnectionTap {
    factory: AuthenticatedHttpClientFactory,
    bytes_down_metric:  IntGauge,
    bytes_up_metric:  IntGauge,
    rate_down_metric:  IntGauge,
    rate_up_metric:  IntGauge,
    bandwidth_down_metric:  IntGauge,
    bandwidth_up_metric:  IntGauge,
    type_metric: IntGaugeVec,
    media_metric: IntGaugeVec,
    state_metric: IntGaugeVec,
    ipv4_metric: IntGaugeVec,
    ipv6_metric: IntGaugeVec,
    ping_metric: IntGauge,
    is_secure_pass_metric: IntGauge,
    remote_access_port_metric: IntGauge,
    remote_access_metric: IntGauge,
    wol_metric: IntGauge,
    adblock_metric: IntGauge,
    adblock_not_set_metric: IntGauge,
    api_remote_access_metric: IntGauge,
    allow_token_request_metric: IntGauge,
    remote_access_ip_metric: IntGaugeVec,
    ipv6_enabled_metric: IntGauge,
    delegations_metric: IntGaugeVec,
    sfp_has_power_report_metric: IntGauge,
    sfp_has_signal_metric: IntGauge,
    sfp_model_metric: IntGaugeVec,
    sfp_vendor_metric: IntGaugeVec,
    sfp_pwr_tx_metric: IntGauge,
    sfp_pwr_rx_metric: IntGauge,
    link_metric: IntGauge,
    sfp_alim_ok_metric: IntGauge,
    sfp_serial_metric: IntGaugeVec,
    sfp_present_metric: IntGauge
}

impl ConnectionTap {
    pub fn new(factory: AuthenticatedHttpClientFactory) -> Self {

        Self {
            factory,
            bytes_down_metric : register_int_gauge!("connection_bytes_down", "connection_bytes_down").expect("cannot create connection_bytes_down gauge"),
            bytes_up_metric : register_int_gauge!("connection_bytes_up", "connection_bytes_up").expect("cannot create connection_bytes_up gauge"),
            rate_down_metric : register_int_gauge!("connection_rate_down", "connection_rate_down").expect("cannot create connection_rate_down gauge"),
            rate_up_metric : register_int_gauge!("connection_rate_up", "connection_rate_up").expect("cannot create connection_rate_up gauge"),
            bandwidth_down_metric : register_int_gauge!("connection_bandwidth_down", "connection_bandwidth_down").expect("cannot create connection_bandwidth_down gauge"),
            bandwidth_up_metric : register_int_gauge!("connection_bandwidth_up", "connection_bandwidth_up").expect("cannot create connection_bandwidth_up gauge"),
            type_metric : register_int_gauge_vec!("connection_type", "connection_type", &["type"]).expect("cannot create connection_type gauge"),
            media_metric : register_int_gauge_vec!("connection_media", "connection_media", &["media"]).expect("cannot create connection_media gauge"),
            state_metric : register_int_gauge_vec!("connection_state", "connection_state", &["state"]).expect("cannot create connection_state gauge"),
            ipv4_metric : register_int_gauge_vec!("connection_ipv4", "connection_ipv4", &["ipv4"]).expect("cannot create connection_ipv4 gauge"),
            ipv6_metric : register_int_gauge_vec!("connection_ipv6", "connection_ipv6", &["ipv6"]).expect("cannot create connection_ipv6 gauge"),
            ping_metric: register_int_gauge!("connection_conf_ping", "connection_conf_ping").expect("cannot create connection_conf_ping gauge"),
            is_secure_pass_metric: register_int_gauge!("connection_conf_is_secure_pass", "connection_conf_is_secure_pass").expect("cannot create connection_conf_is_secure_pass gauge"),
            remote_access_port_metric: register_int_gauge!("connection_conf_remote_access_port", "connection_conf_remote_access_port").expect("cannot create connection_conf_remote_access_port gauge"),
            remote_access_metric: register_int_gauge!("connection_conf_remote_access", "connection_conf_remote_access").expect("cannot create connection_conf_remote_access gauge"),
            wol_metric: register_int_gauge!("connection_wol_conf", "connection_conf_wol").expect("cannot create connection_conf_wol gauge"),
            adblock_metric: register_int_gauge!("connection_conf_adblock", "connection_conf_adblock").expect("cannot create connection_conf_adblock gauge"),
            adblock_not_set_metric: register_int_gauge!("connection_conf_adblock_not_set", "connection_conf_adblock_not_set").expect("cannot create connection_conf_adblock_not_set"),
            api_remote_access_metric: register_int_gauge!("connection_conf_api_remote_access", "connection_conf_api_remote_access").expect("cannot create connection_conf_api_remote_access gauge"),
            allow_token_request_metric: register_int_gauge!("connection_conf_allow_token_request", "connection_conf_allow_token_request").expect("cannot create connection_conf_allow_token_request gauge"),
            remote_access_ip_metric: register_int_gauge_vec!("connection_conf_remote_access_ip", "connection_conf_remote_access_ip", &["remote_access_ip"]).expect("cannot create connection_conf_remote_access_ip gauge"),
            ipv6_enabled_metric: register_int_gauge!("connection_ipv6_conf_ipv6_enabled", "connection_ipv6_conf_ipv6_enabled").expect("cannot create connection_ipv6_conf_ipv6_enabled"),
            delegations_metric: register_int_gauge_vec!("connection_ipv6_conf_delegations", "connection_ipv6_conf_delegations", &["prefix", "next_hop"]).expect("cannot create connection_ipv6_conf_delegations"),
            sfp_has_power_report_metric: register_int_gauge!("connection_ftth_sfp_has_power_report", "connection_ftth_sfp_has_power_report").expect("cannot create connection_ftth_sfp_has_power_report gauge"),
            sfp_has_signal_metric: register_int_gauge!("connection_ftth_sfp_has_signal", "connection_ftth_sfp_has_signal").expect("cannot create connection_ftth_sfp_has_signal gauge"),
            sfp_model_metric: register_int_gauge_vec!("connection_ftth_sfp_model", "connection_ftth_sfp_model", &["sfp_model"]).expect("cannot create connection_ftth_sfp_model gauge"),
            sfp_vendor_metric: register_int_gauge_vec!("connection_ftth_sfp_vendor", "connection_ftth_sfp_vendor", &["sfp_vendor"]).expect("cannot create connection_ftth_sfp_vendor gauge"),
            sfp_pwr_tx_metric: register_int_gauge!("connection_ftth_sfp_pwr_tx", "connection_ftth_sfp_pwr_tx").expect("cannot create connection_ftth_sfp_pwr_tx gauge"),
            sfp_pwr_rx_metric: register_int_gauge!("connection_ftth_sfp_pwr_rx", "connection_ftth_sfp_pwr_rx").expect("cannot create connection_ftth_sfp_pwr_rx gauge"),
            link_metric: register_int_gauge!("connection_ftth_link", "connection_ftth_link").expect("cannot create connection_ftth_link gauge"),
            sfp_alim_ok_metric: register_int_gauge!("connection_ffth_sfp_alim_ok", "connection_ffth_sfp_alim_ok").expect("cannot create connection_ffth_sfp_alim_ok gauge"),
            sfp_serial_metric: register_int_gauge_vec!("connection_ftth_sfp_serial", "connection_ftth_sfp_serial", &["sfp_serial"]).expect("cannot create connection_ftth_sfp_serial gauge"),
            sfp_present_metric: register_int_gauge!("connection_ffth_sfp_present", "connection_ffth_sfp_present").expect("cannot create connection_ffth_sfp_present gauge")
        }
    }

    async fn set_connection_ftth(&self) -> Result<(), Box<dyn std::error::Error>> {

        debug!("fetching connection ftth");

        let body =
            self.factory.create_client().unwrap().get(format!("{}v4/connection/ftth", self.factory.api_url))
            .send().await?
            .text().await?;

        let res = serde_json::from_str::<FreeboxResponse<ConnectionFtth>>(&body);

        let ftth = res.expect("Cannot read response").result;

        self.sfp_has_power_report_metric.set(ftth.sfp_has_power_report.unwrap_or_default().into());
        self.sfp_has_signal_metric.set(ftth.sfp_has_signal.unwrap_or_default().into());
        self.sfp_model_metric.with_label_values(&[&ftth.sfp_model.clone().unwrap_or_default()]).set(ftth.sfp_model.is_some().into());
        self.sfp_vendor_metric.with_label_values(&[&ftth.sfp_vendor.clone().unwrap_or_default()]).set(ftth.sfp_vendor.is_some().into());
        self.sfp_pwr_tx_metric.set(ftth.sfp_pwr_tx.unwrap_or_default());
        self.sfp_pwr_rx_metric.set(ftth.sfp_pwr_rx.unwrap_or_default());
        self.link_metric.set(ftth.link.unwrap_or_default().into());
        self.sfp_alim_ok_metric.set(ftth.sfp_alim_ok.unwrap_or_default().into());
        self.sfp_serial_metric.with_label_values(&[&ftth.sfp_serial.clone().unwrap_or_default()]).set(ftth.sfp_serial.is_some().into());
        self.sfp_present_metric.set(ftth.sfp_present.unwrap_or_default().into());
        Ok(())
    }

    async fn set_connection_status(&self) -> Result<(), Box<dyn std::error::Error>> {

        debug!("fetching connection status");
        let body =
            self.factory.create_client().unwrap().get(format!("{}v4/connection", self.factory.api_url))
            .send().await?
            .text().await?;

        let res = serde_json::from_str::<FreeboxResponse<ConnectionStatus>>(&body);

        let status = res.expect("Cannot read response").result;

        self.type_metric.with_label_values(&[&status._type.unwrap()]).set(1);
        self.state_metric.with_label_values(&["up"]).set(if status.state.unwrap() == "up" { 1 } else { 0 } );
        self.media_metric.with_label_values(&[&status.media.unwrap()]).set(1);
        self.ipv4_metric.with_label_values(&[&status.ipv4.unwrap()]).set(1);
        self.ipv6_metric.with_label_values(&[&status.ipv6.unwrap()]).set(1);
        self.bytes_down_metric.set(status.bytes_down.unwrap());
        self.bytes_up_metric.set(status.bytes_up.unwrap());
        self.rate_down_metric.set(status.rate_down.unwrap());
        self.rate_up_metric.set(status.rate_up.unwrap());
        self.bandwidth_down_metric.set(status.bandwidth_down.unwrap());
        self.bandwidth_up_metric.set(status.bandwidth_up.unwrap());

        Ok(())
    }

    async fn set_connection_conf(&self) -> Result<(), Box<dyn std::error::Error>> {

        debug!("fetching connection configuration");

        let body =
            self.factory.create_client().unwrap().get(format!("{}v4/connection/config", self.factory.api_url))
            .send().await?
            .text().await?;

        let res = serde_json::from_str::<FreeboxResponse<ConnectionConfiguration>>(&body);

        let conf = res.expect("Cannot read response").result;

        self.ping_metric.set(conf.ping.unwrap_or_default().into());
        self.is_secure_pass_metric.set(conf.is_secure_pass.unwrap_or_default().into());
        self.remote_access_port_metric.set(conf.remote_access_port.unwrap_or_else(|| 0).into());
        self.remote_access_metric.set(conf.remote_access.unwrap_or_default().into());
        self.wol_metric.set(conf.wol.unwrap_or_default().into());
        self.adblock_metric.set(conf.adblock.unwrap_or_default().into());
        self.adblock_not_set_metric.set(conf.adblock_not_set.unwrap_or_default().into());
        self.api_remote_access_metric.set(conf.api_remote_access.unwrap_or_default().into());
        self.allow_token_request_metric.set(conf.allow_token_request.unwrap_or_default().into());
        self.remote_access_ip_metric.with_label_values(&[&conf.remote_access_ip.unwrap_or_else(|| String::new())]).set(conf.remote_access.is_some().into());

        Ok(())
    }

    async fn set_connection_ipv6_conf(&self) -> Result<(), Box<dyn std::error::Error>> {

        debug!("fetching connection ipv6 configuration");

        let body =
            self.factory.create_client().unwrap().get(format!("{}v4/connection/ipv6/config", self.factory.api_url))
            .send().await?
            .text().await?;

        let res = serde_json::from_str::<FreeboxResponse<ConnectionIpv6Configuration>>(&body);

        let conf = res.expect("Cannot read response").result;

        self.ipv6_enabled_metric.set(conf.ipv6_enabled.unwrap_or_default().into());

        if conf.delegations.is_some() {
            for delegation in conf.delegations.unwrap() {

                self.delegations_metric.with_label_values(&[&delegation.prefix.unwrap(), &delegation.next_hop.unwrap()]).set(1);
            }
        }

        Ok(())
    }
}

#[async_trait]
impl TranslatorMetricTap for ConnectionTap {

    async fn set(&self) -> Result<(), Box<dyn std::error::Error>> {

        self.set_connection_status().await.expect("cannot set connection status gauges");
        self.set_connection_conf().await.expect("cannot set connection configuration gauges");
        self.set_connection_ipv6_conf().await.expect("cannot set connection ipv6 configuration gauges");
        self.set_connection_ftth().await.expect("cannot set connection ftth gauges");
        Ok(())
    }
}