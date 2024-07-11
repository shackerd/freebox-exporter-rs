use async_trait::async_trait;
use prometheus_exporter::prometheus::{core::GenericGauge, register_int_gauge};
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
    bytes_down_metric:  GenericGauge<prometheus_exporter::prometheus::core::AtomicI64>,
    bytes_up_metric:  GenericGauge<prometheus_exporter::prometheus::core::AtomicI64>,
    rate_down_metric:  GenericGauge<prometheus_exporter::prometheus::core::AtomicI64>,
    rate_up_metric:  GenericGauge<prometheus_exporter::prometheus::core::AtomicI64>,
    bandwidth_down_metric:  GenericGauge<prometheus_exporter::prometheus::core::AtomicI64>,
    bandwidth_up_metric:  GenericGauge<prometheus_exporter::prometheus::core::AtomicI64>

}

impl ConnectionTap {
    pub fn new(factory: AuthenticatedHttpClientFactory) -> Self {

        Self {
            factory,
            bytes_down_metric : register_int_gauge!("connection_bytes_down", "connection_bytes_down").expect("cannot create connection_bytes_down gauge"),
            bytes_up_metric : register_int_gauge!("connection_bytes_up", "connection_bytes_up").expect("cannot create connection_bytes_up gauge"),
            rate_down_metric : register_int_gauge!("connection_rate_down", "connection_rate_down").expect("cannot create connection_rate_down gauge"),
            rate_up_metric : register_int_gauge!("connection_rate_up", "connection_rate_up").expect("cannot create connection_rate_up gauge"),
            bandwidth_down_metric : register_int_gauge!("connection_bandwidth_down", "connection_bandwidth_down").expect("cannot create connection_bandwidth_down gauge"),
            bandwidth_up_metric : register_int_gauge!("connection_bandwidth_up", "connection_bandwidth_up").expect("cannot create connection_bandwidth_up gauge"),
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

        self.bytes_down_metric.set(connection.bytes_down as i64);
        self.bytes_up_metric.set(connection.bytes_up as i64);
        self.rate_down_metric.set(connection.rate_down as i64);
        self.rate_up_metric.set(connection.rate_up as i64);
        self.bandwidth_down_metric.set(connection.bandwidth_down as i64);
        self.bandwidth_up_metric.set(connection.bandwidth_up as i64);

        Ok(())
    }
}