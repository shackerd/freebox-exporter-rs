use async_trait::async_trait;
use connection::ConnectionTap;
use system::SystemTap;

use crate::core::{common::AuthenticatedHttpClientFactory, configuration::MetricsConfiguration};

pub mod connection;
pub mod system;

#[async_trait]
pub trait TranslatorMetricTap {
    async fn set(&self) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct Translator {
    taps: Vec<Box<dyn TranslatorMetricTap>>
}

impl Translator {
    pub fn new(factory: AuthenticatedHttpClientFactory, conf: MetricsConfiguration) -> Self {

        let mut taps: Vec<Box<dyn TranslatorMetricTap>> = vec![];

        if conf.connection.unwrap() {
            taps.push(Box::new(ConnectionTap::new(factory.clone(), conf.prefix.clone().unwrap())));
        }
        if conf.settings.unwrap() {
            taps.push(Box::new(SystemTap::new(factory.clone(), conf.prefix.clone().unwrap())));
        }
        Self { taps }
    }

    pub async fn set_all(&self) -> Result<(), Box<dyn std::error::Error>>
    {
        for tap in self.taps.iter() {
            tap.set().await.expect("Cannot set translator metric tap");
        }

        Ok(())
    }
}