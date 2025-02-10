use async_trait::async_trait;
use chrono::Duration;
use connection::ConnectionMetricMap;
use lan::LanMetricMap;
use lanbrowser::LanBrowserMetricMap;
use log::{error, warn};
use switch::SwitchMetricMap;
use system::SystemMetricMap;

use crate::core::{
    common::http_client_factory::AuthenticatedHttpClientFactory,
    configuration::sections::{ApiConfiguration, MetricsConfiguration},
};

pub mod connection;
pub mod dhcp;
pub mod lan;
pub mod lanbrowser;
pub mod switch;
pub mod system;
pub mod wifi;

#[async_trait]
pub trait MetricMap<'a> {
    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub struct Mapper<'a> {
    maps: Vec<Box<dyn MetricMap<'a> + 'a>>,
}

impl<'a> Mapper<'a> {
    pub fn new(
        factory: &'a AuthenticatedHttpClientFactory<'a>,
        conf: MetricsConfiguration,
        api_conf: ApiConfiguration,
    ) -> Self {
        let mut maps: Vec<Box<dyn MetricMap<'a> + 'a>> = vec![];

        if let Some(e) = conf.connection {
            if e {
                maps.push(Box::new(ConnectionMetricMap::new(
                    &factory,
                    conf.prefix.to_owned().unwrap(),
                )));
            }
        } else {
            warn!(
                "Connection metrics are disabled by default, missing entry in the configuration file"
            );
        }
        if let Some(e) = conf.system {
            if e {
                maps.push(Box::new(SystemMetricMap::new(
                    &factory,
                    conf.prefix.to_owned().unwrap(),
                )));
            }
        } else {
            warn!(
                "System metrics are disabled by default, missing entry in the configuration file"
            );
        }

        if let Some(e) = conf.lan {
            if e {
                maps.push(Box::new(LanMetricMap::new(
                    &factory,
                    conf.prefix.to_owned().unwrap(),
                )));
            }
        } else {
            warn!("LAN metrics are disabled by default, missing entry in the configuration file");
        }

        if let Some(e) = conf.lan_browser {
            if e {
                let mode = api_conf.mode.unwrap_or_default();
                if mode == "router" {
                    let lan_browser_map =
                        LanBrowserMetricMap::new(&factory, conf.prefix.to_owned().unwrap());
                    maps.push(Box::new(lan_browser_map));
                } else {
                    warn!("lan_browser is incompatible with this freebox mode ({}), the option has been disabled", mode);
                }
            }
        } else {
            warn!("LAN browser metrics are disabled by default, missing entry in the configuration file");
        }

        if let Some(e) = conf.switch {
            if e {
                maps.push(Box::new(SwitchMetricMap::new(
                    &factory,
                    conf.prefix.to_owned().unwrap(),
                )));
            }
        } else {
            warn!(
                "Switch metrics are disabled by default, missing entry in the configuration file"
            );
        }

        if let Some(e) = conf.wifi {
            if e {
                let wifi_map = wifi::WifiMetricMap::new(
                    &factory,
                    conf.prefix.to_owned().unwrap(),
                    Duration::seconds(api_conf.refresh.unwrap_or(5) as i64),
                );
                maps.push(Box::new(wifi_map));
            }
        } else {
            warn!("Wifi metrics are disabled by default, missing entry in the configuration file");
        }

        if let Some(e) = conf.dhcp {
            if e {
                maps.push(Box::new(dhcp::DhcpMetricMap::new(
                    &factory,
                    conf.prefix.to_owned().unwrap(),
                )));
            }
        } else {
            warn!("DHCP metrics are disabled by default, missing entry in the configuration file");
        }

        Self { maps }
    }

    pub async fn init_all(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for map in self.maps.iter_mut() {
            let res = map.init().await;
            match res {
                Err(e) => {
                    error!("{}", e);
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn set_all(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for map in self.maps.iter_mut() {
            let res = map.set().await;

            match res {
                Err(e) => {
                    error!("{}", e);
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod api_specs_provider {
    /// Get the API specs data from the file system
    pub async fn get_specs_data(
        api: &'static str,
        endpoint: &'static str,
    ) -> Result<String, std::io::Error> {
        let path = format!("src/mappers/specs-data/{}/{}.json", api, endpoint);
        tokio::fs::read_to_string(path).await
    }
}
