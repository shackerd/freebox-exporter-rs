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

/// VM API was added in API v7.0 (Freebox Delta)
pub const MIN_API_VERSION: u32 = 7;

const MEGABYTE: i64 = 1024 * 1024;

/// Subset of the VM object; other fields are ignored on purpose
/// (e.g. `bind_usb_ports` is documented as an array but returned as "" by real boxes)
#[derive(Deserialize, Clone, Debug)]
struct Vm {
    id: Option<i64>,
    name: Option<String>,
    status: Option<String>,
    /// megabytes
    memory: Option<i64>,
    vcpus: Option<i64>,
    os: Option<String>,
    disk_type: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
struct VmSystemInfo {
    /// megabytes, unit inferred from used_memory matching the sum of VM.memory
    total_memory: Option<i64>,
    used_memory: Option<i64>,
    total_cpus: Option<i64>,
    used_cpus: Option<i64>,
}

pub struct VmMetricMap<'a> {
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    managed_client: Option<ManagedHttpClient>,
    running_metric: IntGaugeVec,
    status_metric: IntGaugeVec,
    memory_metric: IntGaugeVec,
    vcpus_metric: IntGaugeVec,
    info_metric: IntGaugeVec,
    host_memory_total_metric: IntGaugeVec,
    host_memory_used_metric: IntGaugeVec,
    host_cpus_total_metric: IntGaugeVec,
    host_cpus_used_metric: IntGaugeVec,
}

impl<'a> VmMetricMap<'a> {
    pub fn new(factory: &'a AuthenticatedHttpClientFactory<'a>, prefix: String) -> Self {
        Self {
            factory,
            managed_client: None,
            running_metric: register_int_gauge_vec!(
                format!("{prefix}_vm_running"),
                "1 if the virtual machine is running, 0 otherwise",
                &["id", "name"]
            )
            .expect(&format!("cannot create {prefix}_vm_running gauge")),
            status_metric: register_int_gauge_vec!(
                format!("{prefix}_vm_status"),
                "virtual machine status (stopped, running, starting, stopping), always 1",
                &["id", "name", "status"]
            )
            .expect(&format!("cannot create {prefix}_vm_status gauge")),
            memory_metric: register_int_gauge_vec!(
                format!("{prefix}_vm_memory_allocated_bytes"),
                "memory allocated to the virtual machine",
                &["id", "name"]
            )
            .expect(&format!("cannot create {prefix}_vm_memory_allocated_bytes gauge")),
            vcpus_metric: register_int_gauge_vec!(
                format!("{prefix}_vm_vcpus"),
                "virtual CPUs allocated to the virtual machine",
                &["id", "name"]
            )
            .expect(&format!("cannot create {prefix}_vm_vcpus gauge")),
            info_metric: register_int_gauge_vec!(
                format!("{prefix}_vm_info"),
                "virtual machine configuration, always 1",
                &["id", "name", "os", "disk_type"]
            )
            .expect(&format!("cannot create {prefix}_vm_info gauge")),
            host_memory_total_metric: register_int_gauge_vec!(
                format!("{prefix}_vm_host_memory_total_bytes"),
                "memory available to virtual machines",
                &[]
            )
            .expect(&format!("cannot create {prefix}_vm_host_memory_total_bytes gauge")),
            host_memory_used_metric: register_int_gauge_vec!(
                format!("{prefix}_vm_host_memory_allocated_bytes"),
                "memory allocated to virtual machines",
                &[]
            )
            .expect(&format!("cannot create {prefix}_vm_host_memory_allocated_bytes gauge")),
            host_cpus_total_metric: register_int_gauge_vec!(
                format!("{prefix}_vm_host_vcpus"),
                "virtual CPUs available to virtual machines",
                &[]
            )
            .expect(&format!("cannot create {prefix}_vm_host_vcpus gauge")),
            host_cpus_used_metric: register_int_gauge_vec!(
                format!("{prefix}_vm_host_vcpus_allocated"),
                "virtual CPUs allocated to virtual machines",
                &[]
            )
            .expect(&format!("cannot create {prefix}_vm_host_vcpus_allocated gauge")),
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
        self.running_metric.reset();
        self.status_metric.reset();
        self.memory_metric.reset();
        self.vcpus_metric.reset();
        self.info_metric.reset();
        self.host_memory_total_metric.reset();
        self.host_memory_used_metric.reset();
        self.host_cpus_total_metric.reset();
        self.host_cpus_used_metric.reset();
    }

    fn set_vm_metrics(&self, vm: &Vm) {
        let id = vm.id.map(|i| i.to_string()).unwrap_or_default();
        let name = vm.name.as_deref().unwrap_or_default();
        let status = vm.status.as_deref().unwrap_or("unknown");

        self.running_metric
            .with_label_values(&[&id, name])
            .set((status == "running") as i64);
        self.status_metric
            .with_label_values(&[&id, name, status])
            .set(1);
        self.info_metric
            .with_label_values(&[
                &id,
                name,
                vm.os.as_deref().unwrap_or("unknown"),
                vm.disk_type.as_deref().unwrap_or("unknown"),
            ])
            .set(1);

        // unknown values are not exposed rather than reported as 0
        if let Some(memory) = vm.memory {
            self.memory_metric
                .with_label_values(&[&id, name])
                .set(memory * MEGABYTE);
        }
        if let Some(vcpus) = vm.vcpus {
            self.vcpus_metric.with_label_values(&[&id, name]).set(vcpus);
        }
    }

    fn set_host_metrics(&self, info: &VmSystemInfo) {
        if let Some(v) = info.total_memory {
            self.host_memory_total_metric.with_label_values::<&str>(&[]).set(v * MEGABYTE);
        }
        if let Some(v) = info.used_memory {
            self.host_memory_used_metric.with_label_values::<&str>(&[]).set(v * MEGABYTE);
        }
        if let Some(v) = info.total_cpus {
            self.host_cpus_total_metric.with_label_values::<&str>(&[]).set(v);
        }
        if let Some(v) = info.used_cpus {
            self.host_cpus_used_metric.with_label_values::<&str>(&[]).set(v);
        }
    }

    async fn set_all(
        &mut self,
        lock: &UpdateLock,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_managed_client().await?;
        let api_url = self.factory.api_url.clone();

        let info =
            get_api_result::<VmSystemInfo>(&client, &api_url, &format!("v{MIN_API_VERSION}/vm/info/"))
                .await?
                .ok_or_else(|| {
                    Box::new(FreeboxResponseError::new("vm/info response has no result".to_string()))
                })?;
        // the API omits the result field when there is no VM
        let vms = get_api_result::<Vec<Vm>>(&client, &api_url, &format!("v{MIN_API_VERSION}/vm/"))
            .await?
            .unwrap_or_default();

        // reset only once both calls succeeded, so a failure keeps the last known values
        // a scrape waits for the update to complete instead of returning part of it
        let _update = lock.lock();
        self.reset_all();

        self.set_host_metrics(&info);
        for vm in vms.iter() {
            self.set_vm_metrics(vm);
        }

        Ok(())
    }
}

#[async_trait]
impl<'a> MetricMap<'a> for VmMetricMap<'a> {
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

