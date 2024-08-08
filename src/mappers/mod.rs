use async_trait::async_trait;
use connection::ConnectionMetricMap;
use log::error;
use system::SystemMetricMap;

use crate::core::{common::AuthenticatedHttpClientFactory, configuration::MetricsConfiguration};

pub mod connection;
pub mod system;

#[async_trait]
pub trait MetricMap {
    async fn set(&self) -> Result<(), Box<dyn std::error::Error>>;
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
        if conf.settings.unwrap() {
            maps.push(Box::new(SystemMetricMap::new(factory.clone(), conf.prefix.clone().unwrap())));
        }
        Self { maps }
    }

    pub async fn set_all(&self) -> Result<(), Box<dyn std::error::Error>>
    {
        for map in self.maps.iter() {
            let res = map.set().await;

            match res {
                Err(e) => { error!("{}", e); },
                _ => { }
            }
        }

        Ok(())
    }
}
