use async_trait::async_trait;
use log::debug;
use prometheus_exporter::prometheus::{register_int_gauge_vec, IntGaugeVec};
use prometheus_exporter::UpdateLock;
use reqwest::Client;
use serde::Deserialize;

use super::MetricMap;
use crate::core::common::{
    http_client_factory::{AuthenticatedHttpClientFactory, ManagedHttpClient},
    transport::{get_api_result, FreeboxResponseError},
};

/// DynDNS status answers identically from v3 to v16 on a Freebox Delta; v4 is the exporter baseline
pub const MIN_API_VERSION: u32 = 4;

/// Dynamic DNS providers supported by the box
const PROVIDERS: [&str; 3] = ["ovh", "dyndns", "noip"];

#[derive(Deserialize, Clone, Debug)]
struct DdnsStatus {
    status: Option<String>,
    last_refresh: Option<i64>,
    next_refresh: Option<i64>,
    last_error: Option<i64>,
    next_retry: Option<i64>,
}

pub struct DdnsMetricMap<'a> {
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    managed_client: Option<ManagedHttpClient>,
    status_metric: IntGaugeVec,
    last_refresh_timestamp_seconds_metric: IntGaugeVec,
    next_refresh_timestamp_seconds_metric: IntGaugeVec,
    last_error_timestamp_seconds_metric: IntGaugeVec,
    next_retry_timestamp_seconds_metric: IntGaugeVec,
}

impl<'a> DdnsMetricMap<'a> {
    pub fn new(factory: &'a AuthenticatedHttpClientFactory<'a>, prefix: String) -> Self {
        Self {
            factory,
            managed_client: None,
            status_metric: register_int_gauge_vec!(
                format!("{prefix}_ddns_status"),
                "dynamic DNS provider status (disabled, ok, wait, reqfail, authfail, ...), always 1",
                &["provider", "status"]
            )
            .expect(&format!("cannot create {prefix}_ddns_status gauge")),
            last_refresh_timestamp_seconds_metric: register_int_gauge_vec!(
                format!("{prefix}_ddns_last_refresh_timestamp_seconds"),
                "timestamp of the last dynamic DNS refresh",
                &["provider"]
            )
            .expect(&format!("cannot create {prefix}_ddns_last_refresh_timestamp_seconds gauge")),
            next_refresh_timestamp_seconds_metric: register_int_gauge_vec!(
                format!("{prefix}_ddns_next_refresh_timestamp_seconds"),
                "timestamp of the next dynamic DNS refresh",
                &["provider"]
            )
            .expect(&format!("cannot create {prefix}_ddns_next_refresh_timestamp_seconds gauge")),
            last_error_timestamp_seconds_metric: register_int_gauge_vec!(
                format!("{prefix}_ddns_last_error_timestamp_seconds"),
                "timestamp of the last dynamic DNS error",
                &["provider"]
            )
            .expect(&format!("cannot create {prefix}_ddns_last_error_timestamp_seconds gauge")),
            next_retry_timestamp_seconds_metric: register_int_gauge_vec!(
                format!("{prefix}_ddns_next_retry_timestamp_seconds"),
                "timestamp of the next dynamic DNS retry after an error",
                &["provider"]
            )
            .expect(&format!("cannot create {prefix}_ddns_next_retry_timestamp_seconds gauge")),
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
        self.status_metric.reset();
        self.last_refresh_timestamp_seconds_metric.reset();
        self.next_refresh_timestamp_seconds_metric.reset();
        self.last_error_timestamp_seconds_metric.reset();
        self.next_retry_timestamp_seconds_metric.reset();
    }

    fn set_provider_metrics(&self, provider: &str, status: &DdnsStatus) {
        if let Some(value) = status.status.as_deref() {
            self.status_metric
                .with_label_values(&[provider, value])
                .set(1);
        }
        // 0 when the event never happened or is not scheduled
        let timestamps = [
            (
                &self.last_refresh_timestamp_seconds_metric,
                status.last_refresh,
            ),
            (
                &self.next_refresh_timestamp_seconds_metric,
                status.next_refresh,
            ),
            (&self.last_error_timestamp_seconds_metric, status.last_error),
            (&self.next_retry_timestamp_seconds_metric, status.next_retry),
        ];
        for (metric, value) in timestamps {
            if let Some(value) = value.filter(|v| *v != 0) {
                metric.with_label_values(&[provider]).set(value);
            }
        }
    }

    async fn set_all(
        &mut self,
        lock: &UpdateLock,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_managed_client().await?;
        let api_url = self.factory.api_url.clone();
        let mut statuses = Vec::with_capacity(PROVIDERS.len());
        for provider in PROVIDERS {
            let endpoint = format!("v{MIN_API_VERSION}/connection/ddns/{provider}/status/");
            // the status is always an object, even for a disabled provider
            let status = get_api_result::<DdnsStatus>(&client, &api_url, &endpoint)
                .await?
                .ok_or_else(|| {
                    Box::new(FreeboxResponseError::new(format!(
                        "{endpoint} response has no result"
                    )))
                })?;
            statuses.push((provider, status));
        }

        // Keep the previous snapshot until all API calls have succeeded.
        // a scrape waits for the update to complete instead of returning part of it
        let _update = lock.lock();
        self.reset_all();
        for (provider, status) in &statuses {
            self.set_provider_metrics(provider, status);
        }
        Ok(())
    }
}

#[async_trait]
impl<'a> MetricMap<'a> for DdnsMetricMap<'a> {
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn set(
        &mut self,
        lock: &UpdateLock,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.set_all(lock).await
    }
}