    /// Value exposed for the series matching all `labels`, None when it does not exist
    fn exposed(vec: &IntGaugeVec, labels: &[(&str, &str)]) -> Option<f64> {
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

    async fn fixture(endpoint: &'static str) -> Value {
        serde_json::from_str(&get_specs_data("vm", endpoint).await.unwrap()).unwrap()
    }

    #[tokio::test]
    async fn set_exposes_vm_and_host_metrics_from_real_box_response() {
        let (server, authenticator) = mock_api().await;
        serve(&server, "/api/v7/vm/", fixture("api_v7_vm-get").await).await;
        serve(&server, "/api/v7/vm/info/", fixture("api_v7_vm_info-get").await).await;
        let factory = authenticator.login().await.unwrap();
        let mut map = VmMetricMap::new(&factory, "test_vm_real".to_string());

        map.set(update_lock()).await.unwrap();

        let vm = [("id", "0"), ("name", "name_vm-1")];
        assert_eq!(Some(1.0), exposed(&map.running_metric, &vm));
        assert_eq!(
            Some(1.0),
            exposed(&map.status_metric, &[("id", "0"), ("status", "running")])
        );
        assert_eq!(Some(2048.0 * 1024.0 * 1024.0), exposed(&map.memory_metric, &vm));
        assert_eq!(Some(2.0), exposed(&map.vcpus_metric, &vm));
        assert_eq!(
            Some(1.0),
            exposed(&map.info_metric, &[("id", "0"), ("os", "ubuntu"), ("disk_type", "qcow2")])
        );
        assert_eq!(Some((7168 * MEGABYTE) as f64), exposed(&map.host_memory_total_metric, &[]));
        assert_eq!(Some((2048 * MEGABYTE) as f64), exposed(&map.host_memory_used_metric, &[]));
        assert_eq!(Some(3.0), exposed(&map.host_cpus_total_metric, &[]));
        assert_eq!(Some(2.0), exposed(&map.host_cpus_used_metric, &[]));
    }

    #[tokio::test]
    async fn set_exposes_stopped_vm_as_not_running() {
        let (server, authenticator) = mock_api().await;
        serve(
            &server,
            "/api/v7/vm/",
            json!({ "success": true, "result": [{ "id": 3, "name": "lab", "status": "stopped", "memory": 512, "vcpus": 1 }] }),
        )
        .await;
        serve(&server, "/api/v7/vm/info/", json!({ "success": true, "result": {} })).await;
        let factory = authenticator.login().await.unwrap();
        let mut map = VmMetricMap::new(&factory, "test_vm_stopped".to_string());

        map.set(update_lock()).await.unwrap();

        assert_eq!(Some(0.0), exposed(&map.running_metric, &[("id", "3")]));
        // an empty vm/info object must not be exposed as zero resources
        assert_eq!(None, exposed(&map.host_memory_total_metric, &[]));
        assert_eq!(None, exposed(&map.host_cpus_total_metric, &[]));
        assert_eq!(
            Some(1.0),
            exposed(&map.status_metric, &[("id", "3"), ("status", "stopped")])
        );
    }

    #[tokio::test]
    async fn set_accepts_missing_result_when_there_is_no_vm() {
        let (server, authenticator) = mock_api().await;
        serve(&server, "/api/v7/vm/", json!({ "success": true })).await;
        serve(&server, "/api/v7/vm/info/", fixture("api_v7_vm_info-get").await).await;
        let factory = authenticator.login().await.unwrap();
        let mut map = VmMetricMap::new(&factory, "test_vm_none".to_string());

        map.set(update_lock()).await.unwrap();

        assert!(map.running_metric.collect()[0].get_metric().is_empty());
        assert_eq!(Some(3.0), exposed(&map.host_cpus_total_metric, &[]));
    }

    #[tokio::test]
    async fn set_fails_and_keeps_host_metrics_when_vm_info_has_no_result() {
        let (server, authenticator) = mock_api().await;
        serve(&server, "/api/v7/vm/", fixture("api_v7_vm-get").await).await;
        serve(&server, "/api/v7/vm/info/", fixture("api_v7_vm_info-get").await).await;
        let factory = authenticator.login().await.unwrap();
        let mut map = VmMetricMap::new(&factory, "test_vm_info_missing".to_string());
        map.set(update_lock()).await.unwrap();

        server.reset().await;
        serve(&server, "/api/v7/vm/", fixture("api_v7_vm-get").await).await;
        serve(&server, "/api/v7/vm/info/", json!({ "success": true })).await;

        let err = map.set(update_lock()).await.unwrap_err();

        assert!(err.to_string().contains("vm/info"), "{err}");
        assert_eq!(Some(3.0), exposed(&map.host_cpus_total_metric, &[]));
    }

    #[tokio::test]
    async fn set_fails_with_freebox_error_code_when_permission_is_missing() {
        let (server, authenticator) = mock_api().await;
        serve(&server, "/api/v7/vm/info/", fixture("api_v7_vm_info-get").await).await;
        serve(
            &server,
            "/api/v7/vm/",
            json!({ "success": false, "error_code": "insufficient_rights", "msg": "Cette application n'est pas autorisée" }),
        )
        .await;
        let factory = authenticator.login().await.unwrap();
        let mut map = VmMetricMap::new(&factory, "test_vm_rights".to_string());

        let err = map.set(update_lock()).await.unwrap_err();

        assert!(err.to_string().contains("insufficient_rights"), "{err}");
    }

    #[tokio::test]
    async fn set_drops_series_of_deleted_vm_and_keeps_them_on_failure() {
        let (server, authenticator) = mock_api().await;
        serve(&server, "/api/v7/vm/info/", json!({ "success": true, "result": {} })).await;
        let factory = authenticator.login().await.unwrap();
        let mut map = VmMetricMap::new(&factory, "test_vm_lifecycle".to_string());

        // cycle 1: one VM
        let first = wiremock::Mock::given(wiremock::matchers::path("/api/v7/vm/"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(
                json!({ "success": true, "result": [{ "id": 1, "name": "a", "status": "running" }] }),
            ))
            .up_to_n_times(1)
            .mount_as_scoped(&server)
            .await;
        map.set(update_lock()).await.unwrap();
        drop(first);

        // cycle 2: API failure, last known values are kept
        let failing = wiremock::Mock::given(wiremock::matchers::path("/api/v7/vm/"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(
                json!({ "success": false, "error_code": "internal_error" }),
            ))
            .mount_as_scoped(&server)
            .await;
        assert!(map.set(update_lock()).await.is_err());
        assert_eq!(Some(1.0), exposed(&map.running_metric, &[("id", "1")]));
        drop(failing);

        // cycle 3: VM deleted, its series disappear
        serve(&server, "/api/v7/vm/", json!({ "success": true })).await;
        map.set(update_lock()).await.unwrap();
        assert_eq!(None, exposed(&map.running_metric, &[("id", "1")]));
    }

    #[tokio::test]
    async fn collection_waits_for_a_scrape_in_progress() {
        let (server, authenticator) = mock_api().await;
        serve(&server, "/api/v7/vm/info/", json!({ "success": true, "result": {} })).await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_vm_lock";
        let mut map = VmMetricMap::new(&factory, prefix.to_string());

        let first = wiremock::Mock::given(wiremock::matchers::path("/api/v7/vm/"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(
                json!({ "success": true, "result": [{ "id": 1, "name": "a", "status": "running" }] }),
            ))
            .mount_as_scoped(&server)
            .await;
        map.set(update_lock()).await.unwrap();
        let before = scrape(prefix);
        assert_eq!(Some(1.0), exposed(&map.running_metric, &[("id", "1")]));
        drop(first);

        // the VM was deleted
        serve(&server, "/api/v7/vm/", json!({ "success": true })).await;
        let lock = new_update_lock();
        let (result, during) = collect_while_locked(&lock, map.set(&lock), prefix).await;
        result.unwrap();

        assert_eq!(before, during);
        assert_eq!(None, exposed(&map.running_metric, &[("id", "1")]));
    }
}
