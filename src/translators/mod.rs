use async_trait::async_trait;
use connection::ConnectionTap;

use crate::core::common::{AuthenticatedHttpClientFactory, Permissions};

pub mod connection;

#[async_trait]
pub trait TranslatorMetricTap {
    async fn set(&self) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct Translator {
    taps: Vec<Box<dyn TranslatorMetricTap>>
}

impl Translator {
    pub fn new(factory: AuthenticatedHttpClientFactory, permissions: Permissions) -> Self {

        let mut taps: Vec<Box<dyn TranslatorMetricTap>> = vec![];

        if permissions.connection {
            taps.push(Box::new(ConnectionTap::new(factory.clone())));
            // taps.push(Box::new(ConnectionTap::new(factory.clone())));
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