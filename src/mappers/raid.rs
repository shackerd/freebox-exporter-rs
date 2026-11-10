use async_trait::async_trait;
use log::debug;
use prometheus_exporter::prometheus::{
    register_gauge_vec, register_int_gauge_vec, GaugeVec, IntGaugeVec,
};
use prometheus_exporter::UpdateLock;
use reqwest::Client;
use serde::Deserialize;

use super::MetricMap;
use crate::core::common::{
    http_client_factory::{AuthenticatedHttpClientFactory, ManagedHttpClient},
    transport::get_api_result,
};

/// RAID API is documented since API v8.2, its introduction version is unknown
pub const MIN_API_VERSION: u32 = 8;

#[derive(Deserialize, Clone, Debug)]
struct RaidArray {
    id: Option<i64>,
    name: Option<String>,
    level: Option<String>,
    degraded: Option<bool>,
    state: Option<String>,
    sync_action: Option<String>,
    sync_completed_percent: Option<f64>,
    sync_speed: Option<i64>,
    array_size: Option<i64>,
    raid_disks: Option<i64>,
    last_check: Option<i64>,
    next_check: Option<i64>,
    members: Option<Vec<RaidMember>>,
}

#[derive(Deserialize, Clone, Debug)]
struct RaidMember {
    id: Option<i64>,
    device_location: Option<String>,
    role: Option<String>,
    corrected_read_errors: Option<i64>,
    total_bytes: Option<i64>,
    disk: Option<RaidDisk>,
}

#[derive(Deserialize, Clone, Debug)]
struct RaidDisk {
    temp: Option<i64>,
}

pub struct RaidMetricMap<'a> {
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    managed_client: Option<ManagedHttpClient>,
    degraded_metric: IntGaugeVec,
    state_metric: IntGaugeVec,
    sync_action_metric: IntGaugeVec,
    sync_completed_ratio_metric: GaugeVec,
    sync_speed_bytes_per_second_metric: IntGaugeVec,
    size_bytes_metric: IntGaugeVec,
    disks_expected_metric: IntGaugeVec,
    last_check_timestamp_seconds_metric: IntGaugeVec,
    next_check_timestamp_seconds_metric: IntGaugeVec,
    member_role_metric: IntGaugeVec,
    member_corrected_read_errors_metric: IntGaugeVec,
    member_size_bytes_metric: IntGaugeVec,
    member_temperature_celsius_metric: IntGaugeVec,
}

