use std::collections::HashMap;

use async_trait::async_trait;
use log::debug;
use prometheus_exporter::prometheus::{
    register_int_counter_vec, register_int_gauge_vec, IntCounterVec, IntGaugeVec,
};
use reqwest::Client;
use serde::Deserialize;

use super::MetricMap;
use crate::core::common::{
    http_client_factory::{AuthenticatedHttpClientFactory, ManagedHttpClient},
    transport::{get_api_result, FreeboxResponseError},
};

/// VPN server and client APIs were added in API v2.0; v4 is the exporter baseline
pub const MIN_API_VERSION: u32 = 4;

#[derive(Deserialize, Clone, Debug)]
struct VpnServer {
    name: Option<String>,
    #[serde(rename = "type")]
    vpn_type: Option<String>,
    state: Option<String>,
    connection_count: Option<i64>,
    auth_connection_count: Option<i64>,
}

#[derive(Deserialize, Clone, Debug)]
struct VpnConnection {
    vpn: Option<String>,
    rx_bytes: Option<i64>,
    tx_bytes: Option<i64>,
}

#[derive(Deserialize, Clone, Debug)]
struct VpnClientStatus {
    enabled: Option<bool>,
    state: Option<String>,
    last_error: Option<String>,
    last_up: Option<i64>,
    stats: Option<VpnClientStats>,
}

#[derive(Deserialize, Clone, Debug)]
struct VpnClientStats {
    rate_up: Option<i64>,
    rate_down: Option<i64>,
    bytes_up: Option<i64>,
    bytes_down: Option<i64>,
}

pub struct VpnMetricMap<'a> {
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    managed_client: Option<ManagedHttpClient>,
    server_state_metric: IntGaugeVec,
    server_started_metric: IntGaugeVec,
    server_connections_metric: IntGaugeVec,
    server_authenticated_connections_metric: IntGaugeVec,
    connections_rx_bytes_metric: IntGaugeVec,
    connections_tx_bytes_metric: IntGaugeVec,
    client_enabled_metric: IntGaugeVec,
    client_up_metric: IntGaugeVec,
    client_state_metric: IntGaugeVec,
    client_last_error_metric: IntGaugeVec,
    client_rate_up_bytes_per_second_metric: IntGaugeVec,
    client_rate_down_bytes_per_second_metric: IntGaugeVec,
    client_bytes_up_metric: IntCounterVec,
    client_bytes_down_metric: IntCounterVec,
    client_last_up_timestamp_seconds_metric: IntGaugeVec,
}

