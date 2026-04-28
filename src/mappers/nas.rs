use async_trait::async_trait;
use log::debug;
use prometheus_exporter::prometheus::{
    register_int_gauge_vec, IntGaugeVec,
};
use reqwest::Client;
use serde::Deserialize;

use super::MetricMap;
use crate::core::common::{
    http_client_factory::{AuthenticatedHttpClientFactory, ManagedHttpClient},
    transport::{FreeboxResponse, FreeboxResponseError},
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
    pub fn new(factory: &'a AuthenticatedHttpClientFactory<'a>, prefix: String) -> Self {
        Self {
            factory,
            managed_client: None,
            
            // Disk metrics with labels for disk identification
            disk_total_bytes_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_disk_total_bytes"),
                format!("{prefix}_nas_disk_total_bytes Total disk size in bytes"),
                &["disk_id", "disk_type", "model", "serial"]
            )
            .expect(&format!("cannot create {prefix}_nas_disk_total_bytes gauge")),
            
            disk_state_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_disk_state"),
                format!("{prefix}_nas_disk_state Disk state (1=enabled, 0=disabled, -1=error, 2=formatting)"),
                &["disk_id", "disk_type", "model", "serial", "state"]
            )
            .expect(&format!("cannot create {prefix}_nas_disk_state gauge")),
            
            disk_temperature_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_disk_temperature_celsius"),
                format!("{prefix}_nas_disk_temperature_celsius Disk temperature in Celsius"),
                &["disk_id", "disk_type", "model", "serial"]
            )
            .expect(&format!("cannot create {prefix}_nas_disk_temperature_celsius gauge")),
            
            disk_spinning_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_disk_spinning"),
                format!("{prefix}_nas_disk_spinning Disk spinning status (1=spinning, 0=not spinning)"),
                &["disk_id", "disk_type", "model", "serial"]
            )
            .expect(&format!("cannot create {prefix}_nas_disk_spinning gauge")),
            
            disk_idle_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_disk_idle"),
                format!("{prefix}_nas_disk_idle Disk idle status (1=idle, 0=active)"),
                &["disk_id", "disk_type", "model", "serial"]
            )
            .expect(&format!("cannot create {prefix}_nas_disk_idle gauge")),
            
            disk_idle_duration_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_disk_idle_duration_seconds"),
                format!("{prefix}_nas_disk_idle_duration_seconds Disk idle duration in seconds"),
                &["disk_id", "disk_type", "model", "serial"]
            )
            .expect(&format!("cannot create {prefix}_nas_disk_idle_duration_seconds gauge")),
            
            disk_active_duration_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_disk_active_duration_seconds"),
                format!("{prefix}_nas_disk_active_duration_seconds Disk active duration in seconds"),
                &["disk_id", "disk_type", "model", "serial"]
            )
            .expect(&format!("cannot create {prefix}_nas_disk_active_duration_seconds gauge")),
            
            disk_time_before_spindown_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_disk_time_before_spindown_seconds"),
                format!("{prefix}_nas_disk_time_before_spindown_seconds Time before disk spindown in seconds"),
                &["disk_id", "disk_type", "model", "serial"]
            )
            .expect(&format!("cannot create {prefix}_nas_disk_time_before_spindown_seconds gauge")),
            
            disk_connector_info_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_disk_connector_info"),
                format!("{prefix}_nas_disk_connector_info Disk physical connector ID"),
                &["disk_id", "disk_type", "model", "serial", "connector"]
            )
            .expect(&format!("cannot create {prefix}_nas_disk_connector_info gauge")),
            
            disk_table_type_info_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_disk_table_type_info"),
                format!("{prefix}_nas_disk_table_type_info Disk partition table type"),
                &["disk_id", "disk_type", "model", "serial", "table_type"]
            )
            .expect(&format!("cannot create {prefix}_nas_disk_table_type_info gauge")),
            
            disk_firmware_info_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_disk_firmware_info"),
                format!("{prefix}_nas_disk_firmware_info Disk firmware version"),
                &["disk_id", "disk_type", "model", "serial", "firmware"]
            )
            .expect(&format!("cannot create {prefix}_nas_disk_firmware_info gauge")),
            
            disk_operation_progress_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_disk_operation_progress_percent"),
                format!("{prefix}_nas_disk_operation_progress_percent Disk operation progress percentage"),
                &["disk_id", "disk_type", "model", "serial"]
            )
            .expect(&format!("cannot create {prefix}_nas_disk_operation_progress_percent gauge")),
            
            disk_operation_steps_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_disk_operation_steps"),
                format!("{prefix}_nas_disk_operation_steps Disk operation steps (done/total)"),
                &["disk_id", "disk_type", "model", "serial", "step_type"]
            )
            .expect(&format!("cannot create {prefix}_nas_disk_operation_steps gauge")),
            
            // Partition metrics with labels for partition identification
            partition_total_bytes_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_partition_total_bytes"),
                format!("{prefix}_nas_partition_total_bytes Partition total size in bytes"),
                &["partition_id", "disk_id", "label", "fstype"]
            )
            .expect(&format!("cannot create {prefix}_nas_partition_total_bytes gauge")),
            
            partition_used_bytes_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_partition_used_bytes"),
                format!("{prefix}_nas_partition_used_bytes Partition used space in bytes"),
                &["partition_id", "disk_id", "label", "fstype"]
            )
            .expect(&format!("cannot create {prefix}_nas_partition_used_bytes gauge")),
            
            partition_free_bytes_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_partition_free_bytes"),
                format!("{prefix}_nas_partition_free_bytes Partition free space in bytes"),
                &["partition_id", "disk_id", "label", "fstype"]
            )
            .expect(&format!("cannot create {prefix}_nas_partition_free_bytes gauge")),
            
            partition_state_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_partition_state"),
                format!("{prefix}_nas_partition_state Partition state (1=mounted, 0=unmounted, -1=error, 2=checking, 3=formatting, 4=mounting, 5=maintenance, 6=umounting, 7=ejecting)"),
                &["partition_id", "disk_id", "label", "fstype", "state"]
            )
            .expect(&format!("cannot create {prefix}_nas_partition_state gauge")),
            
            partition_fsck_result_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_partition_fsck_result"),
                format!("{prefix}_nas_partition_fsck_result File system check result (0=no_run_yet, 1=running, 2=fs_clean, 3=fs_corrected, 4=fs_needs_correction, -1=failed)"),
                &["partition_id", "disk_id", "label", "fstype", "fsck_result"]
            )
            .expect(&format!("cannot create {prefix}_nas_partition_fsck_result gauge")),
            
            partition_operation_progress_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_partition_operation_progress_percent"),
                format!("{prefix}_nas_partition_operation_progress_percent Partition operation progress percentage"),
                &["partition_id", "disk_id", "label", "fstype"]
            )
            .expect(&format!("cannot create {prefix}_nas_partition_operation_progress_percent gauge")),
            
            partition_operation_steps_metric: register_int_gauge_vec!(
                format!("{prefix}_nas_partition_operation_steps"),
                format!("{prefix}_nas_partition_operation_steps Partition operation steps (done/total)"),
                &["partition_id", "disk_id", "label", "fstype", "step_type"]
            )
            .expect(&format!("cannot create {prefix}_nas_partition_operation_steps gauge")),
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

        let client = self.managed_client.as_ref().clone().unwrap();
        let res = client.get();

        if res.is_ok() {
            return Ok(res.unwrap());
        } else {
            debug!("renewing managed client");

            let client = self.factory.create_managed_client().await;
            self.managed_client = Some(client.unwrap());

            return self.managed_client.as_ref().unwrap().get();
        }
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
        self.partition_total_bytes_metric.reset();
        self.partition_used_bytes_metric.reset();
        self.partition_free_bytes_metric.reset();
        self.partition_state_metric.reset();
        self.partition_fsck_result_metric.reset();
        self.partition_operation_progress_metric.reset();
        self.partition_operation_steps_metric.reset();
    }

    async fn set_storage_metrics(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching storage disks");

        let body = self
            .get_managed_client()
            .await
            .unwrap()
            .get(format!("{}v4/storage/disk", self.factory.api_url))
            .send()
            .await?
            .text()
            .await?;

        let res = match serde_json::from_str::<FreeboxResponse<Vec<StorageDisk>>>(&body) {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        let disks: Vec<StorageDisk> = match res.result {
            None => {
                return Err(Box::new(FreeboxResponseError::new(
                    "v4/storage/disk response was empty".to_string(),
                )))
            }
            Some(r) => r,
        };

        for disk in disks {
            let disk_id = disk.id.to_string();
            let disk_type = disk.disk_type.unwrap_or_else(|| "unknown".to_string());
            let model = disk.model.unwrap_or_else(|| "unknown".to_string());
            let serial = disk.serial.unwrap_or_else(|| "unknown".to_string());
            
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
                    .with_label_values(&[&disk_id, &disk_type, &model, &serial, &connector.to_string()])
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
                            .with_label_values(&[&partition_id, &partition_disk_id, &label, &fstype])
                            .set(total_bytes);
                    }

                    if let Some(used_bytes) = partition.used_bytes {
                        self.partition_used_bytes_metric
                            .with_label_values(&[&partition_id, &partition_disk_id, &label, &fstype])
                            .set(used_bytes);
                    }

                    if let Some(free_bytes) = partition.free_bytes {
                        self.partition_free_bytes_metric
                            .with_label_values(&[&partition_id, &partition_disk_id, &label, &fstype])
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
                        .with_label_values(&[&partition_id, &partition_disk_id, &label, &fstype, &partition_state])
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
                            .with_label_values(&[&partition_id, &partition_disk_id, &label, &fstype, fsck_result])
                            .set(fsck_result_value);
                    }

                    // Set partition operation progress
                    if let Some(operation_pct) = &partition.operation_pct {
                        if let Some(percent) = operation_pct.percent {
                            self.partition_operation_progress_metric
                                .with_label_values(&[&partition_id, &partition_disk_id, &label, &fstype])
                                .set(percent);
                        }
                        
                        // Set partition operation steps (done/total)
                        if let Some(done_steps) = operation_pct.done_steps {
                            self.partition_operation_steps_metric
                                .with_label_values(&[&partition_id, &partition_disk_id, &label, &fstype, "done"])
                                .set(done_steps);
                        }
                        
                        if let Some(max_steps) = operation_pct.max_steps {
                            self.partition_operation_steps_metric
                                .with_label_values(&[&partition_id, &partition_disk_id, &label, &fstype, "total"])
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

    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.reset_all();

        match self.set_storage_metrics().await {
            Err(e) => return Err(e),
            _ => {}
        };
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
        assert_eq!(disk.idle.unwrap(), true);
        assert_eq!(disk.spinning.unwrap(), true);
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
        assert_eq!(disk.idle.unwrap(), false);
        assert_eq!(disk.spinning.unwrap(), false);
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