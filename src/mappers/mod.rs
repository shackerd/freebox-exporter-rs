use async_trait::async_trait;
use chrono::Duration;
use connection::ConnectionMetricMap;
use lan::LanMetricMap;
use lanbrowser::LanBrowserMetricMap;
use log::{error, warn};
use switch::SwitchMetricMap;
use system::SystemMetricMap;

use crate::core::{
    common::AuthenticatedHttpClientFactory,
    configuration::{ApiConfiguration, MetricsConfiguration},
};

pub mod connection;
pub mod lan;
pub mod lanbrowser;
pub mod switch;
pub mod system;
pub mod wifi;

#[async_trait]
pub trait MetricMap {
    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct Mapper {
    maps: Vec<Box<dyn MetricMap>>,
}

impl Mapper {
    pub fn new(
        factory: AuthenticatedHttpClientFactory,
        conf: MetricsConfiguration,
        api_conf: ApiConfiguration,
    ) -> Self {
        let mut maps: Vec<Box<dyn MetricMap>> = vec![];

        if conf.connection.unwrap() {
            maps.push(Box::new(ConnectionMetricMap::new(
                factory.to_owned(),
                conf.prefix.to_owned().unwrap(),
            )));
        }
        if conf.system.unwrap() {
            maps.push(Box::new(SystemMetricMap::new(
                factory.to_owned(),
                conf.prefix.to_owned().unwrap(),
            )));
        }

        if conf.lan.unwrap() {
            maps.push(Box::new(LanMetricMap::new(
                factory.to_owned(),
                conf.prefix.to_owned().unwrap(),
            )));
        }

        if conf.lan_browser.unwrap() {
            let mode = api_conf.mode.unwrap_or_default();
            if mode == "router" {
                let lan_browser_map =
                    LanBrowserMetricMap::new(factory.to_owned(), conf.prefix.to_owned().unwrap());
                maps.push(Box::new(lan_browser_map));
            } else {
                warn!("lan_browser is incompatible with this freebox mode ({}), the option has been disabled", mode);
            }
        }

        if conf.switch.unwrap() {
            maps.push(Box::new(SwitchMetricMap::new(
                factory.to_owned(),
                conf.prefix.to_owned().unwrap(),
            )));
        }

        if conf.wifi.unwrap() {
            let ttl = Duration::seconds(api_conf.refresh.unwrap_or(5) as i64);

            maps.push(Box::new(wifi::WifiMetricMap::new(
                factory.to_owned(),
                conf.prefix.to_owned().unwrap(),
                ttl,
            )));
        }
        Self { maps }
    }

    pub async fn init_all(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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

    pub async fn set_all(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