impl<'a> VpnMetricMap<'a> {
    pub fn new(factory: &'a AuthenticatedHttpClientFactory<'a>, prefix: String) -> Self {
        Self {
            factory,
            managed_client: None,
            server_state_metric: register_int_gauge_vec!(
                format!("{prefix}_vpn_server_state"),
                "VPN server state, always 1",
                &["name", "type", "state"]
            )
            .expect(&format!("cannot create {prefix}_vpn_server_state gauge")),
            server_started_metric: register_int_gauge_vec!(
                format!("{prefix}_vpn_server_started"),
                "1 if the VPN server is started, 0 otherwise",
                &["name", "type"]
            )
            .expect(&format!("cannot create {prefix}_vpn_server_started gauge")),
            server_connections_metric: register_int_gauge_vec!(
                format!("{prefix}_vpn_server_connections"),
                "number of active VPN server connections",
                &["name", "type"]
            )
            .expect(&format!("cannot create {prefix}_vpn_server_connections gauge")),
            server_authenticated_connections_metric: register_int_gauge_vec!(
                format!("{prefix}_vpn_server_authenticated_connections"),
                "number of authenticated VPN server connections",
                &["name", "type"]
            )
            .expect(&format!("cannot create {prefix}_vpn_server_authenticated_connections gauge")),
            connections_rx_bytes_metric: register_int_gauge_vec!(
                format!("{prefix}_vpn_connections_rx_bytes"),
                "bytes received by the VPN connections currently open, drops when one closes",
                &["vpn"]
            )
            .expect(&format!("cannot create {prefix}_vpn_connections_rx_bytes gauge")),
            connections_tx_bytes_metric: register_int_gauge_vec!(
                format!("{prefix}_vpn_connections_tx_bytes"),
                "bytes sent by the VPN connections currently open, drops when one closes",
                &["vpn"]
            )
            .expect(&format!("cannot create {prefix}_vpn_connections_tx_bytes gauge")),
            client_enabled_metric: register_int_gauge_vec!(
                format!("{prefix}_vpn_client_enabled"),
                "1 if the VPN client is enabled, 0 otherwise",
                &[]
            )
            .expect(&format!("cannot create {prefix}_vpn_client_enabled gauge")),
            client_up_metric: register_int_gauge_vec!(
                format!("{prefix}_vpn_client_up"),
                "1 if the VPN client is up, 0 otherwise",
                &[]
            )
            .expect(&format!("cannot create {prefix}_vpn_client_up gauge")),
            client_state_metric: register_int_gauge_vec!(
                format!("{prefix}_vpn_client_state"),
                "VPN client state, always 1",
                &["state"]
            )
            .expect(&format!("cannot create {prefix}_vpn_client_state gauge")),
            client_last_error_metric: register_int_gauge_vec!(
                format!("{prefix}_vpn_client_last_error"),
                "VPN client last error, always 1",
                &["last_error"]
            )
            .expect(&format!("cannot create {prefix}_vpn_client_last_error gauge")),
            client_rate_up_bytes_per_second_metric: register_int_gauge_vec!(
                format!("{prefix}_vpn_client_rate_up_bytes_per_second"),
                "VPN client upload rate in bytes per second",
                &[]
            )
            .expect(&format!("cannot create {prefix}_vpn_client_rate_up_bytes_per_second gauge")),
            client_rate_down_bytes_per_second_metric: register_int_gauge_vec!(
                format!("{prefix}_vpn_client_rate_down_bytes_per_second"),
                "VPN client download rate in bytes per second",
                &[]
            )
            .expect(&format!("cannot create {prefix}_vpn_client_rate_down_bytes_per_second gauge")),
            client_bytes_up_metric: register_int_counter_vec!(
                format!("{prefix}_vpn_client_uploaded_bytes_total"),
                "total bytes uploaded by the VPN client, as reported by the box",
                &[]
            )
            .expect(&format!("cannot create {prefix}_vpn_client_uploaded_bytes_total counter")),
            client_bytes_down_metric: register_int_counter_vec!(
                format!("{prefix}_vpn_client_downloaded_bytes_total"),
                "total bytes downloaded by the VPN client, as reported by the box",
                &[]
            )
            .expect(&format!("cannot create {prefix}_vpn_client_downloaded_bytes_total counter")),
            client_last_up_timestamp_seconds_metric: register_int_gauge_vec!(
                format!("{prefix}_vpn_client_last_up_timestamp_seconds"),
                "timestamp of the last successful VPN client connection",
                &[]
            )
            .expect(&format!("cannot create {prefix}_vpn_client_last_up_timestamp_seconds gauge")),
        }
    }

    async fn get_managed_client(
        &mut self,
    ) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(Ok(client)) = self.managed_client.as_ref().map(|m| m.get()) {
            return Ok(client);
        }

