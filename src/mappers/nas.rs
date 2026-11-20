use async_trait::async_trait;
use log::debug;
use prometheus_exporter::prometheus::{
    register_int_counter_vec_with_registry, register_int_gauge_vec_with_registry, IntCounterVec,
    IntGaugeVec, Registry,
};
use prometheus_exporter::UpdateLock;
use reqwest::Client;
use serde::Deserialize;

use super::MetricMap;
use crate::core::common::{
    http_client_factory::{AuthenticatedHttpClientFactory, ManagedHttpClient},
    transport::get_required_api_result,
};

/// NAS Storage Metrics for Freebox API
///
/// This module provides metrics collection for Freebox storage devices (NAS functionality).
/// It implements the Storage API as documented at: https://dev.freebox.fr/sdk/os/storage/
///
/// The implementation uses the `/api/v4/storage/disk/` endpoint to retrieve information about:
/// - Storage disks (internal, USB, SATA)
/// - Disk partitions and their usage
///
/// All data structures and field types match the official API specification.
/// Note: The Storage API is marked as [UNSTABLE] in the official documentation
/// and may change without notice in future releases.

#[derive(Deserialize, Clone, Debug)]
pub struct OperationProgress {
    pub done_steps: Option<i64>,
    pub max_steps: Option<i64>,
    pub percent: Option<i64>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct DiskPartition {
    pub id: i64,
    pub disk_id: i64,
    pub state: Option<String>,
    pub fstype: Option<String>,
    pub label: Option<String>,
    // path is base64 encoded mount point - not useful for metrics
    pub total_bytes: Option<i64>,
    pub used_bytes: Option<i64>,
    pub free_bytes: Option<i64>,
    pub fsck_result: Option<String>,
    pub operation_pct: Option<OperationProgress>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct StorageDisk {
    pub id: i64,
    #[serde(rename = "type")]
    pub disk_type: Option<String>,
    pub state: Option<String>,
    pub connector: Option<i64>,
    pub total_bytes: Option<i64>,
    pub table_type: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub firmware: Option<String>,
    pub temp: Option<i64>,
    pub operation_pct: Option<OperationProgress>,
    pub partitions: Option<Vec<DiskPartition>>,
    pub idle: Option<bool>,
    pub idle_duration: Option<i64>,
    pub spinning: Option<bool>,
    pub active_duration: Option<i64>,
    pub time_before_spindown: Option<i64>,
    // requests counted by the box since its start (when available)
    pub read_requests: Option<i64>,
    pub read_error_requests: Option<i64>,
    pub write_requests: Option<i64>,
    pub write_error_requests: Option<i64>,
}

pub struct NasMetricMap<'a> {
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    managed_client: Option<ManagedHttpClient>,

    // Disk metrics
    disk_total_bytes_metric: IntGaugeVec,
    disk_state_metric: IntGaugeVec,
    disk_temperature_metric: IntGaugeVec,
    disk_spinning_metric: IntGaugeVec,
    disk_idle_metric: IntGaugeVec,
    disk_idle_duration_metric: IntGaugeVec,
    disk_active_duration_metric: IntGaugeVec,
    disk_time_before_spindown_metric: IntGaugeVec,
    disk_connector_info_metric: IntGaugeVec,
    disk_table_type_info_metric: IntGaugeVec,
    disk_firmware_info_metric: IntGaugeVec,
    disk_operation_progress_metric: IntGaugeVec,
    disk_operation_steps_metric: IntGaugeVec,
    // counts of the box: reset with the other metrics, then set to the value of the box.
    // The update lock keeps a scrape from seeing the reset, and a lower value of the box,
    // after its restart, is a counter reset
    disk_read_requests_metric: IntCounterVec,
    disk_read_error_requests_metric: IntCounterVec,
    disk_write_requests_metric: IntCounterVec,
    disk_write_error_requests_metric: IntCounterVec,

    // Partition metrics
    partition_total_bytes_metric: IntGaugeVec,
    partition_used_bytes_metric: IntGaugeVec,
    partition_free_bytes_metric: IntGaugeVec,
    partition_state_metric: IntGaugeVec,
    partition_fsck_result_metric: IntGaugeVec,
    partition_operation_progress_metric: IntGaugeVec,
    partition_operation_steps_metric: IntGaugeVec,
}

impl<'a> NasMetricMap<'a> {
    /// Registers its metrics into the default registry, as the tests read it
    #[cfg(test)]
    pub fn new(factory: &'a AuthenticatedHttpClientFactory<'a>, prefix: String) -> Self {
        Self::new_in(
            factory,
            prefix,
            prometheus_exporter::prometheus::default_registry(),
        )
    }

