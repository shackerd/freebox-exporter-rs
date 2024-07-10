use async_trait::async_trait;
use prometheus_exporter::prometheus::{core::{AtomicF64, GenericGauge}, register_gauge};
use serde::Deserialize;

use crate::core::common::{AuthenticatedHttpClientFactory, FreeboxResponse};

use super::TranslatorMetricTap;

#[derive(Deserialize, Debug)]
pub struct Connection {
    #[serde(alias="type")]
    pub _type: String,
    pub rate_down: u64,
    pub bytes_up: u64,
    pub rate_up: u64,
    pub bandwidth_up: u64,
    pub ipv4: String,
    pub ipv6: String,
    pub bandwidth_down: u64,
    pub state: String,
    pub bytes_down: u64,
    pub media: String
}

pub struct ConnectionTap {
    factory: AuthenticatedHttpClientFactory,
    bytes_down_metric: GenericGauge<AtomicF64>
}

impl ConnectionTap {
    pub fn new(factory: AuthenticatedHttpClientFactory) -> Self {
        Self {
            factory,
            bytes_down_metric : register_gauge!("bytes_down", "bytes_down").expect("cannot create test gauge")
        }
    }
}

#[async_trait]
impl TranslatorMetricTap for ConnectionTap {

    async fn set(&self) -> Result<(), Box<dyn std::error::Error>> {
        let body =
            self.factory.create_client().unwrap().get(format!("{}v4/connection", self.factory.api_url))
            .send().await.unwrap()
            .text().await.unwrap();

        let res = serde_json::from_str::<FreeboxResponse<Connection>>(&body);

        let connection = res.expect("Cannot read response").result;

        self.bytes_down_metric.set(connection.bytes_down as f64);

        Ok(())
    }
}