        debug!("creating managed client");
        let managed_client = self.factory.create_managed_client().await?;
        let client = managed_client.get()?;
        self.managed_client = Some(managed_client);
        Ok(client)
    }

    fn reset_all(&self) {
        self.server_state_metric.reset();
        self.server_started_metric.reset();
        self.server_connections_metric.reset();
        self.server_authenticated_connections_metric.reset();
        self.connections_rx_bytes_metric.reset();
        self.connections_tx_bytes_metric.reset();
        self.client_enabled_metric.reset();
        self.client_up_metric.reset();
        self.client_state_metric.reset();
        self.client_last_error_metric.reset();
        self.client_rate_up_bytes_per_second_metric.reset();
        self.client_rate_down_bytes_per_second_metric.reset();
        self.client_last_up_timestamp_seconds_metric.reset();
    }

    fn set_server_metrics(&self, server: &VpnServer) {
        let name = server.name.as_deref().unwrap_or("unknown");
        let vpn_type = server.vpn_type.as_deref().unwrap_or("unknown");
        let labels = [name, vpn_type];
        if let Some(state) = server.state.as_deref() {
            self.server_state_metric
                .with_label_values(&[name, vpn_type, state])
                .set(1);
            self.server_started_metric
                .with_label_values(&labels)
                .set((state == "started") as i64);
        }
        if let Some(value) = server.connection_count {
            self.server_connections_metric
                .with_label_values(&labels)
                .set(value);
        }
        if let Some(value) = server.auth_connection_count {
            self.server_authenticated_connections_metric
                .with_label_values(&labels)
                .set(value);
        }
    }

    fn set_client_metrics(&self, client: &VpnClientStatus) {
        if let Some(enabled) = client.enabled {
            self.client_enabled_metric
                .with_label_values(&[])
                .set(enabled as i64);
        }
        if let Some(state) = client.state.as_deref() {
            self.client_up_metric
                .with_label_values(&[])
                .set((state == "up") as i64);
            self.client_state_metric
                .with_label_values(&[state])
                .set(1);
        }
        if let Some(error) = client.last_error.as_deref() {
            self.client_last_error_metric
                .with_label_values(&[error])
                .set(1);
        }
        // 0 when the client never connected
        if let Some(value) = client.last_up.filter(|v| *v != 0) {
            self.client_last_up_timestamp_seconds_metric
                .with_label_values(&[])
                .set(value);
        }
        if let Some(stats) = client.stats.as_ref() {
            if let Some(value) = stats.rate_up {
                self.client_rate_up_bytes_per_second_metric
                    .with_label_values(&[])
                    .set(value);
            }
            if let Some(value) = stats.rate_down {
                self.client_rate_down_bytes_per_second_metric
                    .with_label_values(&[])
                    .set(value);
            }
        }
    }

    async fn set_all(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_managed_client().await?;
        let api_url = self.factory.api_url.clone();
        let servers = get_api_result::<Vec<VpnServer>>(&client, &api_url, &format!("v{MIN_API_VERSION}/vpn/"))
            .await?
            .unwrap_or_default();
        let connections = get_api_result::<Vec<VpnConnection>>(
            &client,
            &api_url,
            &format!("v{MIN_API_VERSION}/vpn/connection/"),
        )
            .await?
            .unwrap_or_default();
        let status = get_api_result::<VpnClientStatus>(
            &client,
            &api_url,
            &format!("v{MIN_API_VERSION}/vpn_client/status"),
        )
        .await?
        // unlike collections, the client status is always an object, even when disabled
        .ok_or_else(|| {
            Box::new(FreeboxResponseError::new("vpn_client/status response has no result".to_string()))
        })?;

        let mut rx = HashMap::<&str, i64>::new();
        let mut tx = HashMap::<&str, i64>::new();
        for connection in &connections {
            if let Some(value) = connection.rx_bytes {
                *rx.entry(connection.vpn.as_deref().unwrap_or("unknown")).or_default() += value;
            }
            if let Some(value) = connection.tx_bytes {
                *tx.entry(connection.vpn.as_deref().unwrap_or("unknown")).or_default() += value;
            }
        }

        // Keep the previous snapshot until all API calls have succeeded.
        self.reset_all();
        for server in &servers {
            self.set_server_metrics(server);
        }
        for (vpn, value) in rx {
            self.connections_rx_bytes_metric
                .with_label_values(&[vpn])
                .set(value);
        }
        for (vpn, value) in tx {
            self.connections_tx_bytes_metric
                .with_label_values(&[vpn])
                .set(value);
        }
        self.set_client_metrics(&status);
        let stats = status.stats.as_ref();
        set_counter(&self.client_bytes_up_metric, stats.and_then(|s| s.bytes_up));
        set_counter(&self.client_bytes_down_metric, stats.and_then(|s| s.bytes_down));
        Ok(())
    }
}