    /// Registers its metrics into `registry`, the registry of its generation of collectors
    pub fn new_in(
        factory: &'a AuthenticatedHttpClientFactory<'a>,
        prefix: String,
        registry: &Registry,
    ) -> Self {
        Self {
            factory,
            managed_client: None,

            // Disk metrics with labels for disk identification
            disk_total_bytes_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_disk_total_bytes"),
                format!("{prefix}_nas_disk_total_bytes Total disk size in bytes"),
                &["disk_id", "disk_type", "model", "serial"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_total_bytes gauge: {e}")),

            disk_state_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_disk_state"),
                format!("{prefix}_nas_disk_state Disk state (1=enabled, 0=disabled, -1=error, 2=formatting)"),
                &["disk_id", "disk_type", "model", "serial", "state"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_state gauge: {e}")),

            disk_temperature_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_disk_temperature_celsius"),
                format!("{prefix}_nas_disk_temperature_celsius Disk temperature in Celsius"),
                &["disk_id", "disk_type", "model", "serial"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_temperature_celsius gauge: {e}")),

            disk_spinning_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_disk_spinning"),
                format!("{prefix}_nas_disk_spinning Disk spinning status (1=spinning, 0=not spinning)"),
                &["disk_id", "disk_type", "model", "serial"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_spinning gauge: {e}")),

            disk_idle_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_disk_idle"),
                format!("{prefix}_nas_disk_idle Disk idle status (1=idle, 0=active)"),
                &["disk_id", "disk_type", "model", "serial"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_idle gauge: {e}")),

            disk_idle_duration_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_disk_idle_duration_seconds"),
                format!("{prefix}_nas_disk_idle_duration_seconds Disk idle duration in seconds"),
                &["disk_id", "disk_type", "model", "serial"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_idle_duration_seconds gauge: {e}")),

            disk_active_duration_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_disk_active_duration_seconds"),
                format!("{prefix}_nas_disk_active_duration_seconds Disk active duration in seconds"),
                &["disk_id", "disk_type", "model", "serial"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_active_duration_seconds gauge: {e}")),

            disk_time_before_spindown_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_disk_time_before_spindown_seconds"),
                format!("{prefix}_nas_disk_time_before_spindown_seconds Time before disk spindown in seconds"),
                &["disk_id", "disk_type", "model", "serial"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_time_before_spindown_seconds gauge: {e}")),

            disk_connector_info_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_disk_connector_info"),
                format!("{prefix}_nas_disk_connector_info Disk physical connector ID"),
                &["disk_id", "disk_type", "model", "serial", "connector"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_connector_info gauge: {e}")),

            disk_table_type_info_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_disk_table_type_info"),
                format!("{prefix}_nas_disk_table_type_info Disk partition table type"),
                &["disk_id", "disk_type", "model", "serial", "table_type"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_table_type_info gauge: {e}")),

            disk_firmware_info_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_disk_firmware_info"),
                format!("{prefix}_nas_disk_firmware_info Disk firmware version"),
                &["disk_id", "disk_type", "model", "serial", "firmware"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_firmware_info gauge: {e}")),

            disk_operation_progress_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_disk_operation_progress_percent"),
                format!("{prefix}_nas_disk_operation_progress_percent Disk operation progress percentage"),
                &["disk_id", "disk_type", "model", "serial"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_operation_progress_percent gauge: {e}")),

            disk_operation_steps_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_disk_operation_steps"),
                format!("{prefix}_nas_disk_operation_steps Disk operation steps (done/total)"),
                &["disk_id", "disk_type", "model", "serial", "step_type"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_operation_steps gauge: {e}")),

            disk_read_requests_metric: register_int_counter_vec_with_registry!(
                format!("{prefix}_nas_disk_read_requests_total"),
                format!("{prefix}_nas_disk_read_requests_total Read requests counted by the box since its start"),
                &["disk_id", "disk_type", "model", "serial"],
                registry
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_read_requests_total counter: {e}")),

            disk_read_error_requests_metric: register_int_counter_vec_with_registry!(
                format!("{prefix}_nas_disk_read_error_requests_total"),
                format!("{prefix}_nas_disk_read_error_requests_total Failed read requests counted by the box since its start"),
                &["disk_id", "disk_type", "model", "serial"],
                registry
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_read_error_requests_total counter: {e}")),

            disk_write_requests_metric: register_int_counter_vec_with_registry!(
                format!("{prefix}_nas_disk_write_requests_total"),
                format!("{prefix}_nas_disk_write_requests_total Write requests counted by the box since its start"),
                &["disk_id", "disk_type", "model", "serial"],
                registry
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_write_requests_total counter: {e}")),

            disk_write_error_requests_metric: register_int_counter_vec_with_registry!(
                format!("{prefix}_nas_disk_write_error_requests_total"),
                format!("{prefix}_nas_disk_write_error_requests_total Failed write requests counted by the box since its start"),
                &["disk_id", "disk_type", "model", "serial"],
                registry
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_disk_write_error_requests_total counter: {e}")),

            // Partition metrics with labels for partition identification
            partition_total_bytes_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_partition_total_bytes"),
                format!("{prefix}_nas_partition_total_bytes Partition total size in bytes"),
                &["partition_id", "disk_id", "label", "fstype"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_partition_total_bytes gauge: {e}")),

            partition_used_bytes_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_partition_used_bytes"),
                format!("{prefix}_nas_partition_used_bytes Partition used space in bytes"),
                &["partition_id", "disk_id", "label", "fstype"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_partition_used_bytes gauge: {e}")),

            partition_free_bytes_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_partition_free_bytes"),
                format!("{prefix}_nas_partition_free_bytes Partition free space in bytes"),
                &["partition_id", "disk_id", "label", "fstype"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_partition_free_bytes gauge: {e}")),

            partition_state_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_partition_state"),
                format!("{prefix}_nas_partition_state Partition state (1=mounted, 0=unmounted, -1=error, 2=checking, 3=formatting, 4=mounting, 5=maintenance, 6=umounting, 7=ejecting)"),
                &["partition_id", "disk_id", "label", "fstype", "state"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_partition_state gauge: {e}")),

            partition_fsck_result_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_partition_fsck_result"),
                format!("{prefix}_nas_partition_fsck_result File system check result (0=no_run_yet, 1=running, 2=fs_clean, 3=fs_corrected, 4=fs_needs_correction, -1=failed)"),
                &["partition_id", "disk_id", "label", "fstype", "fsck_result"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_partition_fsck_result gauge: {e}")),

            partition_operation_progress_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_partition_operation_progress_percent"),
                format!("{prefix}_nas_partition_operation_progress_percent Partition operation progress percentage"),
                &["partition_id", "disk_id", "label", "fstype"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_partition_operation_progress_percent gauge: {e}")),

            partition_operation_steps_metric: register_int_gauge_vec_with_registry!(
                format!("{prefix}_nas_partition_operation_steps"),
                format!("{prefix}_nas_partition_operation_steps Partition operation steps (done/total)"),
                &["partition_id", "disk_id", "label", "fstype", "step_type"],
                registry,
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_nas_partition_operation_steps gauge: {e}")),
        }
    }

    async fn get_managed_client(
        &mut self,
    ) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
        if self.managed_client.as_ref().is_none() {
            debug!("creating managed client");

            let res = self.factory.create_managed_client().await;

            if res.is_err() {
                debug!("cannot create managed client");
                return Err(res.err().unwrap());
            }

            self.managed_client = Some(res.unwrap());
        }

        let client = self.managed_client.as_ref().unwrap();
        if let Ok(client) = client.get() {
            return Ok(client);
        }

        debug!("renewing managed client");

        let client = self.factory.create_managed_client().await;
        self.managed_client = Some(client?);

        self.managed_client.as_ref().unwrap().get()
    }

    fn reset_all(&mut self) {
        self.disk_total_bytes_metric.reset();
        self.disk_state_metric.reset();
        self.disk_temperature_metric.reset();
        self.disk_spinning_metric.reset();
        self.disk_idle_metric.reset();
        self.disk_idle_duration_metric.reset();
        self.disk_active_duration_metric.reset();
        self.disk_time_before_spindown_metric.reset();
        self.disk_connector_info_metric.reset();
        self.disk_table_type_info_metric.reset();
        self.disk_firmware_info_metric.reset();
        self.disk_operation_progress_metric.reset();
        self.disk_operation_steps_metric.reset();
        self.disk_read_requests_metric.reset();
        self.disk_read_error_requests_metric.reset();
        self.disk_write_requests_metric.reset();
        self.disk_write_error_requests_metric.reset();
        self.partition_total_bytes_metric.reset();
        self.partition_used_bytes_metric.reset();
        self.partition_free_bytes_metric.reset();
        self.partition_state_metric.reset();
        self.partition_fsck_result_metric.reset();
        self.partition_operation_progress_metric.reset();
        self.partition_operation_steps_metric.reset();
    }

    async fn set_storage_metrics(
        &mut self,
        lock: &UpdateLock,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching storage disks");

        let client = self.get_managed_client().await?;
        let disks: Vec<StorageDisk> =
            get_required_api_result(&client, &self.factory.api_url, "v4/storage/disk").await?;

        // reset only once the response is valid: a scrape during the request still sees
        // the previous collection, and a failed request keeps it
        // a scrape waits for the update to complete instead of returning part of it
        let _update = lock.lock();
        self.reset_all();

        for disk in disks {
            let disk_id = disk.id.to_string();
            let disk_type = disk.disk_type.unwrap_or_else(|| "unknown".to_string());
            let model = disk.model.unwrap_or_else(|| "unknown".to_string());
            let serial = disk.serial.unwrap_or_else(|| "unknown".to_string());

            for (metric, value) in [
                (&self.disk_read_requests_metric, disk.read_requests),
                (
                    &self.disk_read_error_requests_metric,
                    disk.read_error_requests,
                ),
                (&self.disk_write_requests_metric, disk.write_requests),
                (
                    &self.disk_write_error_requests_metric,
                    disk.write_error_requests,
                ),
            ] {
                // a negative value is not a count: left out like a missing one
                if let Some(value) = value.and_then(|v| u64::try_from(v).ok()) {
                    metric
                        .with_label_values(&[&disk_id, &disk_type, &model, &serial])
                        .inc_by(value);
                }
            }

            // Set disk metrics
            if let Some(total_bytes) = disk.total_bytes {
                self.disk_total_bytes_metric
                    .with_label_values(&[&disk_id, &disk_type, &model, &serial])
                    .set(total_bytes);
            }

            // Map disk state to numeric value according to API spec
            let state_value = match disk.state.as_deref() {
                Some("enabled") => 1,
                Some("disabled") => 0,
                Some("error") => -1,
                Some("formatting") => 2,
                _ => 0,
            };
            let state = disk.state.unwrap_or_else(|| "unknown".to_string());
            self.disk_state_metric
                .with_label_values(&[&disk_id, &disk_type, &model, &serial, &state])
                .set(state_value);

            if let Some(temp) = disk.temp {
                self.disk_temperature_metric
                    .with_label_values(&[&disk_id, &disk_type, &model, &serial])
                    .set(temp);
            }

            if let Some(spinning) = disk.spinning {
                self.disk_spinning_metric
                    .with_label_values(&[&disk_id, &disk_type, &model, &serial])
                    .set(if spinning { 1 } else { 0 });
            }

            if let Some(idle) = disk.idle {
                self.disk_idle_metric
                    .with_label_values(&[&disk_id, &disk_type, &model, &serial])
                    .set(if idle { 1 } else { 0 });
            }

            if let Some(idle_duration) = disk.idle_duration {
                self.disk_idle_duration_metric
                    .with_label_values(&[&disk_id, &disk_type, &model, &serial])
                    .set(idle_duration);
            }

            if let Some(active_duration) = disk.active_duration {
                self.disk_active_duration_metric
                    .with_label_values(&[&disk_id, &disk_type, &model, &serial])
                    .set(active_duration);
            }

            if let Some(time_before_spindown) = disk.time_before_spindown {
                self.disk_time_before_spindown_metric
                    .with_label_values(&[&disk_id, &disk_type, &model, &serial])
                    .set(time_before_spindown);
            }

            // Set connector information
            if let Some(connector) = disk.connector {
                self.disk_connector_info_metric
                    .with_label_values(&[
                        &disk_id,
                        &disk_type,
                        &model,
                        &serial,
                        &connector.to_string(),
                    ])
                    .set(1);
            }

            // Set table type information
            if let Some(table_type) = &disk.table_type {
                self.disk_table_type_info_metric
                    .with_label_values(&[&disk_id, &disk_type, &model, &serial, table_type])
                    .set(1);
            }

            // Set firmware information
            if let Some(firmware) = &disk.firmware {
                if !firmware.is_empty() {
                    self.disk_firmware_info_metric
                        .with_label_values(&[&disk_id, &disk_type, &model, &serial, firmware])
                        .set(1);
                }
            }

            // Set disk operation progress
            if let Some(operation_pct) = &disk.operation_pct {
                if let Some(percent) = operation_pct.percent {
                    self.disk_operation_progress_metric
                        .with_label_values(&[&disk_id, &disk_type, &model, &serial])
                        .set(percent);
                }

                // Set disk operation steps (done/total)
                if let Some(done_steps) = operation_pct.done_steps {
                    self.disk_operation_steps_metric
                        .with_label_values(&[&disk_id, &disk_type, &model, &serial, "done"])
                        .set(done_steps);
                }

                if let Some(max_steps) = operation_pct.max_steps {
                    self.disk_operation_steps_metric
                        .with_label_values(&[&disk_id, &disk_type, &model, &serial, "total"])
                        .set(max_steps);
                }
            }

            // Process partitions
            if let Some(partitions) = disk.partitions {
                for partition in partitions {
                    let partition_id = partition.id.to_string();
                    let partition_disk_id = partition.disk_id.to_string();
                    let label = partition.label.unwrap_or_else(|| "unknown".to_string());
                    let fstype = partition.fstype.unwrap_or_else(|| "unknown".to_string());

                    if let Some(total_bytes) = partition.total_bytes {
                        self.partition_total_bytes_metric
                            .with_label_values(&[
                                &partition_id,
                                &partition_disk_id,
                                &label,
                                &fstype,
                            ])
                            .set(total_bytes);
                    }

                    if let Some(used_bytes) = partition.used_bytes {
                        self.partition_used_bytes_metric
                            .with_label_values(&[
                                &partition_id,
                                &partition_disk_id,
                                &label,
                                &fstype,
                            ])
                            .set(used_bytes);
                    }

                    if let Some(free_bytes) = partition.free_bytes {
                        self.partition_free_bytes_metric
                            .with_label_values(&[
                                &partition_id,
                                &partition_disk_id,
                                &label,
                                &fstype,
                            ])
                            .set(free_bytes);
                    }

                    // Map partition state to numeric value according to API spec
                    let partition_state_value = match partition.state.as_deref() {
                        Some("mounted") => 1,
                        Some("umounted") => 0,
                        Some("error") => -1,
                        Some("checking") => 2,
                        Some("formatting") => 3,
                        Some("mounting") => 4,
                        Some("maintenance") => 5,
                        Some("umounting") => 6,
                        Some("ejecting") => 7,
                        _ => 0,
                    };
                    let partition_state = partition.state.unwrap_or_else(|| "unknown".to_string());
                    self.partition_state_metric
                        .with_label_values(&[
                            &partition_id,
                            &partition_disk_id,
                            &label,
                            &fstype,
                            &partition_state,
                        ])
                        .set(partition_state_value);

                    // Set fsck result
                    if let Some(fsck_result) = &partition.fsck_result {
                        let fsck_result_value = match fsck_result.as_str() {
                            "no_run_yet" => 0,
                            "running" => 1,
                            "fs_clean" => 2,
                            "fs_corrected" => 3,
                            "fs_needs_correction" => 4,
                            "failed" => -1,
                            _ => 0,
                        };
                        self.partition_fsck_result_metric
                            .with_label_values(&[
                                &partition_id,
                                &partition_disk_id,
                                &label,
                                &fstype,
                                fsck_result,
                            ])
                            .set(fsck_result_value);
                    }

                    // Set partition operation progress
                    if let Some(operation_pct) = &partition.operation_pct {
                        if let Some(percent) = operation_pct.percent {
                            self.partition_operation_progress_metric
                                .with_label_values(&[
                                    &partition_id,
                                    &partition_disk_id,
                                    &label,
                                    &fstype,
                                ])
                                .set(percent);
                        }

                        // Set partition operation steps (done/total)
                        if let Some(done_steps) = operation_pct.done_steps {
                            self.partition_operation_steps_metric
                                .with_label_values(&[
                                    &partition_id,
                                    &partition_disk_id,
                                    &label,
                                    &fstype,
                                    "done",
                                ])
                                .set(done_steps);
                        }

                        if let Some(max_steps) = operation_pct.max_steps {
                            self.partition_operation_steps_metric
                                .with_label_values(&[
                                    &partition_id,
                                    &partition_disk_id,
                                    &label,
                                    &fstype,
                                    "total",
                                ])
                                .set(max_steps);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl<'a> MetricMap<'a> for NasMetricMap<'a> {
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn set(
        &mut self,
        lock: &UpdateLock,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.set_storage_metrics(lock).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_storage_disk_deserialize() {
        // Based on official API documentation example response
        let json_data = r#"{
            "idle_duration": 368,
            "spinning": true,
            "table_type": "msdos",
            "firmware": "PB2ICC0E",
            "type": "internal",
            "idle": true,
            "connector": 0,
            "id": 1,
            "state": "enabled",
            "time_before_spindown": 232,
            "total_bytes": 250059350016,
            "model": "Hitachi HCC545025B9A300",
            "active_duration": 0,
            "temp": 51,
            "serial": "GSCH35VC",
            "partitions": [
                {
                    "fstype": "ext4",
                    "total_bytes": 245091500032,
                    "label": "Disque dur",
                    "id": 3,
                    "fsck_result": "no_run_yet",
                    "state": "mounted",
                    "disk_id": 1,
                    "free_bytes": 68120969216,
                    "used_bytes": 164520534016,
                    "path": "L0Rpc3F1ZSBkdXI="
                }
            ]
        }"#;

        let disk: StorageDisk = serde_json::from_str(json_data).unwrap();

        assert_eq!(disk.id, 1);
        assert_eq!(disk.disk_type.unwrap(), "internal");
        assert_eq!(disk.state.unwrap(), "enabled");
        assert_eq!(disk.total_bytes.unwrap(), 250059350016);
        assert_eq!(disk.temp.unwrap(), 51);
        assert!(disk.idle.unwrap());
        assert!(disk.spinning.unwrap());
        assert_eq!(disk.table_type.unwrap(), "msdos");
        assert_eq!(disk.model.unwrap(), "Hitachi HCC545025B9A300");
        assert_eq!(disk.serial.unwrap(), "GSCH35VC");
        assert_eq!(disk.firmware.unwrap(), "PB2ICC0E");
        assert_eq!(disk.idle_duration.unwrap(), 368);
        assert_eq!(disk.active_duration.unwrap(), 0);
        assert_eq!(disk.time_before_spindown.unwrap(), 232);

        let partitions = disk.partitions.unwrap();
        assert_eq!(partitions.len(), 1);

        let partition = &partitions[0];
        assert_eq!(partition.id, 3);
        assert_eq!(partition.disk_id, 1);
        assert_eq!(partition.state.as_ref().unwrap(), "mounted");
        assert_eq!(partition.fstype.as_ref().unwrap(), "ext4");
        assert_eq!(partition.label.as_ref().unwrap(), "Disque dur");
        assert_eq!(partition.total_bytes.unwrap(), 245091500032);
        assert_eq!(partition.used_bytes.unwrap(), 164520534016);
        assert_eq!(partition.free_bytes.unwrap(), 68120969216);
        assert_eq!(partition.fsck_result.as_ref().unwrap(), "no_run_yet");
    }

    #[test]
    fn test_storage_disk_deserialize_usb() {
        // Based on official API documentation example response
        let json_data = r#"{
            "type": "usb",
            "total_bytes": 125435904,
            "connector": 1,
            "id": 1001,
            "active_duration": 0,
            "partitions": [
                {
                    "fstype": "ext4",
                    "total_bytes": 121418752,
                    "label": "Disque 1",
                    "id": 1002,
                    "fsck_result": "no_run_yet",
                    "state": "mounted",
                    "disk_id": 1001,
                    "free_bytes": 108904448,
                    "used_bytes": 6245376,
                    "path": "L0Rpc3F1ZSAx"
                }
            ],
            "idle_duration": 0,
            "state": "enabled",
            "idle": false,
            "spinning": false,
            "model": "",
            "table_type": "gpt",
            "temp": 0,
            "serial": "",
            "firmware": ""
        }"#;

        let disk: StorageDisk = serde_json::from_str(json_data).unwrap();

        assert_eq!(disk.id, 1001);
        assert_eq!(disk.disk_type.unwrap(), "usb");
        assert_eq!(disk.state.unwrap(), "enabled");
        assert_eq!(disk.total_bytes.unwrap(), 125435904);
        assert_eq!(disk.temp.unwrap(), 0);
        assert!(!disk.idle.unwrap());
        assert!(!disk.spinning.unwrap());
        assert_eq!(disk.table_type.unwrap(), "gpt");
        assert_eq!(disk.model.unwrap(), "");
        assert_eq!(disk.serial.unwrap(), "");
        assert_eq!(disk.firmware.unwrap(), "");
        assert_eq!(disk.idle_duration.unwrap(), 0);
        assert_eq!(disk.active_duration.unwrap(), 0);

        let partitions = disk.partitions.unwrap();
        assert_eq!(partitions.len(), 1);

        let partition = &partitions[0];
        assert_eq!(partition.id, 1002);
        assert_eq!(partition.disk_id, 1001);
        assert_eq!(partition.state.as_ref().unwrap(), "mounted");
        assert_eq!(partition.fstype.as_ref().unwrap(), "ext4");
        assert_eq!(partition.label.as_ref().unwrap(), "Disque 1");
        assert_eq!(partition.total_bytes.unwrap(), 121418752);
        assert_eq!(partition.used_bytes.unwrap(), 6245376);
        assert_eq!(partition.free_bytes.unwrap(), 108904448);
        assert_eq!(partition.fsck_result.as_ref().unwrap(), "no_run_yet");
    }

    #[test]
    fn test_disk_partition_deserialize() {
        // Based on official API documentation example response for partition API
        let json_data = r#"{
            "fstype": "vfat",
            "total_bytes": 123485184,
            "label": "freebox",
            "id": 1002,
            "fsck_result": "no_run_yet",
            "state": "mounted",
            "disk_id": 1001,
            "free_bytes": 123484672,
            "used_bytes": 512,
            "path": "L2ZyZWVib3g="
        }"#;

        let partition: DiskPartition = serde_json::from_str(json_data).unwrap();

        assert_eq!(partition.id, 1002);
        assert_eq!(partition.disk_id, 1001);
        assert_eq!(partition.state.as_ref().unwrap(), "mounted");
        assert_eq!(partition.fstype.as_ref().unwrap(), "vfat");
        assert_eq!(partition.label.as_ref().unwrap(), "freebox");
        assert_eq!(partition.total_bytes.unwrap(), 123485184);
        assert_eq!(partition.used_bytes.unwrap(), 512);
        assert_eq!(partition.free_bytes.unwrap(), 123484672);
        assert_eq!(partition.fsck_result.as_ref().unwrap(), "no_run_yet");
    }
}

#[cfg(test)]
mod tests_collect {
    use serde_json::{json, Value};

    use super::*;
    use crate::mappers::test_support::{
        collect_while_locked, mock_api, new_update_lock, scrape, scrape_during, serve,
        serve_scoped, serve_slowly, update_lock,
    };

    fn disks(total_bytes: i64) -> Value {
        json!({ "success": true, "result": [{ "id": 1, "type": "sata", "total_bytes": total_bytes }] })
    }

    #[tokio::test]
    async fn scrape_during_collection_sees_the_previous_collection() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_nas_scrape";
        let mut map = NasMetricMap::new(&factory, prefix.to_string());
        let total = |lines: &[String]| {
            lines
                .iter()
                .find(|l| l.starts_with(&format!("{prefix}_nas_disk_total_bytes{{")))
                .cloned()
        };

        let first = serve_scoped(&server, "/api/v4/storage/disk", disks(1000)).await;
        map.set(update_lock()).await.unwrap();
        let before = scrape(prefix);
        assert!(
            total(&before).is_some_and(|l| l.ends_with(" 1000")),
            "{before:?}"
        );
        drop(first);

        // the only request of the collection is slow, and returns a new size
        serve_slowly(&server, "/api/v4/storage/disk", disks(2000)).await;
        let (result, during) = scrape_during(
            &server,
            "/api/v4/storage/disk",
            map.set(update_lock()),
            prefix,
        )
        .await;
        result.unwrap();

        assert_eq!(before, during);
        assert!(total(&scrape(prefix)).is_some_and(|l| l.ends_with(" 2000")));
    }

    #[tokio::test]
    async fn collection_waits_for_a_scrape_in_progress() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_nas_lock";
        let mut map = NasMetricMap::new(&factory, prefix.to_string());
        let total = |lines: &[String]| {
            lines
                .iter()
                .find(|l| l.starts_with(&format!("{prefix}_nas_disk_total_bytes{{")))
                .cloned()
        };

        let first = serve_scoped(&server, "/api/v4/storage/disk", disks(1000)).await;
        map.set(update_lock()).await.unwrap();
        let before = scrape(prefix);
        assert!(
            total(&before).is_some_and(|l| l.ends_with(" 1000")),
            "{before:?}"
        );
        drop(first);

        // a scrape holds the lock while the collection gets a new response
        serve(&server, "/api/v4/storage/disk", disks(2000)).await;
        let lock = new_update_lock();
        let (result, during) = collect_while_locked(&lock, map.set(&lock), prefix).await;
        result.unwrap();

        assert_eq!(before, during);
        assert!(total(&scrape(prefix)).is_some_and(|l| l.ends_with(" 2000")));
    }

    /// A disk with the request counters of a Freebox Delta, `None` leaves a counter out
    fn disk_with_requests(id: i64, requests: [Option<i64>; 4]) -> Value {
        let [read, read_errors, write, write_errors] = requests;
        json!({ "id": id, "type": "sata", "model": "ST4000", "serial": "S1",
                "read_requests": read, "read_error_requests": read_errors,
                "write_requests": write, "write_error_requests": write_errors })
    }

    fn disks_with_requests(disks: Vec<Value>) -> Value {
        json!({ "success": true, "result": disks })
    }

    /// Value of the counter `name` of the disk `disk_id`, None without this series
    fn request_counter(prefix: &str, name: &str, disk_id: &str) -> Option<String> {
        let series = format!(
            "{prefix}_nas_disk_{name}_total{{disk_id=\"{disk_id}\",disk_type=\"sata\",model=\"ST4000\",serial=\"S1\"}} "
        );
        scrape(prefix)
            .into_iter()
            .find_map(|l| l.strip_prefix(&series).map(str::to_string))
    }

    #[tokio::test]
    async fn requests_of_a_disk_are_exposed_as_counters() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_nas_requests";
        let mut map = NasMetricMap::new(&factory, prefix.to_string());

        let disk = disk_with_requests(1, [Some(120), Some(1), Some(80), Some(2)]);
        serve(
            &server,
            "/api/v4/storage/disk",
            disks_with_requests(vec![disk]),
        )
        .await;
        map.set(update_lock()).await.unwrap();

        for (name, value) in [
            ("read_requests", "120"),
            ("read_error_requests", "1"),
            ("write_requests", "80"),
            ("write_error_requests", "2"),
        ] {
            assert_eq!(
                request_counter(prefix, name, "1").as_deref(),
                Some(value),
                "{name}"
            );
            let family = prometheus_exporter::prometheus::gather()
                .into_iter()
                .find(|f| f.name() == format!("{prefix}_nas_disk_{name}_total"))
                .unwrap();
            assert_eq!(
                family.get_field_type(),
                prometheus_exporter::prometheus::proto::MetricType::COUNTER
            );
        }
    }

    #[tokio::test]
    async fn a_disk_without_request_counts_has_no_counter() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_nas_no_requests";
        let mut map = NasMetricMap::new(&factory, prefix.to_string());

        // a negative value is not a count
        let disk = disk_with_requests(1, [None, Some(-1), None, None]);
        serve(
            &server,
            "/api/v4/storage/disk",
            disks_with_requests(vec![disk]),
        )
        .await;
        map.set(update_lock()).await.unwrap();

        let metrics = scrape(prefix);
        assert!(
            metrics
                .iter()
                .any(|l| l.starts_with(&format!("{prefix}_nas_disk_state{{"))),
            "the disk is collected: {metrics:?}"
        );
        assert!(
            !metrics.iter().any(|l| l.contains("_requests_total")),
            "{metrics:?}"
        );
    }

    #[tokio::test]
    async fn request_counters_follow_the_box_across_its_restart() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_nas_requests_restart";
        let mut map = NasMetricMap::new(&factory, prefix.to_string());

        // the box counts up, restarts and counts again from zero
        for reads in [100, 150, 30, 40] {
            let disk = disk_with_requests(1, [Some(reads), None, None, None]);
            let _served = serve_scoped(
                &server,
                "/api/v4/storage/disk",
                disks_with_requests(vec![disk]),
            )
            .await;
            map.set(update_lock()).await.unwrap();
            assert_eq!(
                request_counter(prefix, "read_requests", "1"),
                Some(reads.to_string())
            );
        }
    }

    #[tokio::test]
    async fn request_counters_of_a_gone_disk_are_removed() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_nas_requests_gone";
        let mut map = NasMetricMap::new(&factory, prefix.to_string());
        let counts = [Some(10), Some(0), Some(5), Some(0)];

        let both = disks_with_requests(vec![
            disk_with_requests(1, counts),
            disk_with_requests(2, counts),
        ]);
        let served = serve_scoped(&server, "/api/v4/storage/disk", both).await;
        map.set(update_lock()).await.unwrap();
        assert!(request_counter(prefix, "write_requests", "2").is_some());
        drop(served);

        // disk 2 is unplugged, disk 1 no longer reports its writes
        let one = disks_with_requests(vec![disk_with_requests(1, [Some(12), Some(0), None, None])]);
        serve(&server, "/api/v4/storage/disk", one).await;
        map.set(update_lock()).await.unwrap();

        assert_eq!(
            request_counter(prefix, "read_requests", "1").as_deref(),
            Some("12")
        );
        assert_eq!(request_counter(prefix, "write_requests", "1"), None);
        for name in [
            "read_requests",
            "read_error_requests",
            "write_requests",
            "write_error_requests",
        ] {
            assert_eq!(request_counter(prefix, name, "2"), None, "{name}");
        }
    }
}
