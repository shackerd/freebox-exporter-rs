use async_trait::async_trait;
use log::debug;
use prometheus_exporter::prometheus::{register_int_gauge_vec, IntGaugeVec};
use serde::Deserialize;

use crate::core::common::{AuthenticatedHttpClientFactory, FreeboxResponse, FreeboxResponseError};

use super::MetricMap;

#[derive(Deserialize, Clone, Debug)]
pub struct LanConfig {
    name_dns: Option<String>,
    name_mdns: Option<String>,
    name: Option<String>,
    mode: Option<String>,
    name_netbios: Option<String>,
    ip: Option<String>
}

pub struct LanMetricMap {
    factory: AuthenticatedHttpClientFactory,
    name_dns_metric: IntGaugeVec,
    name_mdns_metric: IntGaugeVec,
    name_metric: IntGaugeVec,
    mode_metric: IntGaugeVec,
    name_netbios_metric: IntGaugeVec,
    ip_metric: IntGaugeVec
}

impl LanMetricMap {
    pub fn new(factory: AuthenticatedHttpClientFactory, prefix: String) -> Self {
        let prfx = format!("{prefix}_lan_config");
        Self {
            factory,
            name_dns_metric: register_int_gauge_vec!(format!("{prfx}_name_dns"), format!("{prfx}_name_dns"), &["name_dns"]).expect(&format!("cannot create {prfx}_name_dns gauge")),
            name_mdns_metric: register_int_gauge_vec!(format!("{prfx}_name_mdns"), format!("{prfx}_name_mdns"), &["name_mdns"]).expect(&format!("cannot create {prfx}_name_mdns gauge")),
            name_metric: register_int_gauge_vec!(format!("{prfx}_name"), format!("{prfx}_name"), &["name"]).expect(&format!("cannot create {prfx}_name gauge")),
            mode_metric: register_int_gauge_vec!(format!("{prfx}_mode"), format!("{prfx}_mode"), &["mode"]).expect(&format!("cannot create {prfx}_mode gauge")),
            name_netbios_metric: register_int_gauge_vec!(format!("{prfx}_name_netbios"), format!("{prfx}_name_netbios"), &["name_netbios"]).expect(&format!("cannot create {prfx}_name_netbios gauge")),
            ip_metric: register_int_gauge_vec!(format!("{prfx}_ip"), format!("{prfx}_ip"), &["ip"]).expect(&format!("cannot create {prfx}_ip gauge"))
        }
    }

    async fn set_lan_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("fetching lan config");

        let body =
            self.factory.create_client().await.unwrap().get(format!("{}v4/lan/config", self.factory.api_url))
            .send().await?
            .text().await?;

        let res = serde_json::from_str::<FreeboxResponse<LanConfig>>(&body);

        if res.is_err() || !res.as_ref().unwrap().success {
            return Err(Box::new(FreeboxResponseError::new(res.as_ref().unwrap().msg.clone())));
        }

        let cfg: LanConfig = res.expect("Cannot read response").result;


        self.name_dns_metric.with_label_values(&[&cfg.name_dns.clone().unwrap_or_default()]).set(cfg.name_dns.is_some().into());
        self.name_mdns_metric.with_label_values(&[&cfg.name_mdns.clone().unwrap_or_default()]).set(cfg.name_mdns.is_some().into());
        self.name_metric.with_label_values(&[&cfg.name.clone().unwrap_or_default()]).set(cfg.name.is_some().into());
        self.name_netbios_metric.with_label_values(&[&cfg.name_netbios.clone().unwrap_or_default()]).set(cfg.name_netbios.is_some().into());
        self.mode_metric.with_label_values(&[&cfg.mode.clone().unwrap_or_default()]).set(cfg.mode.is_some().into());
        self.ip_metric.with_label_values(&[&cfg.ip.clone().unwrap_or_default()]).set(cfg.ip.is_some().into());

        Ok(())
    }
}

#[async_trait]
impl MetricMap for LanMetricMap {

    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }

    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.set_lan_config().await { Err(e) => return Err(e), _ => {} };
        Ok(())
    }
}