/// How to move a counter from `current` to the box `value`: (reset first, then increment by).
/// A value lower than the previous one means the box restarted counting
fn counter_step(current: u64, value: u64) -> (bool, u64) {
    if value >= current {
        (false, value - current)
    } else {
        (true, value)
    }
}

/// Exposes a cumulative value of the box as a counter. Counters are left out of reset_all
/// and only reset when the box value decreases, so a scrape never reads a spurious reset
fn set_counter(vec: &IntCounterVec, value: Option<i64>) {
    match value.and_then(|v| u64::try_from(v).ok()) {
        Some(value) => {
            let counter = vec.with_label_values(&[]);
            let (reset, increment) = counter_step(counter.get(), value);
            if reset {
                counter.reset();
            }
            counter.inc_by(increment);
        }
        None => {
            let _ = vec.remove_label_values(&[]);
        }
    }
}

#[async_trait]
impl<'a> MetricMap<'a> for VpnMetricMap<'a> {
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.set_all().await
    }
}

#[cfg(test)]
mod tests {
    use prometheus_exporter::prometheus::{core::Collector, proto::MetricType};
    use serde_json::{json, Value};

    use super::*;
    use crate::mappers::{
        api_specs_provider::get_specs_data,
        test_support::{mock_api, serve},
    };

    /// Value of the first gauge or counter series matching all `labels`, None when it does not exist
    fn exposed(vec: &dyn Collector, labels: &[(&str, &str)]) -> Option<f64> {
        vec.collect()
            .iter()
            .flat_map(|family| family.get_metric().iter().map(move |m| (family.get_field_type(), m)))
            .find(|(_, m)| {
                labels.iter().all(|(k, v)| {
                    m.get_label()
                        .iter()
                        .any(|l| l.get_name() == *k && l.get_value() == *v)
                })
            })
            .map(|(kind, m)| match kind {
                MetricType::COUNTER => m.get_counter().get_value(),
                _ => m.get_gauge().get_value(),
            })
    }

    fn series_count(vec: &IntGaugeVec) -> usize {
        vec.collect().iter().map(|family| family.get_metric().len()).sum()
    }

    fn assert_client_optional_absent(map: &VpnMetricMap<'_>) {
        let metrics: [&dyn Collector; 8] = [
            &map.client_up_metric,
            &map.client_state_metric,
            &map.client_last_error_metric,
            &map.client_rate_up_bytes_per_second_metric,
            &map.client_rate_down_bytes_per_second_metric,
            &map.client_bytes_up_metric,
            &map.client_bytes_down_metric,
            &map.client_last_up_timestamp_seconds_metric,
        ];
        for metric in metrics {
            assert_eq!(None, exposed(metric, &[]));
        }
    }

    async fn fixture(endpoint: &'static str) -> Value {
        serde_json::from_str(&get_specs_data("vpn", endpoint).await.unwrap()).unwrap()
    }

    async fn serve_cycle(server: &wiremock::MockServer, servers: Value, connections: Value, client: Value) {
        serve(server, "/api/v4/vpn/", servers).await;
        serve(server, "/api/v4/vpn/connection/", connections).await;
        serve(server, "/api/v4/vpn_client/status", client).await;
    }