#[cfg(test)]
mod tests {
    use prometheus_exporter::prometheus::core::Collector;
    use serde_json::{json, Value};

    use super::*;
    use crate::mappers::{
        api_specs_provider::get_specs_data,
        test_support::{
            collect_while_locked, mock_api, new_update_lock, scrape, serve, update_lock,
        },
    };

    /// Value of the first series matching all `labels`, None when it does not exist
    fn exposed(vec: &IntGaugeVec, labels: &[(&str, &str)]) -> Option<f64> {
        vec.collect()
            .iter()
            .flat_map(|family| family.get_metric().iter())
            .find(|m| {
                labels.iter().all(|(k, v)| {
                    m.get_label()
                        .iter()
                        .any(|l| l.name() == *k && l.value() == *v)
                })
            })
            .map(|m| m.get_gauge().get_value())
    }

    fn series_count(vec: &IntGaugeVec) -> usize {
        vec.collect()
            .iter()
            .map(|family| family.get_metric().len())
            .sum()
    }

    async fn fixture(endpoint: &'static str) -> Value {
        serde_json::from_str(&get_specs_data("ddns", endpoint).await.unwrap()).unwrap()
    }

    async fn serve_cycle(server: &wiremock::MockServer, ovh: Value, dyndns: Value, noip: Value) {
        serve(server, "/api/v4/connection/ddns/ovh/status/", ovh).await;
        serve(server, "/api/v4/connection/ddns/dyndns/status/", dyndns).await;
        serve(server, "/api/v4/connection/ddns/noip/status/", noip).await;
    }

