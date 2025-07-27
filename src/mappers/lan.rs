use async_trait::async_trait;
use log::debug;
use prometheus_exporter::prometheus::{register_int_gauge_vec, IntGaugeVec};
use reqwest::Client;
use serde::Deserialize;
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

#[derive(Deserialize, Clone, Debug)]
pub struct LanConfig {
    pub name_dns: Option<String>,
    pub name_mdns: Option<String>,
    pub name: Option<String>,
    pub mode: Option<String>,
    pub name_netbios: Option<String>,
    pub ip: Option<String>,
}

pub struct LanMetricMap<'a> {
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    managed_client: Option<ManagedHttpClient>,
    name_dns_metric: IntGaugeVec,
    name_mdns_metric: IntGaugeVec,
    name_metric: IntGaugeVec,
    mode_metric: IntGaugeVec,
    name_netbios_metric: IntGaugeVec,
    ip_metric: IntGaugeVec,
}

impl<'a> LanMetricMap<'a> {
    pub fn new(factory: &'a AuthenticatedHttpClientFactory<'a>, prefix: String) -> Self {
        let prfx = format!("{prefix}_lan_config");
        Self {
            factory,
            managed_client: None,
            name_dns_metric: register_int_gauge_vec!(
                format!("{prfx}_name_dns"),
                format!("{prfx}_name_dns"),
                &["name_dns"]
            )
            .expect(&format!("cannot create {prfx}_name_dns gauge")),
            name_mdns_metric: register_int_gauge_vec!(
                format!("{prfx}_name_mdns"),
                format!("{prfx}_name_mdns"),
                &["name_mdns"]
            )
            .expect(&format!("cannot create {prfx}_name_mdns gauge")),
            name_metric: register_int_gauge_vec!(
                format!("{prfx}_name"),
                format!("{prfx}_name"),
                &["name"]
            )
            .expect(&format!("cannot create {prfx}_name gauge")),
            mode_metric: register_int_gauge_vec!(
                format!("{prfx}_mode"),
                format!("{prfx}_mode"),
                &["mode"]
            )
            .expect(&format!("cannot create {prfx}_mode gauge")),
            name_netbios_metric: register_int_gauge_vec!(
                format!("{prfx}_name_netbios"),
                format!("{prfx}_name_netbios"),
                &["name_netbios"]
            )
            .expect(&format!("cannot create {prfx}_name_netbios gauge")),
            ip_metric: register_int_gauge_vec!(format!("{prfx}_ip"), format!("{prfx}_ip"), &["ip"])
                .expect(&format!("cannot create {prfx}_ip gauge")),
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

    async fn set_lan_config(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching lan config");

        let body = self
            .get_managed_client()
            .await
            .unwrap()
            .get(format!("{}v4/lan/config", self.factory.api_url))
            .send()
            .await?
            .text()
            .await?;

        let res = match serde_json::from_str::<FreeboxResponse<LanConfig>>(&body) {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        let cfg: LanConfig = match res.result {
            None => {
                return Err(Box::new(FreeboxResponseError::new(
                    "v4/lan/config response was empty".to_string(),
                )))
            }
            Some(r) => r,
        };

        self.name_dns_metric
            .with_label_values(&[&cfg.name_dns.clone().unwrap_or_default()])
            .set(cfg.name_dns.is_some().into());
        self.name_mdns_metric
            .with_label_values(&[&cfg.name_mdns.clone().unwrap_or_default()])
            .set(cfg.name_mdns.is_some().into());
        self.name_metric
            .with_label_values(&[&cfg.name.clone().unwrap_or_default()])
            .set(cfg.name.is_some().into());
        self.name_netbios_metric
            .with_label_values(&[&cfg.name_netbios.clone().unwrap_or_default()])
            .set(cfg.name_netbios.is_some().into());
        self.mode_metric
            .with_label_values(&[&cfg.mode.clone().unwrap_or_default()])
            .set(cfg.mode.is_some().into());
        self.ip_metric
            .with_label_values(&[&cfg.ip.clone().unwrap_or_default()])
            .set(cfg.ip.is_some().into());

        Ok(())
    }

    fn reset_all(&mut self) {
        self.name_dns_metric.reset();
        self.name_mdns_metric.reset();
        self.name_metric.reset();
        self.mode_metric.reset();
        self.name_netbios_metric.reset();
        self.ip_metric.reset();
    }
}

#[async_trait]
impl<'a> MetricMap<'a> for LanMetricMap<'a> {
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.reset_all();

        if let Err(e) = self.set_lan_config().await {
            return Err(e);
        };
        Ok(())
    }
}

#[async_trait]
impl DryRunnable for LanMetricMap<'_> {
    fn get_name(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        Ok("lan".to_string())
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