    #[tokio::test]
    async fn set_exposes_real_box_servers_and_disabled_client() {
        let (server, authenticator) = mock_api().await;
        serve_cycle(
            &server,
            fixture("api_v4_vpn-get").await,
            fixture("api_v4_vpn_connection_empty-get").await,
            fixture("api_v4_vpn_client_status_disabled-get").await,
        ).await;
        let factory = authenticator.login().await.unwrap();
        let mut map = VpnMetricMap::new(&factory, "test_vpn_real".to_string());
        map.set().await.unwrap();

        for metric in [
            &map.server_state_metric,
            &map.server_started_metric,
            &map.server_connections_metric,
            &map.server_authenticated_connections_metric,
        ] {
            assert_eq!(5, series_count(metric));
        }
        for (name, vpn_type) in [
            ("pptp", "pptp"),
            ("openvpn_routed", "openvpn"),
            ("ipsec", "ipsec"),
            ("openvpn_bridge", "openvpn"),
            ("wireguard", "wireguard"),
        ] {
            let labels = [("name", name), ("type", vpn_type)];
            assert_eq!(Some(0.0), exposed(&map.server_started_metric, &labels));
            assert_eq!(Some(0.0), exposed(&map.server_connections_metric, &labels));
            assert_eq!(Some(0.0), exposed(&map.server_authenticated_connections_metric, &labels));
            assert_eq!(Some(1.0), exposed(&map.server_state_metric,
                &[("name", name), ("type", vpn_type), ("state", "stopped")]));
        }
        assert_eq!(Some(0.0), exposed(&map.client_enabled_metric, &[]));
        assert_client_optional_absent(&map);
        assert_eq!(None, exposed(&map.connections_rx_bytes_metric, &[]));
        assert_eq!(None, exposed(&map.connections_tx_bytes_metric, &[]));
    }

    #[tokio::test]
    async fn set_exposes_doc_examples() {
        let (server, authenticator) = mock_api().await;
        serve_cycle(
            &server,
            fixture("api_v4_vpn-get").await,
            fixture("api_v4_vpn_connection-get").await,
            fixture("api_v4_vpn_client_status-get").await,
        ).await;
        let factory = authenticator.login().await.unwrap();
        let mut map = VpnMetricMap::new(&factory, "test_vpn_doc".to_string());
        map.set().await.unwrap();

        for metric in [&map.connections_rx_bytes_metric, &map.connections_tx_bytes_metric] {
            assert_eq!(1, series_count(metric));
            assert_eq!(Some(94.0), exposed(metric, &[("vpn", "pptp")]));
            for family in metric.collect() {
                for sample in family.get_metric() {
                    assert_eq!(1, sample.get_label().len());
                    assert_eq!("vpn", sample.get_label()[0].get_name());
                    assert_eq!("pptp", sample.get_label()[0].get_value());
                }
            }
        }
        assert_eq!(Some(1.0), exposed(&map.client_enabled_metric, &[]));
        assert_eq!(Some(1.0), exposed(&map.client_up_metric, &[]));
        assert_eq!(Some(1.0), exposed(&map.client_state_metric, &[("state", "up")]));
        assert_eq!(Some(1.0), exposed(&map.client_last_error_metric, &[("last_error", "none")]));
        let cases: [(&dyn Collector, f64); 5] = [
            (&map.client_rate_up_bytes_per_second_metric, 0.0),
            (&map.client_rate_down_bytes_per_second_metric, 0.0),
            (&map.client_bytes_up_metric, 94.0),
            (&map.client_bytes_down_metric, 94.0),
            (&map.client_last_up_timestamp_seconds_metric, 1392904510.0),
        ];
        for (metric, expected) in cases {
            assert_eq!(Some(expected), exposed(metric, &[]));
            for family in metric.collect() {
                for sample in family.get_metric() {
                    assert!(sample.get_label().is_empty());
                }
            }
        }
    }