impl<'a> RaidMetricMap<'a> {
    pub fn new(factory: &'a AuthenticatedHttpClientFactory<'a>, prefix: String) -> Self {
        Self {
            factory,
            managed_client: None,
            degraded_metric: register_int_gauge_vec!(
                format!("{prefix}_raid_degraded"),
                "1 if the array is degraded, 0 otherwise",
                &["id", "name", "level"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_raid_degraded gauge: {e}")),
            state_metric: register_int_gauge_vec!(
                format!("{prefix}_raid_state"),
                "array state, always 1",
                &["id", "name", "state"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_raid_state gauge: {e}")),
            sync_action_metric: register_int_gauge_vec!(
                format!("{prefix}_raid_sync_action"),
                "array sync action, always 1",
                &["id", "name", "sync_action"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_raid_sync_action gauge: {e}")),
            sync_completed_ratio_metric: register_gauge_vec!(
                format!("{prefix}_raid_sync_completed_ratio"),
                "fraction of sync completed",
                &["id", "name", "level"]
            )
            .unwrap_or_else(|e| {
                panic!("cannot create {prefix}_raid_sync_completed_ratio gauge: {e}")
            }),
            sync_speed_bytes_per_second_metric: register_int_gauge_vec!(
                format!("{prefix}_raid_sync_speed_bytes_per_second"),
                "sync speed in bytes per second",
                &["id", "name", "level"]
            )
            .unwrap_or_else(|e| {
                panic!("cannot create {prefix}_raid_sync_speed_bytes_per_second gauge: {e}")
            }),
            size_bytes_metric: register_int_gauge_vec!(
                format!("{prefix}_raid_size_bytes"),
                "array size in bytes",
                &["id", "name", "level"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_raid_size_bytes gauge: {e}")),
            disks_expected_metric: register_int_gauge_vec!(
                format!("{prefix}_raid_disks_expected"),
                "expected number of array members",
                &["id", "name", "level"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_raid_disks_expected gauge: {e}")),
            last_check_timestamp_seconds_metric: register_int_gauge_vec!(
                format!("{prefix}_raid_last_check_timestamp_seconds"),
                "Unix timestamp of last check in seconds",
                &["id", "name", "level"]
            )
            .unwrap_or_else(|e| {
                panic!("cannot create {prefix}_raid_last_check_timestamp_seconds gauge: {e}")
            }),
            next_check_timestamp_seconds_metric: register_int_gauge_vec!(
                format!("{prefix}_raid_next_check_timestamp_seconds"),
                "Unix timestamp of next check in seconds",
                &["id", "name", "level"]
            )
            .unwrap_or_else(|e| {
                panic!("cannot create {prefix}_raid_next_check_timestamp_seconds gauge: {e}")
            }),
            member_role_metric: register_int_gauge_vec!(
                format!("{prefix}_raid_member_role"),
                "member role, always 1",
                &["array_id", "id", "device_location", "role"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_raid_member_role gauge: {e}")),
            member_corrected_read_errors_metric: register_int_gauge_vec!(
                format!("{prefix}_raid_member_corrected_read_errors"),
                "member corrected read errors",
                &["array_id", "id", "device_location"]
            )
            .unwrap_or_else(|e| {
                panic!("cannot create {prefix}_raid_member_corrected_read_errors gauge: {e}")
            }),
            member_size_bytes_metric: register_int_gauge_vec!(
                format!("{prefix}_raid_member_size_bytes"),
                "member size in bytes",
                &["array_id", "id", "device_location"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_raid_member_size_bytes gauge: {e}")),
            member_temperature_celsius_metric: register_int_gauge_vec!(
                format!("{prefix}_raid_member_temperature_celsius"),
                "member disk temperature in Celsius",
                &["array_id", "id", "device_location"]
            )
            .unwrap_or_else(|e| {
                panic!("cannot create {prefix}_raid_member_temperature_celsius gauge: {e}")
            }),
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
        self.degraded_metric.reset();
        self.state_metric.reset();
        self.sync_action_metric.reset();
        self.sync_completed_ratio_metric.reset();
        self.sync_speed_bytes_per_second_metric.reset();
        self.size_bytes_metric.reset();
        self.disks_expected_metric.reset();
        self.last_check_timestamp_seconds_metric.reset();
        self.next_check_timestamp_seconds_metric.reset();
        self.member_role_metric.reset();
        self.member_corrected_read_errors_metric.reset();
        self.member_size_bytes_metric.reset();
        self.member_temperature_celsius_metric.reset();
    }

    fn set_array_metrics(&self, array: &RaidArray) {
        let id = array.id.map(|i| i.to_string()).unwrap_or_default();
        let name = array.name.as_deref().unwrap_or_default();
        let level = array.level.as_deref().unwrap_or("unknown");
        let labels = [&*id, name, level];

        if let Some(value) = array.degraded {
            self.degraded_metric
                .with_label_values(&labels)
                .set(value as i64);
        }
        if let Some(state) = array.state.as_deref() {
            self.state_metric
                .with_label_values(&[&id, name, state])
                .set(1);
        }
        if let Some(action) = array.sync_action.as_deref() {
            self.sync_action_metric
                .with_label_values(&[&id, name, action])
                .set(1);
        }
        if let Some(value) = array.sync_completed_percent {
            self.sync_completed_ratio_metric
                .with_label_values(&labels)
                .set(value / 100.0);
        }
        if let Some(value) = array.sync_speed {
            self.sync_speed_bytes_per_second_metric
                .with_label_values(&labels)
                .set(value);
        }
        if let Some(value) = array.array_size {
            self.size_bytes_metric.with_label_values(&labels).set(value);
        }
        if let Some(value) = array.raid_disks {
            self.disks_expected_metric
                .with_label_values(&labels)
                .set(value);
        }
        if let Some(value) = array.last_check.filter(|v| *v != 0) {
            self.last_check_timestamp_seconds_metric
                .with_label_values(&labels)
                .set(value);
        }
        if let Some(value) = array.next_check.filter(|v| *v != 0) {
            self.next_check_timestamp_seconds_metric
                .with_label_values(&labels)
                .set(value);
        }
        for member in array.members.iter().flatten() {
            let member_id = member.id.map(|i| i.to_string()).unwrap_or_default();
            let location = member.device_location.as_deref().unwrap_or("unknown");
            let labels = [&*id, &*member_id, location];
            if let Some(role) = member.role.as_deref() {
                self.member_role_metric
                    .with_label_values(&[&id, &member_id, location, role])
                    .set(1);
            }
            if let Some(value) = member.corrected_read_errors {
                self.member_corrected_read_errors_metric
                    .with_label_values(&labels)
                    .set(value);
            }
            if let Some(value) = member.total_bytes {
                self.member_size_bytes_metric
                    .with_label_values(&labels)
                    .set(value);
            }
            if let Some(value) = member.disk.as_ref().and_then(|disk| disk.temp) {
                self.member_temperature_celsius_metric
                    .with_label_values(&labels)
                    .set(value);
            }
        }
    }

    async fn set_all(
        &mut self,
        lock: &UpdateLock,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_managed_client().await?;
        let api_url = self.factory.api_url.clone();
        let arrays = get_api_result::<Vec<RaidArray>>(
            &client,
            &api_url,
            &format!("v{MIN_API_VERSION}/storage/raid/"),
        )
        .await?
        .unwrap_or_default();

        // Keep the last known values if the API call fails.
        // a scrape waits for the update to complete instead of returning part of it
        let _update = lock.lock();
        self.reset_all();
        for array in &arrays {
            self.set_array_metrics(array);
        }
        Ok(())
    }
}

#[async_trait]
impl<'a> MetricMap<'a> for RaidMetricMap<'a> {
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

    fn exposed(vec: &impl Collector, labels: &[(&str, &str)]) -> Option<f64> {
        vec.collect()
            .iter()
            .flat_map(|family| family.get_metric())
            .find(|m| {
                labels.iter().all(|(k, v)| {
                    m.get_label()
                        .iter()
                        .any(|l| l.name() == *k && l.value() == *v)
                })
            })
            .map(|m| m.get_gauge().get_value())
    }

    fn collectors<'m>(map: &'m RaidMetricMap<'_>) -> Vec<&'m dyn Collector> {
        vec![
            &map.degraded_metric,
            &map.state_metric,
            &map.sync_action_metric,
            &map.sync_completed_ratio_metric,
            &map.sync_speed_bytes_per_second_metric,
            &map.size_bytes_metric,
            &map.disks_expected_metric,
            &map.last_check_timestamp_seconds_metric,
            &map.next_check_timestamp_seconds_metric,
            &map.member_role_metric,
            &map.member_corrected_read_errors_metric,
            &map.member_size_bytes_metric,
            &map.member_temperature_celsius_metric,
        ]
    }

    /// Metric name, labels and value of one exposed series
    type Series = (String, Vec<(String, String)>, f64);

    fn snapshot(map: &RaidMetricMap<'_>) -> Vec<Series> {
        let mut values = Vec::new();
        for collector in collectors(map) {
            for family in collector.collect() {
                for metric in family.get_metric() {
                    let mut labels: Vec<_> = metric
                        .get_label()
                        .iter()
                        .map(|label| (label.name().to_string(), label.value().to_string()))
                        .collect();
                    labels.sort();
                    values.push((
                        family.name().to_string(),
                        labels,
                        metric.get_gauge().get_value(),
                    ));
                }
            }
        }
        values.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        values
    }

    async fn fixture(endpoint: &'static str) -> Value {
        serde_json::from_str(&get_specs_data("raid", endpoint).await.unwrap()).unwrap()
    }

    #[tokio::test]
    async fn set_exposes_doc_array_and_all_four_members() {
        let (server, authenticator) = mock_api().await;
        serve(
            &server,
            "/api/v8/storage/raid/",
            fixture("api_v8_storage_raid-get").await,
        )
        .await;
        let factory = authenticator.login().await.unwrap();
        let mut map = RaidMetricMap::new(&factory, "test_raid_doc".to_string());
        map.set(update_lock()).await.unwrap();

        let labels = [("id", "0"), ("name", "Freebox"), ("level", "raid5")];
        assert_eq!(Some(0.0), exposed(&map.degraded_metric, &labels));
        assert_eq!(Some(4.0), exposed(&map.disks_expected_metric, &labels));
        assert_eq!(
            Some(3_000_000_000_000.0),
            exposed(&map.size_bytes_metric, &labels)
        );
        assert_eq!(
            Some(0.0),
            exposed(&map.sync_speed_bytes_per_second_metric, &labels)
        );
        assert_eq!(
            Some(0.0),
            exposed(&map.sync_completed_ratio_metric, &labels)
        );
        assert_eq!(
            Some(1.0),
            exposed(
                &map.state_metric,
                &[("id", "0"), ("name", "Freebox"), ("state", "running")]
            )
        );
        assert_eq!(
            Some(1.0),
            exposed(
                &map.sync_action_metric,
                &[("id", "0"), ("name", "Freebox"), ("sync_action", "idle")]
            )
        );
        for (id, location, temp) in [
            ("1000", "sata-internal-p1", 43.0),
            ("2000", "sata-internal-p0", 47.0),
            ("3000", "sata-internal-p2", 46.0),
            ("4000", "sata-internal-p3", 46.0),
        ] {
            let labels = [("array_id", "0"), ("id", id), ("device_location", location)];
            assert_eq!(
                Some(1.0),
                exposed(
                    &map.member_role_metric,
                    &[
                        ("array_id", "0"),
                        ("id", id),
                        ("device_location", location),
                        ("role", "active")
                    ]
                )
            );
            assert_eq!(
                Some(temp),
                exposed(&map.member_temperature_celsius_metric, &labels)
            );
            assert_eq!(
                Some(0.0),
                exposed(&map.member_corrected_read_errors_metric, &labels)
            );
            assert_eq!(
                Some(1_000_000_000_000.0),
                exposed(&map.member_size_bytes_metric, &labels)
            );
        }
        for (index, collector) in collectors(&map).iter().enumerate() {
            let families = collector.collect();
            let count: usize = families.iter().map(|f| f.get_metric().len()).sum();
            assert_eq!(
                if index == 8 {
                    0
                } else if index >= 9 {
                    4
                } else {
                    1
                },
                count
            );
            let expected: &[&str] = match index {
                1 => &["id", "name", "state"],
                2 => &["id", "name", "sync_action"],
                9 => &["array_id", "id", "device_location", "role"],
                10..=12 => &["array_id", "id", "device_location"],
                _ => &["id", "name", "level"],
            };
            for metric in families.iter().flat_map(|family| family.get_metric()) {
                assert_eq!(expected.len(), metric.get_label().len());
                assert!(metric
                    .get_label()
                    .iter()
                    .all(|label| expected.contains(&label.name())));
            }
        }
    }

    #[tokio::test]
    async fn set_accepts_real_empty_response() {
        let (server, authenticator) = mock_api().await;
        serve(
            &server,
            "/api/v8/storage/raid/",
            fixture("api_v8_storage_raid_empty-get").await,
        )
        .await;
        let factory = authenticator.login().await.unwrap();
        let mut map = RaidMetricMap::new(&factory, "test_raid_empty".to_string());
        map.set(update_lock()).await.unwrap();
        assert!(snapshot(&map).is_empty());
    }

    #[tokio::test]
    async fn set_exposes_degraded_faulty_and_resync_progress() {
        let (server, authenticator) = mock_api().await;
        serve(
            &server,
            "/api/v8/storage/raid/",
            json!({
                "success": true, "result": [{
                    "id": 7, "name": "rebuilding", "level": "raid1", "degraded": true,
                    "state": "error", "sync_action": "resync", "sync_completed_percent": 42,
                    "sync_speed": 123456, "array_size": 9876543210_i64, "raid_disks": 2,
                    "members": [{"id": 9, "array_id": 7, "device_location": "sata-internal-p0",
                        "role": "faulty", "corrected_read_errors": 17,
                        "total_bytes": 9876543210_i64, "disk": {"temp": 58}}]
                }]
            }),
        )
        .await;
        let factory = authenticator.login().await.unwrap();
        let mut map = RaidMetricMap::new(&factory, "test_raid_degraded".to_string());
        map.set(update_lock()).await.unwrap();
        let labels = [("id", "7"), ("name", "rebuilding"), ("level", "raid1")];
        assert_eq!(Some(1.0), exposed(&map.degraded_metric, &labels));
        assert_eq!(
            Some(0.42),
            exposed(&map.sync_completed_ratio_metric, &labels)
        );
        assert_eq!(
            Some(123456.0),
            exposed(&map.sync_speed_bytes_per_second_metric, &labels)
        );
        assert_eq!(Some(9876543210.0), exposed(&map.size_bytes_metric, &labels));
        assert_eq!(Some(2.0), exposed(&map.disks_expected_metric, &labels));
        assert_eq!(
            Some(1.0),
            exposed(
                &map.sync_action_metric,
                &[("id", "7"), ("sync_action", "resync")]
            )
        );
        assert_eq!(
            Some(1.0),
            exposed(&map.state_metric, &[("id", "7"), ("state", "error")])
        );
        let labels = [
            ("array_id", "7"),
            ("id", "9"),
            ("device_location", "sata-internal-p0"),
        ];
        assert_eq!(
            Some(1.0),
            exposed(
                &map.member_role_metric,
                &[
                    ("array_id", "7"),
                    ("id", "9"),
                    ("device_location", "sata-internal-p0"),
                    ("role", "faulty")
                ]
            )
        );
        assert_eq!(
            Some(17.0),
            exposed(&map.member_corrected_read_errors_metric, &labels)
        );
        assert_eq!(
            Some(9876543210.0),
            exposed(&map.member_size_bytes_metric, &labels)
        );
        assert_eq!(
            Some(58.0),
            exposed(&map.member_temperature_celsius_metric, &labels)
        );
    }

    #[tokio::test]
    async fn set_omits_zero_timestamps_and_exposes_nonzero_timestamps() {
        let (server, authenticator) = mock_api().await;
        serve(
            &server,
            "/api/v8/storage/raid/",
            json!({"success": true, "result": [
                {"id": 1, "last_check": 1576082428, "next_check": 0},
                {"id": 2, "last_check": 0, "next_check": 1576086028}
            ]}),
        )
        .await;
        let factory = authenticator.login().await.unwrap();
        let mut map = RaidMetricMap::new(&factory, "test_raid_timestamps".to_string());
        map.set(update_lock()).await.unwrap();
        assert_eq!(
            Some(1576082428.0),
            exposed(&map.last_check_timestamp_seconds_metric, &[("id", "1")])
        );
        assert_eq!(
            None,
            exposed(&map.next_check_timestamp_seconds_metric, &[("id", "1")])
        );
        assert_eq!(
            None,
            exposed(&map.last_check_timestamp_seconds_metric, &[("id", "2")])
        );
        assert_eq!(
            Some(1576086028.0),
            exposed(&map.next_check_timestamp_seconds_metric, &[("id", "2")])
        );
    }

    #[tokio::test]
    async fn set_reports_insufficient_rights() {
        let (server, authenticator) = mock_api().await;
        serve(
            &server,
            "/api/v8/storage/raid/",
            json!({
                "success": false, "error_code": "insufficient_rights"
            }),
        )
        .await;
        let factory = authenticator.login().await.unwrap();
        let mut map = RaidMetricMap::new(&factory, "test_raid_rights".to_string());
        let err = map.set(update_lock()).await.unwrap_err();
        assert!(err.to_string().contains("insufficient_rights"), "{err}");
    }

    #[tokio::test]
    async fn set_keeps_all_series_on_failure_and_removes_disappeared_array() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let mut map = RaidMetricMap::new(&factory, "test_raid_lifecycle".to_string());
        let mut body = fixture("api_v8_storage_raid-get").await;
        body["result"][0]["next_check"] = json!(1576086028);
        let first = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v8/storage/raid/"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(body))
            .mount_as_scoped(&server)
            .await;
        map.set(update_lock()).await.unwrap();
        let before = snapshot(&map);
        assert_eq!(25, before.len());
        assert!(collectors(&map)
            .iter()
            .all(|c| c.collect().iter().any(|f| !f.get_metric().is_empty())));
        drop(first);

        let failure = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v8/storage/raid/"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "success": false, "error_code": "internal_error"
            })))
            .mount_as_scoped(&server)
            .await;
        assert!(map.set(update_lock()).await.is_err());
        assert_eq!(before, snapshot(&map));
        drop(failure);

        serve(
            &server,
            "/api/v8/storage/raid/",
            fixture("api_v8_storage_raid_empty-get").await,
        )
        .await;
        map.set(update_lock()).await.unwrap();
        assert!(snapshot(&map).is_empty());
    }

    #[tokio::test]
    async fn set_omits_missing_values_and_clears_previous_values() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let mut map = RaidMetricMap::new(&factory, "test_raid_missing".to_string());
        let first = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v8/storage/raid/"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(fixture("api_v8_storage_raid-get").await),
            )
            .mount_as_scoped(&server)
            .await;
        map.set(update_lock()).await.unwrap();
        assert_eq!(24, snapshot(&map).len());
        drop(first);

        serve(
            &server,
            "/api/v8/storage/raid/",
            json!({"success": true, "result": [{
                "id": 0, "name": "Freebox", "level": "raid5", "members": [
                    {"id": 1000, "device_location": "sata-internal-p1"},
                    {"id": 2000, "device_location": "sata-internal-p0", "disk": {}},
                    {"id": 3000, "device_location": "sata-internal-p2", "disk": {"temp": null}}
                ]
            }]}),
        )
        .await;
        map.set(update_lock()).await.unwrap();
        assert!(snapshot(&map).is_empty());
    }

    #[tokio::test]
    async fn collection_waits_for_a_scrape_in_progress() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_raid_lock";
        let mut map = RaidMetricMap::new(&factory, prefix.to_string());

        let first = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v8/storage/raid/"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(fixture("api_v8_storage_raid-get").await),
            )
            .mount_as_scoped(&server)
            .await;
        map.set(update_lock()).await.unwrap();
        let before = scrape(prefix);
        assert!(!before.is_empty());
        drop(first);

        // the array disappeared
        serve(
            &server,
            "/api/v8/storage/raid/",
            fixture("api_v8_storage_raid_empty-get").await,
        )
        .await;
        let lock = new_update_lock();
        let (result, during) = collect_while_locked(&lock, map.set(&lock), prefix).await;
        result.unwrap();

        assert_eq!(before, during);
        assert!(snapshot(&map).is_empty());
    }
}
