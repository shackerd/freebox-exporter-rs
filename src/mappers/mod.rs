use async_trait::async_trait;
use connection::ConnectionMetricMap;
use system::SystemMetricMap;
use lanbrowser::LanBrowserMetricMap;
use lan::LanMetricMap;
use log::error;

use crate::core::{common::AuthenticatedHttpClientFactory, configuration::MetricsConfiguration};

pub mod connection;
pub mod system;
pub mod lan;
pub mod lanbrowser;

#[async_trait]
pub trait MetricMap {
    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct Mapper {
    maps: Vec<Box<dyn MetricMap>>
}

impl Mapper {
    pub fn new(factory: AuthenticatedHttpClientFactory, conf: MetricsConfiguration) -> Self {

        let mut maps: Vec<Box<dyn MetricMap>> = vec![];

        if conf.connection.unwrap() {
            maps.push(Box::new(ConnectionMetricMap::new(factory.clone(), conf.prefix.clone().unwrap())));
        }
        if conf.system.unwrap() {
            maps.push(Box::new(SystemMetricMap::new(factory.clone(), conf.prefix.clone().unwrap())));
        }

        if conf.lan.unwrap() {
            maps.push(Box::new(LanMetricMap::new(factory.clone(), conf.prefix.clone().unwrap())));
        }

        if conf.lan_browser.unwrap() {
            let lan_browser_map = LanBrowserMetricMap::new(factory.clone(), conf.prefix.clone().unwrap());
            maps.push(Box::new(lan_browser_map));
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