    #[tokio::test]
    async fn set_sums_connections_per_vpn_and_maps_distinct_client_values() {
        let (server, authenticator) = mock_api().await;
        let mut connections = fixture("api_v4_vpn_connection-get").await;
        connections["result"].as_array_mut().unwrap().extend([
            json!({"vpn":"pptp", "rx_bytes":6, "tx_bytes":17}),
            json!({"vpn":"wireguard", "rx_bytes":3, "tx_bytes":5}),
        ]);
        serve_cycle(&server,
            json!({"success":true,"result":[{"name":"pptp","type":"pptp","state":"started","connection_count":3,"auth_connection_count":2}]}),
            connections,
            json!({"success":true,"result":{"enabled":true,"state":"down","last_error":"connect_timeout","last_up":123,"stats":{"rate_up":7,"rate_down":11,"bytes_up":13,"bytes_down":19}}}),
        ).await;
        let factory = authenticator.login().await.unwrap();
        let mut map = VpnMetricMap::new(&factory, "test_vpn_sums".to_string());
        map.set().await.unwrap();

        assert_eq!(Some(100.0), exposed(&map.connections_rx_bytes_metric, &[("vpn","pptp")]));
        assert_eq!(Some(111.0), exposed(&map.connections_tx_bytes_metric, &[("vpn","pptp")]));
        assert_eq!(Some(3.0), exposed(&map.connections_rx_bytes_metric, &[("vpn","wireguard")]));
        assert_eq!(Some(5.0), exposed(&map.connections_tx_bytes_metric, &[("vpn","wireguard")]));
        assert_eq!(2, series_count(&map.connections_rx_bytes_metric));
        assert_eq!(2, series_count(&map.connections_tx_bytes_metric));
        let labels = [("name","pptp"),("type","pptp")];
        assert_eq!(Some(1.0), exposed(&map.server_started_metric, &labels));
        assert_eq!(Some(3.0), exposed(&map.server_connections_metric, &labels));
        assert_eq!(Some(2.0), exposed(&map.server_authenticated_connections_metric, &labels));
        assert_eq!(Some(1.0), exposed(&map.server_state_metric, &[("name","pptp"),("type","pptp"),("state","started")]));
        assert_eq!(Some(0.0), exposed(&map.client_up_metric, &[]));
        assert_eq!(Some(1.0), exposed(&map.client_state_metric, &[("state","down")]));
        assert_eq!(Some(1.0), exposed(&map.client_last_error_metric, &[("last_error","connect_timeout")]));
        let cases: [(&dyn Collector, f64); 5] = [
            (&map.client_rate_up_bytes_per_second_metric, 7.0),
            (&map.client_rate_down_bytes_per_second_metric, 11.0),
            (&map.client_bytes_up_metric, 13.0),
            (&map.client_bytes_down_metric, 19.0),
            (&map.client_last_up_timestamp_seconds_metric, 123.0),
        ];
        for (metric, value) in cases {
            assert_eq!(Some(value), exposed(metric, &[]));
        }
    }

    #[tokio::test]
    async fn set_propagates_connection_permission_error() {
        let (server, authenticator) = mock_api().await;
        serve_cycle(&server,
            fixture("api_v4_vpn-get").await,
            json!({"success":false,"error_code":"insufficient_rights"}),
            fixture("api_v4_vpn_client_status_disabled-get").await,
        ).await;
        let factory = authenticator.login().await.unwrap();
        let mut map = VpnMetricMap::new(&factory, "test_vpn_rights".to_string());
        let err = map.set().await.unwrap_err();
        assert!(err.to_string().contains("insufficient_rights"), "{err}");
    }

    #[tokio::test]
    async fn set_removes_stale_series_only_after_all_calls_succeed() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let mut map = VpnMetricMap::new(&factory, "test_vpn_lifecycle".to_string());
        serve_cycle(&server,
            fixture("api_v4_vpn-get").await,
            fixture("api_v4_vpn_connection-get").await,
            fixture("api_v4_vpn_client_status-get").await,
        ).await;
        map.set().await.unwrap();
        assert_eq!(Some(94.0), exposed(&map.connections_rx_bytes_metric, &[("vpn","pptp")]));
        assert_eq!(Some(94.0), exposed(&map.connections_tx_bytes_metric, &[("vpn","pptp")]));

