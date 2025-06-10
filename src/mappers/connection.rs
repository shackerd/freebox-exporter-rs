use async_trait::async_trait;
use log::{debug, info};
use models::{
    ConnectionConfiguration, ConnectionFtth, ConnectionIpv6Configuration, ConnectionStatus,
    XdslInfo, XdslStats,
};
use prometheus_exporter::prometheus::{
    register_int_gauge, register_int_gauge_vec, IntGauge, IntGaugeVec,
};
use reqwest::Client;
use std::error::Error;

use super::MetricMap;
use crate::diagnostics::DryRunOutputWriter;
use crate::{
    core::common::{
        http_client_factory::{AuthenticatedHttpClientFactory, ManagedHttpClient},
        transport::{FreeboxResponse, FreeboxResponseError},
    },
    diagnostics::DryRunnable,
};
mod models;
mod unittests;

pub struct ConnectionMetricMap<'a> {
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    is_ftth: Option<bool>,
    managed_client: Option<ManagedHttpClient>,
    bytes_down_metric: IntGauge,
    bytes_up_metric: IntGauge,
    rate_down_metric: IntGauge,
    rate_up_metric: IntGauge,
    bandwidth_down_metric: IntGauge,
    bandwidth_up_metric: IntGauge,
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
    sfp_present_metric: IntGauge,
    xdsl_status_uptime: IntGaugeVec,
    xdsl_stats_maxrate: IntGaugeVec,
    xdsl_stats_rate: IntGaugeVec,
    xdsl_stats_snr: IntGaugeVec,
    xdsl_stats_attn: IntGaugeVec,
    xdsl_stats_fec: IntGaugeVec,
    xdsl_stats_crc: IntGaugeVec,
    xdsl_stats_hec: IntGaugeVec,
    xdsl_stats_es: IntGaugeVec,
    xdsl_stats_ses: IntGaugeVec,
    xdsl_stats_rxmt: IntGaugeVec,
    xdsl_stats_rxmt_corr: IntGaugeVec,
    xdsl_stats_rxmt_uncorr: IntGaugeVec,
    xdsl_stats_rtx_tx: IntGaugeVec,
    xdsl_stats_rtx_c: IntGaugeVec,
    xdsl_stats_rtx_uc: IntGaugeVec,
}