    fn timestamps<'m>(map: &'m DdnsMetricMap<'_>) -> [&'m IntGaugeVec; 4] {
        [
            &map.last_refresh_timestamp_seconds_metric,
            &map.next_refresh_timestamp_seconds_metric,
            &map.last_error_timestamp_seconds_metric,
            &map.next_retry_timestamp_seconds_metric,
        ]
    }

    #[tokio::test]
    async fn set_exposes_real_box_statuses() {
        let (server, authenticator) = mock_api().await;
        serve_cycle(
            &server,
            fixture("api_v4_connection_ddns_ovh_status-get").await,
            fixture("api_v4_connection_ddns_disabled_status-get").await,
            fixture("api_v4_connection_ddns_disabled_status-get").await,
        )
        .await;
        let factory = authenticator.login().await.unwrap();
        let mut map = DdnsMetricMap::new(&factory, "test_ddns_real".to_string());
        map.set(update_lock()).await.unwrap();

        assert_eq!(3, series_count(&map.status_metric));
        assert_eq!(
            Some(1.0),
            exposed(&map.status_metric, &[("provider", "ovh"), ("status", "ok")])
        );
        assert_eq!(
            Some(1.0),
            exposed(
                &map.status_metric,
                &[("provider", "dyndns"), ("status", "disabled")]
            )
        );
        assert_eq!(
            Some(1.0),
            exposed(
                &map.status_metric,
                &[("provider", "noip"), ("status", "disabled")]
            )
        );
        let ovh = [("provider", "ovh")];
        assert_eq!(
            Some(1794216061.0),
            exposed(&map.last_refresh_timestamp_seconds_metric, &ovh)
        );
        assert_eq!(
            Some(1794216361.0),
            exposed(&map.next_refresh_timestamp_seconds_metric, &ovh)
        );
        // zero timestamps are not exposed, and disabled providers have only zeros
        assert_eq!(1, series_count(&map.last_refresh_timestamp_seconds_metric));
        assert_eq!(1, series_count(&map.next_refresh_timestamp_seconds_metric));
        assert_eq!(None, exposed(&map.last_error_timestamp_seconds_metric, &[]));
        assert_eq!(None, exposed(&map.next_retry_timestamp_seconds_metric, &[]));
    }

    #[tokio::test]
    async fn set_exposes_doc_example_and_maps_each_timestamp_to_its_metric() {
        let (server, authenticator) = mock_api().await;
        serve_cycle(
            &server,
            json!({"success":true,"result":{"status":"reqfail","last_refresh":11,"next_refresh":13,"last_error":17,"next_retry":19}}),
            fixture("api_v4_connection_ddns_dyndns_status_doc-get").await,
            fixture("api_v4_connection_ddns_disabled_status-get").await,
        )
        .await;
        let factory = authenticator.login().await.unwrap();
        let mut map = DdnsMetricMap::new(&factory, "test_ddns_doc".to_string());
        map.set(update_lock()).await.unwrap();

        assert_eq!(
            Some(1.0),
            exposed(
                &map.status_metric,
                &[("provider", "dyndns"), ("status", "hostinval")]
            )
        );
        assert_eq!(
            Some(1354127350.0),
            exposed(
                &map.last_error_timestamp_seconds_metric,
                &[("provider", "dyndns")]
            )
        );
        assert_eq!(
            Some(1.0),
            exposed(
                &map.status_metric,
                &[("provider", "ovh"), ("status", "reqfail")]
            )
        );
        let ovh = [("provider", "ovh")];
        for (metric, expected) in timestamps(&map).into_iter().zip([11.0, 13.0, 17.0, 19.0]) {
            assert_eq!(Some(expected), exposed(metric, &ovh));
        }
        assert_eq!(2, series_count(&map.last_error_timestamp_seconds_metric));
        assert_eq!(1, series_count(&map.next_retry_timestamp_seconds_metric));
    }

    #[tokio::test]
    async fn set_reports_insufficient_rights() {
        let (server, authenticator) = mock_api().await;
        serve_cycle(
            &server,
            fixture("api_v4_connection_ddns_ovh_status-get").await,
            json!({"success":false,"error_code":"insufficient_rights"}),
            fixture("api_v4_connection_ddns_disabled_status-get").await,
        )
        .await;
        let factory = authenticator.login().await.unwrap();
        let mut map = DdnsMetricMap::new(&factory, "test_ddns_rights".to_string());
        let err = map.set(update_lock()).await.unwrap_err();
        assert!(err.to_string().contains("insufficient_rights"), "{err}");
    }

    #[tokio::test]
    async fn set_keeps_last_snapshot_until_every_provider_succeeds() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let mut map = DdnsMetricMap::new(&factory, "test_ddns_lifecycle".to_string());
        serve_cycle(
            &server,
            fixture("api_v4_connection_ddns_ovh_status-get").await,
            fixture("api_v4_connection_ddns_dyndns_status_doc-get").await,
            fixture("api_v4_connection_ddns_disabled_status-get").await,
        )
        .await;
        map.set(update_lock()).await.unwrap();

        // fail each provider in turn, with an error or a response without result
        for (failing, provider) in PROVIDERS.iter().enumerate() {
            let no_result = format!("ddns/{provider}/status/ response has no result");
            for (failure, expected) in [
                (
                    json!({"success":false,"error_code":"internal_error"}),
                    "internal_error",
                ),
                (json!({"success":true}), no_result.as_str()),
            ] {
                server.reset().await;
                let mut bodies = [
                    json!({"success":true,"result":{"status":"ok","last_refresh":5}}),
                    json!({"success":true,"result":{"status":"ok","last_refresh":5}}),
                    json!({"success":true,"result":{"status":"ok","last_refresh":5}}),
                ];
                bodies[failing] = failure;
                serve_cycle(
                    &server,
                    bodies[0].clone(),
                    bodies[1].clone(),
                    bodies[2].clone(),
                )
                .await;
                let err = map.set(update_lock()).await.unwrap_err();
                assert!(err.to_string().contains(expected), "{err}");
                assert_eq!(
                    Some(1.0),
                    exposed(&map.status_metric, &[("provider", "ovh"), ("status", "ok")])
                );
                assert_eq!(
                    Some(1.0),
                    exposed(
                        &map.status_metric,
                        &[("provider", "dyndns"), ("status", "hostinval")]
                    )
                );
                assert_eq!(
                    Some(1794216061.0),
                    exposed(
                        &map.last_refresh_timestamp_seconds_metric,
                        &[("provider", "ovh")]
                    )
                );
                assert_eq!(
                    Some(1354127350.0),
                    exposed(
                        &map.last_error_timestamp_seconds_metric,
                        &[("provider", "dyndns")]
                    )
                );
            }
        }

        // the next successful collection replaces the snapshot: no stale status nor timestamp
        server.reset().await;
        let disabled = fixture("api_v4_connection_ddns_disabled_status-get").await;
        serve_cycle(&server, disabled.clone(), disabled.clone(), disabled).await;
        map.set(update_lock()).await.unwrap();
        assert_eq!(3, series_count(&map.status_metric));
        assert_eq!(None, exposed(&map.status_metric, &[("status", "ok")]));
        assert_eq!(
            None,
            exposed(&map.status_metric, &[("status", "hostinval")])
        );
        for metric in timestamps(&map) {
            assert_eq!(0, series_count(metric));
        }
    }

    #[tokio::test]
    async fn set_omits_missing_fields() {
        let (server, authenticator) = mock_api().await;
        let empty = || json!({"success":true,"result":{}});
        serve_cycle(&server, empty(), empty(), empty()).await;
        let factory = authenticator.login().await.unwrap();
        let mut map = DdnsMetricMap::new(&factory, "test_ddns_missing".to_string());
        map.set(update_lock()).await.unwrap();

        assert_eq!(0, series_count(&map.status_metric));
        for metric in timestamps(&map) {
            assert_eq!(0, series_count(metric));
        }
    }

    #[tokio::test]
    async fn collection_waits_for_a_scrape_in_progress() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_ddns_lock";
        let mut map = DdnsMetricMap::new(&factory, prefix.to_string());
        let disabled = fixture("api_v4_connection_ddns_disabled_status-get").await;

        serve_cycle(
            &server,
            fixture("api_v4_connection_ddns_ovh_status-get").await,
            fixture("api_v4_connection_ddns_dyndns_status_doc-get").await,
            disabled.clone(),
        )
        .await;
        map.set(update_lock()).await.unwrap();
        let before = scrape(prefix);
        assert!(!before.is_empty());

        // every provider is now disabled
        server.reset().await;
        serve_cycle(&server, disabled.clone(), disabled.clone(), disabled).await;
        let lock = new_update_lock();
        let (result, during) = collect_while_locked(&lock, map.set(&lock), prefix).await;
        result.unwrap();

        assert_eq!(before, during);
        assert_ne!(before, scrape(prefix));
    }
}
