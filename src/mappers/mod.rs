use async_trait::async_trait;
use connection::ConnectionMetricMap;
use system::SystemMetricMap;
use lanbrowser::LanBrowserMetricMap;
use lan::LanMetricMap;
use switch::SwitchMetricMap;
use log::{error, warn};

use crate::core::{common::AuthenticatedHttpClientFactory, configuration::{ApiConfiguration, MetricsConfiguration}};

pub mod connection;
pub mod system;
pub mod lan;
pub mod lanbrowser;
pub mod switch;

#[async_trait]
pub trait MetricMap {
    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct Mapper {
    maps: Vec<Box<dyn MetricMap>>
}

impl Mapper {
    pub fn new(factory: AuthenticatedHttpClientFactory, conf: MetricsConfiguration, api_conf: ApiConfiguration) -> Self {

        let mut maps: Vec<Box<dyn MetricMap>> = vec![];

        if conf.connection.unwrap() {
            maps.push(Box::new(ConnectionMetricMap::new(factory.to_owned(), conf.prefix.to_owned().unwrap())));
        }
        if conf.system.unwrap() {
            maps.push(Box::new(SystemMetricMap::new(factory.to_owned(), conf.prefix.to_owned().unwrap())));
        }

        if conf.lan.unwrap() {
            maps.push(Box::new(LanMetricMap::new(factory.to_owned(), conf.prefix.to_owned().unwrap())));
        }

        if conf.lan_browser.unwrap() {

            let mode = api_conf.mode.unwrap_or_default();
            if mode == "router" {
                let lan_browser_map = LanBrowserMetricMap::new(factory.to_owned(), conf.prefix.to_owned().unwrap());
                maps.push(Box::new(lan_browser_map));
            }
            else {
                warn!("lan_browser is incompatible with this freebox mode ({}), the option has been disabled", mode);
            }
        }

        if conf.switch.unwrap() {
            maps.push(Box::new(SwitchMetricMap::new(factory.to_owned(), conf.prefix.to_owned().unwrap())));
        }


        Self { maps }
    }

    pub async fn init_all(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for map in self.maps.iter_mut() {
            let res = map.init().await;
            match res {
                Err(e) => { error!("{}", e); },
                _ => { }
            }
        }
        Ok(())
    }

    pub async fn set_all(&mut self) -> Result<(), Box<dyn std::error::Error>>
    {
        for map in self.maps.iter_mut() {
            let res = map.set().await;

            match res {
                Err(e) => { error!("{}", e); },
                _ => { }
            }
        }

        Ok(())
    }
}
