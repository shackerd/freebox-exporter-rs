use async_trait::async_trait;
use prometheus_exporter::prometheus::{core::{AtomicI64, GenericGauge, MetricVec}, register_int_gauge, register_int_gauge_vec, GaugeVec, IntGauge, IntGaugeVec};
use serde::Deserialize;

use crate::core::common::{AuthenticatedHttpClientFactory, FreeboxResponse};

use super::TranslatorMetricTap;

#[derive(Deserialize, Debug)]
pub struct ConnectionStatus {
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

#[derive(Deserialize, Debug)]
pub struct ConnectionConfiguration {

}

pub struct ConnectionTap {
    factory: AuthenticatedHttpClientFactory,
    bytes_down_metric:  IntGauge,
    bytes_up_metric:  IntGauge,
    rate_down_metric:  IntGauge,
    rate_up_metric:  IntGauge,
    bandwidth_down_metric:  IntGauge,
    bandwidth_up_metric:  IntGauge,
    type_metric: IntGaugeVec,
    media_metric: IntGaugeVec,
    state_metric: IntGaugeVec,
    ipv4_metric: IntGaugeVec,
    ipv6_metric: IntGaugeVec,
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
            type_metric : register_int_gauge_vec!("connection_type", "connection_type", &["type"]).expect("cannot create connection_type gauge"),
            media_metric : register_int_gauge_vec!("connection_media", "connection_media", &["media"]).expect("cannot create connection_media gauge"),
            state_metric : register_int_gauge_vec!("connection_state", "connection_state", &["state"]).expect("cannot create connection_state gauge"),
            ipv4_metric : register_int_gauge_vec!("connection_ipv4", "connection_ipv4", &["ipv4"]).expect("cannot create connection_ipv4 gauge"),
            ipv6_metric : register_int_gauge_vec!("connection_ipv6", "connection_ipv6", &["ipv6"]).expect("cannot create connection_ipv6 gauge")
        }
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Box<dyn std::error::Error>> {

        let con_body =
            self.factory.create_client().unwrap().get(format!("{}v4/connection", self.factory.api_url))
            .send().await.unwrap()
            .text().await.unwrap();

        let con_res = serde_json::from_str::<FreeboxResponse<ConnectionStatus>>(&con_body);

        let con = con_res.expect("Cannot read response").result;

        Ok(con)
    }

    async fn get_connection_conf(&self) -> Result<ConnectionConfiguration, Box<dyn std::error::Error>> {
        todo!()
    }
}

#[async_trait]
impl TranslatorMetricTap for ConnectionTap {

    async fn set(&self) -> Result<(), Box<dyn std::error::Error>> {

        let status = self.get_connection_status().await?;

        self.type_metric.with_label_values(&[status._type.as_str()]).set(1);
        self.state_metric.with_label_values(&["up"]).set(if status.state == "up" { 1 } else { 0 } );
        self.media_metric.with_label_values(&[status.media.as_str()]).set(1);
        self.ipv4_metric.with_label_values(&[status.ipv4.as_str()]).set(1);
        self.ipv6_metric.with_label_values(&[status.ipv6.as_str()]).set(1);
        self.bytes_down_metric.set(status.bytes_down as i64);
        self.bytes_up_metric.set(status.bytes_up as i64);
        self.rate_down_metric.set(status.rate_down as i64);
        self.rate_up_metric.set(status.rate_up as i64);
        self.bandwidth_down_metric.set(status.bandwidth_down as i64);
        self.bandwidth_up_metric.set(status.bandwidth_up as i64);

        Ok(())
    }
}