        // Fail each endpoint in turn, including the final call after empty collections.
        for failing_endpoint in 0..3 {
            server.reset().await;
            let mut bodies = [json!({"success":true}), json!({"success":true}), json!({"success":true,"result":{"enabled":false}})];
            bodies[failing_endpoint] = json!({"success":false,"error_code":"internal_error"});
            serve_cycle(&server, bodies[0].clone(), bodies[1].clone(), bodies[2].clone()).await;
            let err = map.set().await.unwrap_err();
            assert!(err.to_string().contains("internal_error"), "{err}");
            assert_eq!(Some(94.0), exposed(&map.connections_rx_bytes_metric, &[("vpn","pptp")]));
            assert_eq!(Some(94.0), exposed(&map.connections_tx_bytes_metric, &[("vpn","pptp")]));
            assert_eq!(5, series_count(&map.server_started_metric));
            assert_eq!(Some(1.0), exposed(&map.client_up_metric, &[]));
            assert_eq!(Some(94.0), exposed(&map.client_bytes_down_metric, &[]));
        }

        server.reset().await;
        serve_cycle(&server, json!({"success":true}), json!({"success":true}),
            fixture("api_v4_vpn_client_status_disabled-get").await).await;
        map.set().await.unwrap();
        assert_eq!(None, exposed(&map.connections_rx_bytes_metric, &[]));
        assert_eq!(None, exposed(&map.connections_tx_bytes_metric, &[]));
        for metric in [&map.server_state_metric, &map.server_started_metric,
            &map.server_connections_metric, &map.server_authenticated_connections_metric] {
            assert_eq!(None, exposed(metric, &[]));
        }
        assert_eq!(Some(0.0), exposed(&map.client_enabled_metric, &[]));
        assert_client_optional_absent(&map);
    }

    #[tokio::test]
    async fn set_omits_missing_numeric_fields_but_exposes_explicit_zero() {
        let (server, authenticator) = mock_api().await;
        serve_cycle(&server,
            json!({"success":true,"result":[{"name":"pptp","type":"pptp","state":"starting"}]}),
            json!({"success":true,"result":[{"vpn":"pptp","tx_bytes":0},{"vpn":"wireguard","rx_bytes":0},{"vpn":"ipsec"}]}),
            json!({"success":true,"result":{"enabled":true,"last_up":0,"stats":{}}}),
        ).await;
        let factory = authenticator.login().await.unwrap();
        let mut map = VpnMetricMap::new(&factory, "test_vpn_optional".to_string());
        map.set().await.unwrap();
        assert_eq!(None, exposed(&map.server_connections_metric, &[]));
        assert_eq!(None, exposed(&map.server_authenticated_connections_metric, &[]));
        assert_eq!(Some(0.0), exposed(&map.server_started_metric, &[("name","pptp")]));
        assert_eq!(None, exposed(&map.connections_rx_bytes_metric, &[("vpn","pptp")]));
        assert_eq!(Some(0.0), exposed(&map.connections_tx_bytes_metric, &[("vpn","pptp")]));
        assert_eq!(Some(0.0), exposed(&map.connections_rx_bytes_metric, &[("vpn","wireguard")]));
        assert_eq!(None, exposed(&map.connections_tx_bytes_metric, &[("vpn","wireguard")]));
        assert_eq!(None, exposed(&map.connections_rx_bytes_metric, &[("vpn","ipsec")]));
        assert_eq!(None, exposed(&map.connections_tx_bytes_metric, &[("vpn","ipsec")]));
        assert_eq!(Some(1.0), exposed(&map.client_enabled_metric, &[]));
        assert_client_optional_absent(&map);
    }

    fn client_status(bytes_up: i64, bytes_down: i64) -> Value {
        json!({"success":true,"result":{"enabled":true,"state":"up",
            "stats":{"rate_up":0,"rate_down":0,"bytes_up":bytes_up,"bytes_down":bytes_down}}})
    }

    #[tokio::test]
    async fn set_exposes_client_transferred_bytes_as_counters_following_reconnections() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let mut map = VpnMetricMap::new(&factory, "test_vpn_counters".to_string());
        let empty = || json!({"success":true});
        let up = |map: &VpnMetricMap<'_>| exposed(&map.client_bytes_up_metric, &[]);
        let down = |map: &VpnMetricMap<'_>| exposed(&map.client_bytes_down_metric, &[]);

        serve_cycle(&server, empty(), empty(), client_status(94, 40)).await;
        map.set().await.unwrap();
        for (metric, name) in [
            (&map.client_bytes_up_metric, "test_vpn_counters_vpn_client_uploaded_bytes_total"),
            (&map.client_bytes_down_metric, "test_vpn_counters_vpn_client_downloaded_bytes_total"),
        ] {
            let families = metric.collect();
            assert_eq!(name, families[0].get_name());
            assert_eq!(MetricType::COUNTER, families[0].get_field_type());
        }
        assert_eq!((Some(94.0), Some(40.0)), (up(&map), down(&map)));

        // the box value grows; the series is updated in place, never removed and recreated,
        // so a concurrent scrape cannot read 0 and take it for a counter reset
        let series_before = map.client_bytes_up_metric.with_label_values(&[]);
        server.reset().await;
        serve_cycle(&server, empty(), empty(), client_status(150, 41)).await;
        map.set().await.unwrap();
        assert_eq!((Some(150.0), Some(41.0)), (up(&map), down(&map)));
        assert_eq!(150, series_before.get());

        // a failed collection keeps the last value
        server.reset().await;
        serve_cycle(&server, empty(), empty(), json!({"success":false,"error_code":"internal_error"})).await;
        map.set().await.unwrap_err();
        assert_eq!((Some(150.0), Some(41.0)), (up(&map), down(&map)));

        // the box restarted counting from zero
        server.reset().await;
        serve_cycle(&server, empty(), empty(), client_status(10, 3)).await;
        map.set().await.unwrap();
        assert_eq!((Some(10.0), Some(3.0)), (up(&map), down(&map)));

        // no more stats: the series disappear
        server.reset().await;
        serve_cycle(&server, empty(), empty(), fixture("api_v4_vpn_client_status_disabled-get").await).await;
        map.set().await.unwrap();
        assert_eq!((None, None), (up(&map), down(&map)));
    }

    #[tokio::test]
    async fn set_fails_and_keeps_last_snapshot_when_client_status_has_no_result() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let mut map = VpnMetricMap::new(&factory, "test_vpn_status_missing".to_string());
        serve_cycle(&server,
            fixture("api_v4_vpn-get").await,
            fixture("api_v4_vpn_connection-get").await,
            fixture("api_v4_vpn_client_status-get").await,
        ).await;
        map.set().await.unwrap();

        server.reset().await;
        serve_cycle(&server, fixture("api_v4_vpn-get").await, json!({"success":true}), json!({"success":true})).await;
        let err = map.set().await.unwrap_err();

        assert!(err.to_string().contains("vpn_client/status"), "{err}");
        assert_eq!(Some(1.0), exposed(&map.client_up_metric, &[]));
        assert_eq!(Some(94.0), exposed(&map.client_bytes_up_metric, &[]));
        assert_eq!(Some(94.0), exposed(&map.connections_rx_bytes_metric, &[("vpn", "pptp")]));
    }

    #[test]
    fn counter_is_reset_only_when_the_box_value_decreases() {
        assert_eq!((false, 56), counter_step(94, 150));
        assert_eq!((false, 0), counter_step(150, 150));
        assert_eq!((true, 10), counter_step(150, 10));
        assert_eq!((false, 94), counter_step(0, 94));
    }
}