impl<'a> ConnectionMetricMap<'a> {
    pub fn new(factory: &'a AuthenticatedHttpClientFactory<'a>, prefix: String) -> Self {
        Self {
            factory,
            is_ftth: None,
            managed_client: None,
            bytes_down_metric: register_int_gauge!(
                format!("{prefix}_connection_bytes_down"),
                format!("{prefix}_connection_bytes_down")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_bytes_down gauge"
            )),
            bytes_up_metric: register_int_gauge!(
                format!("{prefix}_connection_bytes_up"),
                format!("{prefix}_connection_bytes_up")
            )
            .expect(&format!("cannot create {prefix}_connection_bytes_up gauge")),
            rate_down_metric: register_int_gauge!(
                format!("{prefix}_connection_rate_down"),
                format!("{prefix}_connection_rate_down")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_rate_down gauge"
            )),
            rate_up_metric: register_int_gauge!(
                format!("{prefix}_connection_rate_up"),
                format!("{prefix}_connection_rate_up")
            )
            .expect(&format!("cannot create {prefix}_connection_rate_up gauge")),
            bandwidth_down_metric: register_int_gauge!(
                format!("{prefix}_connection_bandwidth_down"),
                format!("{prefix}_connection_bandwidth_down")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_bandwidth_down gauge"
            )),
            bandwidth_up_metric: register_int_gauge!(
                format!("{prefix}_connection_bandwidth_up"),
                format!("{prefix}_connection_bandwidth_up")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_bandwidth_up gauge"
            )),
            type_metric: register_int_gauge_vec!(
                format!("{prefix}_connection_type"),
                format!("{prefix}_connection_type"),
                &["type"]
            )
            .expect(&format!("cannot create {prefix}_connection_type gauge")),
            media_metric: register_int_gauge_vec!(
                format!("{prefix}_connection_media"),
                format!("{prefix}_connection_media"),
                &["media"]
            )
            .expect(&format!("cannot create {prefix}_connection_media gauge")),
            state_metric: register_int_gauge_vec!(
                format!("{prefix}_connection_state"),
                format!("{prefix}_connection_state"),
                &["state"]
            )
            .expect(&format!("cannot create {prefix}_connection_state gauge")),
            ipv4_metric: register_int_gauge_vec!(
                format!("{prefix}_connection_ipv4"),
                format!("{prefix}_connection_ipv4"),
                &["ipv4"]
            )
            .expect(&format!("cannot create {prefix}_connection_ipv4 gauge")),
            ipv6_metric: register_int_gauge_vec!(
                format!("{prefix}_connection_ipv6"),
                format!("{prefix}_connection_ipv6"),
                &["ipv6"]
            )
            .expect(&format!("cannot create {prefix}_connection_ipv6 gauge")),
            ping_metric: register_int_gauge!(
                format!("{prefix}_connection_conf_ping"),
                format!("{prefix}_connection_conf_ping")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_conf_ping gauge"
            )),
            is_secure_pass_metric: register_int_gauge!(
                format!("{prefix}_connection_conf_is_secure_pass"),
                format!("{prefix}_connection_conf_is_secure_pass")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_conf_is_secure_pass gauge"
            )),
            remote_access_port_metric: register_int_gauge!(
                format!("{prefix}_connection_conf_remote_access_port"),
                format!("{prefix}_connection_conf_remote_access_port")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_conf_remote_access_port gauge"
            )),
            remote_access_metric: register_int_gauge!(
                format!("{prefix}_connection_conf_remote_access"),
                format!("{prefix}_connection_conf_remote_access")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_conf_remote_access gauge"
            )),
            wol_metric: register_int_gauge!(
                format!("{prefix}_connection_wol_conf"),
                format!("{prefix}_connection_conf_wol")
            )
            .expect(&format!("cannot create {prefix}_connection_conf_wol gauge")),
            adblock_metric: register_int_gauge!(
                format!("{prefix}_connection_conf_adblock"),
                format!("{prefix}_connection_conf_adblock")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_conf_adblock gauge"
            )),
            adblock_not_set_metric: register_int_gauge!(
                format!("{prefix}_connection_conf_adblock_not_set"),
                format!("{prefix}_connection_conf_adblock_not_set")
            )
            .expect(&format!(
                "cannot {prefix}_create connection_conf_adblock_not_set"
            )),
            api_remote_access_metric: register_int_gauge!(
                format!("{prefix}_connection_conf_api_remote_access"),
                format!("{prefix}_connection_conf_api_remote_access")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_conf_api_remote_access gauge"
            )),
            allow_token_request_metric: register_int_gauge!(
                format!("{prefix}_connection_conf_allow_token_request"),
                format!("{prefix}_connection_conf_allow_token_request")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_conf_allow_token_request gauge"
            )),
            remote_access_ip_metric: register_int_gauge_vec!(
                format!("{prefix}_connection_conf_remote_access_ip"),
                format!("{prefix}_connection_conf_remote_access_ip"),
                &["remote_access_ip"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_conf_remote_access_ip gauge"
            )),
            ipv6_enabled_metric: register_int_gauge!(
                format!("{prefix}_connection_ipv6_conf_ipv6_enabled"),
                format!("{prefix}_connection_ipv6_conf_ipv6_enabled")
            )
            .expect(&format!(
                "cannot create {prefix}_create connection_ipv6_conf_ipv6_enabled"
            )),
            delegations_metric: register_int_gauge_vec!(
                format!("{prefix}_connection_ipv6_conf_delegations"),
                format!("{prefix}_connection_ipv6_conf_delegations"),
                &["prefix", "next_hop"]
            )
            .expect(&format!(
                "cannot create {prefix}_create connection_ipv6_conf_delegations"
            )),
            sfp_has_power_report_metric: register_int_gauge!(
                format!("{prefix}_connection_ftth_sfp_has_power_report"),
                format!("{prefix}_connection_ftth_sfp_has_power_report")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_ftth_sfp_has_power_report gauge"
            )),
            sfp_has_signal_metric: register_int_gauge!(
                format!("{prefix}_connection_ftth_sfp_has_signal"),
                format!("{prefix}_connection_ftth_sfp_has_signal")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_ftth_sfp_has_signal gauge"
            )),
            sfp_model_metric: register_int_gauge_vec!(
                format!("{prefix}_connection_ftth_sfp_model"),
                format!("{prefix}_connection_ftth_sfp_model"),
                &["sfp_model"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_ftth_sfp_model gauge"
            )),
            sfp_vendor_metric: register_int_gauge_vec!(
                format!("{prefix}_connection_ftth_sfp_vendor"),
                format!("{prefix}_connection_ftth_sfp_vendor"),
                &["sfp_vendor"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_ftth_sfp_vendor gauge"
            )),
            sfp_pwr_tx_metric: register_int_gauge!(
                format!("{prefix}_connection_ftth_sfp_pwr_tx"),
                format!("{prefix}_connection_ftth_sfp_pwr_tx")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_ftth_sfp_pwr_tx gauge"
            )),
            sfp_pwr_rx_metric: register_int_gauge!(
                format!("{prefix}_connection_ftth_sfp_pwr_rx"),
                format!("{prefix}_connection_ftth_sfp_pwr_rx")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_ftth_sfp_pwr_rx gauge"
            )),
            link_metric: register_int_gauge!(
                format!("{prefix}_connection_ftth_link"),
                format!("{prefix}_connection_ftth_link")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_ftth_link gauge"
            )),
            sfp_alim_ok_metric: register_int_gauge!(
                format!("{prefix}_connection_ffth_sfp_alim_ok"),
                format!("{prefix}_connection_ffth_sfp_alim_ok")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_ffth_sfp_alim_ok gauge"
            )),
            sfp_serial_metric: register_int_gauge_vec!(
                format!("{prefix}_connection_ftth_sfp_serial"),
                format!("{prefix}_connection_ftth_sfp_serial"),
                &["sfp_serial"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_ftth_sfp_serial gauge"
            )),
            sfp_present_metric: register_int_gauge!(
                format!("{prefix}_connection_ffth_sfp_present"),
                format!("{prefix}_connection_ffth_sfp_present")
            )
            .expect(&format!(
                "cannot create {prefix}_connection_ffth_sfp_present gauge"
            )),
            xdsl_status_uptime: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_status_uptime"),
                format!("{prefix}_connection_xdsl_status_uptime"),
                &["status", "protocol", "modulation"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_status_uptime gauge"
            )),
            xdsl_stats_maxrate: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_stats_maxrate"),
                format!("{prefix}_connection_xdsl_stats_maxrate"),
                &["direction"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_stats_maxrate gauge"
            )),
            xdsl_stats_rate: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_stats_rate"),
                format!("{prefix}_connection_xdsl_stats_rate"),
                &["direction"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_stats_rate gauge"
            )),
            xdsl_stats_snr: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_stats_snr"),
                format!("{prefix}_connection_xdsl_stats_snr"),
                &["direction"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_stats_snr gauge"
            )),
            xdsl_stats_attn: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_stats_attn"),
                format!("{prefix}_connection_xdsl_stats_attn"),
                &["direction"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_stats_attn gauge"
            )),
            xdsl_stats_fec: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_stats_fec"),
                format!("{prefix}_connection_xdsl_stats_fec"),
                &["direction"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_stats_fec gauge"
            )),
            xdsl_stats_crc: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_stats_crc"),
                format!("{prefix}_connection_xdsl_stats_crc"),
                &["direction"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_stats_crc gauge"
            )),
            xdsl_stats_hec: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_stats_hec"),
                format!("{prefix}_connection_xdsl_stats_hec"),
                &["direction"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_stats_hec gauge"
            )),
            xdsl_stats_es: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_stats_es"),
                format!("{prefix}_connection_xdsl_stats_es"),
                &["direction"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_stats_es gauge"
            )),
            xdsl_stats_ses: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_stats_ses"),
                format!("{prefix}_connection_xdsl_stats_ses"),
                &["direction"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_stats_ses gauge"
            )),
            xdsl_stats_rxmt: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_stats_rxmt"),
                format!("{prefix}_connection_xdsl_stats_rxmt"),
                &["direction"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_stats_rxmt gauge"
            )),
            xdsl_stats_rxmt_corr: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_stats_rxmt_corr"),
                format!("{prefix}_connection_xdsl_stats_rxmt_corr"),
                &["direction"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_stats_rxmt_corr gauge"
            )),
            xdsl_stats_rxmt_uncorr: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_stats_rxmt_uncorr"),
                format!("{prefix}_connection_xdsl_stats_rxmt_uncorr"),
                &["direction"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_stats_rxmt_uncorr gauge"
            )),
            xdsl_stats_rtx_tx: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_stats_rtx_tx"),
                format!("{prefix}_connection_xdsl_stats_rtx_tx"),
                &["direction"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_stats_rtx_tx gauge"
            )),
            xdsl_stats_rtx_c: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_stats_rtx_c"),
                format!("{prefix}_connection_xdsl_stats_rtx_c"),
                &["direction"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_stats_rtx_c gauge"
            )),
            xdsl_stats_rtx_uc: register_int_gauge_vec!(
                format!("{prefix}_connection_xdsl_stats_rtx_uc"),
                format!("{prefix}_connection_xdsl_stats_rtx_uc"),
                &["direction"]
            )
            .expect(&format!(
                "cannot create {prefix}_connection_xdsl_stats_rtx_uc gauge"
            )),
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

    async fn set_connection_ftth_status(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching connection ftth");

        let body = self
            .get_managed_client()
            .await
            .unwrap()
            .get(format!("{}v4/connection/ftth", self.factory.api_url))
            .send()
            .await?
            .text()
            .await?;

        let res = match serde_json::from_str::<FreeboxResponse<ConnectionFtth>>(&body) {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        let ftth = match res.result {
            None => {
                return Err(Box::new(FreeboxResponseError::new(
                    "v4/connection/ftth response was empty".to_string(),
                )))
            }
            Some(r) => r,
        };

        self.sfp_has_power_report_metric
            .set(ftth.sfp_has_power_report.unwrap_or_default().into());
        self.sfp_has_signal_metric
            .set(ftth.sfp_has_signal.unwrap_or_default().into());
        self.sfp_model_metric
            .with_label_values(&[&ftth.sfp_model.clone().unwrap_or_default()])
            .set(ftth.sfp_model.is_some().into());
        self.sfp_vendor_metric
            .with_label_values(&[&ftth.sfp_vendor.clone().unwrap_or_default()])
            .set(ftth.sfp_vendor.is_some().into());
        self.sfp_pwr_tx_metric
            .set(ftth.sfp_pwr_tx.unwrap_or_default());
        self.sfp_pwr_rx_metric
            .set(ftth.sfp_pwr_rx.unwrap_or_default());
        self.link_metric.set(ftth.link.unwrap_or_default().into());
        self.sfp_alim_ok_metric
            .set(ftth.sfp_alim_ok.unwrap_or_default().into());
        self.sfp_serial_metric
            .with_label_values(&[&ftth.sfp_serial.clone().unwrap_or_default()])
            .set(ftth.sfp_serial.is_some().into());
        self.sfp_present_metric
            .set(ftth.sfp_present.unwrap_or_default().into());
        Ok(())
    }

    async fn get_connection_status(
        &mut self,
    ) -> Result<ConnectionStatus, Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching connection status");

        let client = self.get_managed_client().await?;
        let response = client
            .get(format!("{}v4/connection", self.factory.api_url))
            .send()
            .await?
            .json::<FreeboxResponse<ConnectionStatus>>()
            .await?;

        if response.success.unwrap_or(false) {
            if let Some(result) = response.result {
                return Ok(result);
            } else {
                return Err(Box::new(FreeboxResponseError::new(
                    "v4/connection response was empty".to_string(),
                )));
            }
        } else {
            return Err(Box::new(FreeboxResponseError::new(
                response.msg.unwrap_or_default(),
            )));
        }
    }

    async fn set_connection_status(
        &mut self,
        status: &ConnectionStatus,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.type_metric
            .with_label_values(&[&status.clone()._type.unwrap_or_default()])
            .set(1);
        self.state_metric.with_label_values(&["up"]).set(
            if status.clone().state.unwrap_or_default() == "up" {
                1
            } else {
                0
            },
        );
        self.media_metric
            .with_label_values(&[&status.clone().media.unwrap_or_default()])
            .set(1);
        self.ipv4_metric
            .with_label_values(&[&status.clone().ipv4.unwrap_or_default()])
            .set(1);
        self.ipv6_metric
            .with_label_values(&[&status.clone().ipv6.unwrap_or_default()])
            .set(1);
        self.bytes_down_metric
            .set(status.bytes_down.unwrap_or_default());
        self.bytes_up_metric
            .set(status.bytes_up.unwrap_or_default());
        self.rate_down_metric
            .set(status.rate_down.unwrap_or_default());
        self.rate_up_metric.set(status.rate_up.unwrap_or_default());
        self.bandwidth_down_metric
            .set(status.bandwidth_down.unwrap_or_default());
        self.bandwidth_up_metric
            .set(status.bandwidth_up.unwrap_or_default());

        Ok(())
    }

    async fn set_connection_conf(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching connection configuration");

        let body = self
            .get_managed_client()
            .await
            .unwrap()
            .get(format!("{}v4/connection/config", self.factory.api_url))
            .send()
            .await?
            .text()
            .await?;

        let res = match serde_json::from_str::<FreeboxResponse<ConnectionConfiguration>>(&body) {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        let conf = match res.result {
            None => {
                return Err(Box::new(FreeboxResponseError::new(
                    "v4/connection/config response was empty".to_string(),
                )))
            }
            Some(r) => r,
        };

        self.ping_metric.set(conf.ping.unwrap_or_default().into());
        self.is_secure_pass_metric
            .set(conf.is_secure_pass.unwrap_or_default().into());
        self.remote_access_port_metric
            .set(conf.remote_access_port.unwrap_or_else(|| 0).into());
        self.remote_access_metric
            .set(conf.remote_access.unwrap_or_default().into());
        self.wol_metric.set(conf.wol.unwrap_or_default().into());
        self.adblock_metric
            .set(conf.adblock.unwrap_or_default().into());
        self.adblock_not_set_metric
            .set(conf.adblock_not_set.unwrap_or_default().into());
        self.api_remote_access_metric
            .set(conf.api_remote_access.unwrap_or_default().into());
        self.allow_token_request_metric
            .set(conf.allow_token_request.unwrap_or_default().into());
        self.remote_access_ip_metric
            .with_label_values(&[&conf.remote_access_ip.unwrap_or_else(|| String::new())])
            .set(conf.remote_access.is_some().into());

        Ok(())
    }

    async fn set_connection_ipv6_conf(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching connection ipv6 configuration");

        let body = self
            .get_managed_client()
            .await
            .unwrap()
            .get(format!("{}v4/connection/ipv6/config", self.factory.api_url))
            .send()
            .await?
            .text()
            .await?;

        let res = match serde_json::from_str::<FreeboxResponse<ConnectionIpv6Configuration>>(&body)
        {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        let conf = match res.result {
            None => {
                return Err(Box::new(FreeboxResponseError::new(
                    "v4/connection/ipv6/config response was empty".to_string(),
                )))
            }
            Some(r) => r,
        };

        self.ipv6_enabled_metric
            .set(conf.ipv6_enabled.unwrap_or_default().into());

        if conf.delegations.is_some() {
            for delegation in conf.delegations.unwrap() {
                self.delegations_metric
                    .with_label_values(&[
                        &delegation.prefix.unwrap(),
                        &delegation.next_hop.unwrap(),
                    ])
                    .set(1);
            }
        }

        Ok(())
    }

    async fn get_xdsl_info(
        &mut self,
    ) -> Result<XdslInfo, Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching xdsl info");

        let client = self.get_managed_client().await?;

        let result = client
            .get(format!("{}v4/connection/xdsl", self.factory.api_url))
            .send()
            .await?
            .json::<FreeboxResponse<XdslInfo>>()
            .await?;

        result.result.ok_or_else(|| {
            Box::new(FreeboxResponseError::new(
                "v4/connection/xdsl/status response was empty".to_string(),
            )) as Box<dyn std::error::Error + Send + Sync>
        })
    }

    async fn set_xdsl_status(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching xdsl status");

        let info = self.get_xdsl_info().await?;
        let status = info.status.unwrap();

        self.xdsl_status_uptime
            .with_label_values(&[
                &status.status.unwrap_or_default(),
                &status.protocol.unwrap_or_default(),
                &status.modulation.unwrap_or_default(),
            ])
            .set(status.uptime.unwrap_or_default().into());

        let up = info.up.unwrap();
        let down = info.down.unwrap();

        struct DirectionStats {
            direction: String,
            stats: XdslStats, // Replace `XdslStatus` with the actual type of `up` and `down`
        }

        let arr = vec![
            DirectionStats {
                direction: "up".to_string(),
                stats: up,
            },
            DirectionStats {
                direction: "down".to_string(),
                stats: down,
            },
        ];

        for stats in arr {
            self.xdsl_stats_maxrate
                .with_label_values(&[&stats.direction])
                .set(stats.stats.maxrate.unwrap_or_default().into());
            self.xdsl_stats_rate
                .with_label_values(&[&stats.direction])
                .set(stats.stats.rate.unwrap_or_default().into());
            self.xdsl_stats_snr
                .with_label_values(&[&stats.direction])
                .set(stats.stats.snr.unwrap_or_default().into());
            self.xdsl_stats_attn
                .with_label_values(&[&stats.direction])
                .set(stats.stats.attn.unwrap_or_default().into());
            self.xdsl_stats_fec
                .with_label_values(&[&stats.direction])
                .set(stats.stats.fec.unwrap_or_default().into());
            self.xdsl_stats_crc
                .with_label_values(&[&stats.direction])
                .set(stats.stats.crc.unwrap_or_default().into());
            self.xdsl_stats_hec
                .with_label_values(&[&stats.direction])
                .set(stats.stats.hec.unwrap_or_default().into());
            self.xdsl_stats_es
                .with_label_values(&[&stats.direction])
                .set(stats.stats.es.unwrap_or_default().into());
            self.xdsl_stats_ses
                .with_label_values(&[&stats.direction])
                .set(stats.stats.ses.unwrap_or_default().into());
            self.xdsl_stats_rxmt
                .with_label_values(&[&stats.direction])
                .set(stats.stats.rxmt.unwrap_or_default().into());
            self.xdsl_stats_rxmt_corr
                .with_label_values(&[&stats.direction])
                .set(stats.stats.rxmt_corr.unwrap_or_default().into());
            self.xdsl_stats_rxmt_uncorr
                .with_label_values(&[&stats.direction])
                .set(stats.stats.rxmt_uncorr.unwrap_or_default().into());
            self.xdsl_stats_rtx_tx
                .with_label_values(&[&stats.direction])
                .set(stats.stats.rtx_tx.unwrap_or_default().into());
            self.xdsl_stats_rtx_c
                .with_label_values(&[&stats.direction])
                .set(stats.stats.rtx_c.unwrap_or_default().into());
            self.xdsl_stats_rtx_uc
                .with_label_values(&[&stats.direction])
                .set(stats.stats.rtx_uc.unwrap_or_default().into());
        }

        Ok(())
    }

    fn reset_all(&mut self) {
        self.bytes_down_metric.set(0);
        self.bytes_up_metric.set(0);
        self.rate_down_metric.set(0);
        self.rate_up_metric.set(0);
        self.bandwidth_down_metric.set(0);
        self.bandwidth_up_metric.set(0);
        self.type_metric.reset();
        self.media_metric.reset();
        self.state_metric.reset();
        self.ipv4_metric.reset();
        self.ipv6_metric.reset();
        self.ping_metric.set(0);
        self.is_secure_pass_metric.set(0);
        self.remote_access_port_metric.set(0);
        self.remote_access_metric.set(0);
        self.wol_metric.set(0);
        self.adblock_metric.set(0);
        self.adblock_not_set_metric.set(0);
        self.api_remote_access_metric.set(0);
        self.allow_token_request_metric.set(0);
        self.remote_access_ip_metric.reset();
        self.ipv6_enabled_metric.set(0);
        self.delegations_metric.reset();
        self.sfp_has_power_report_metric.set(0);
        self.sfp_has_signal_metric.set(0);
        self.sfp_model_metric.reset();
        self.sfp_vendor_metric.reset();
        self.sfp_pwr_tx_metric.set(0);
        self.sfp_pwr_rx_metric.set(0);
        self.link_metric.set(0);
        self.sfp_alim_ok_metric.set(0);
        self.sfp_serial_metric.reset();
        self.sfp_present_metric.set(0);
        self.ipv6_enabled_metric.set(0);
        self.delegations_metric.reset();
        self.sfp_has_power_report_metric.set(0);
        self.sfp_has_signal_metric.set(0);
        self.sfp_model_metric.reset();
        self.sfp_vendor_metric.reset();
        self.sfp_pwr_tx_metric.set(0);
        self.sfp_pwr_rx_metric.set(0);
        self.xdsl_status_uptime.reset();
        self.xdsl_stats_maxrate.reset();
        self.xdsl_stats_rate.reset();
        self.xdsl_stats_snr.reset();
        self.xdsl_stats_attn.reset();
        self.xdsl_stats_fec.reset();
        self.xdsl_stats_crc.reset();
        self.xdsl_stats_hec.reset();
        self.xdsl_stats_es.reset();
        self.xdsl_stats_ses.reset();
        self.xdsl_stats_rxmt.reset();
        self.xdsl_stats_rxmt_corr.reset();
        self.xdsl_stats_rxmt_uncorr.reset();
        self.xdsl_stats_rtx_tx.reset();
        self.xdsl_stats_rtx_c.reset();
        self.xdsl_stats_rtx_uc.reset();
    }
}

#[async_trait]
impl<'a> MetricMap<'a> for ConnectionMetricMap<'a> {
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let status = self.get_connection_status().await?;
        let media = status.media.unwrap_or_default();

        info!("exposing network media metrics: {}", media);
        self.is_ftth = Some(media.trim().to_lowercase() == "ftth".to_string());
        Ok(())
    }

    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.reset_all();

        let status = self.get_connection_status().await?;
        self.set_connection_status(&status).await?;

        self.set_connection_conf().await?;
        self.set_connection_ipv6_conf().await?;

        let media = status.media.unwrap_or("unknown".to_string()).to_lowercase();
        let is_ftth = media == "ftth";

        if is_ftth != self.is_ftth.unwrap_or(true) {
            info!("network media has changed, now exposing metrics: {}", media);
            self.is_ftth = Some(is_ftth);
        }

        if is_ftth {
            self.set_connection_ftth_status().await?;
        } else {
            self.set_xdsl_status().await?;
        }

        Ok(())
    }
}

#[async_trait]
impl DryRunnable for ConnectionMetricMap<'_> {
    fn get_name(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        Ok("connection".to_string())
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
