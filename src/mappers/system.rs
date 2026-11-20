use std::collections::BTreeMap;

use async_trait::async_trait;
use log::debug;
use prometheus_exporter::prometheus::{
    register_int_gauge, register_int_gauge_vec, IntGauge, IntGaugeVec,
};
use prometheus_exporter::UpdateLock;
use reqwest::Client;
use serde::Deserialize;

use super::MetricMap;
use crate::core::common::{
    http_client_factory::{AuthenticatedHttpClientFactory, ManagedHttpClient},
    transport::{get_api_result, get_required_api_result},
};

/// First API version whose system/ lists every fan and every temperature sensor, in
/// fans and sensors: v4 and v5 only have fan_rpm and the temp_* fields, measured on a
/// Freebox Delta
pub const V6_SYSTEM_API_VERSION: u32 = 6;

/// (id, name, value) of a fan or a temperature sensor
type Labeled = (String, String, i64);

/// An entry of the fans or sensors list of v6/system/
#[derive(Deserialize, Clone, Debug)]
struct Measure {
    id: Option<String>,
    name: Option<String>,
    value: Option<i64>,
}

impl Measure {
    /// (id, name, value), the name defaults to the id. None without an id or a value
    fn labeled(self) -> Option<Labeled> {
        let (id, value) = (self.id?, self.value?);
        let name = self.name.unwrap_or_else(|| id.clone());
        Some((id, name, value))
    }
}

/// The part of v6/system/ missing from v4/system
#[derive(Deserialize, Clone, Debug, Default)]
struct V6System {
    fans: Option<Vec<Measure>>,
    sensors: Option<Vec<Measure>>,
}

/// The model of the box, from box_model and box_model_name of /api_version,
/// e.g. "fbxgw7-r1/full" and "Freebox v7 (r1)"
#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
pub struct BoxModel {
    pub model: Option<String>,
    pub model_name: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SystemConfig {
    pub mac: Option<String>,
    pub box_flavor: Option<String>,
    pub box_model_name: Option<String>,
    pub device_name: Option<String>,
    pub api_version: Option<String>,
    pub temp_hdd: Option<i64>,
    // Legacy CPU temperature format (older Freebox models)
    pub temp_cpub: Option<i64>,
    pub temp_cpum: Option<i64>,
    // Additional temperature sensors found in real Freebox data
    pub temp_t1: Option<i64>,
    pub temp_t2: Option<i64>,
    // New CPU temperature format (Freebox Ultra)
    pub temp_cpu0: Option<i64>,
    pub temp_cpu1: Option<i64>,
    pub temp_cpu2: Option<i64>,
    pub temp_cpu3: Option<i64>,
    pub disk_status: Option<String>,
    pub box_authenticated: Option<bool>,
    pub board_name: Option<String>,
    pub fan_rpm: Option<i64>,
    pub temp_sw: Option<i64>,
    pub uptime_val: Option<i64>,
    pub user_main_storage: Option<String>,
    pub serial: Option<String>,
    pub firmware_version: Option<String>,
    /// Fields without a dedicated member, such as the temperature sensors of a Freebox
    /// Delta (temp_cpu_cp_master, temp_cpu_ap, temp_hdd0, ...)
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_json::Value>,
}

impl SystemConfig {
    /// Every temperature reported by the box, as (sensor, celsius) where sensor is the
    /// field name without its "temp_" prefix. The fields and their meaning depend on the
    /// model, e.g. the CPU temperatures of a Delta are only in cpu_cp_master,
    /// cpu_cp_slave and cpu_ap
    pub fn temperatures(&self) -> Vec<(String, i64)> {
        let known = [
            ("temp_hdd", self.temp_hdd),
            ("temp_cpub", self.temp_cpub),
            ("temp_cpum", self.temp_cpum),
            ("temp_t1", self.temp_t1),
            ("temp_t2", self.temp_t2),
            ("temp_cpu0", self.temp_cpu0),
            ("temp_cpu1", self.temp_cpu1),
            ("temp_cpu2", self.temp_cpu2),
            ("temp_cpu3", self.temp_cpu3),
            ("temp_sw", self.temp_sw),
        ];
        let others = self
            .other
            .iter()
            .filter(|(name, _)| name.starts_with("temp_"))
            .map(|(name, value)| (name.as_str(), value.as_i64()));

        let mut temperatures: Vec<(String, i64)> = known
            .into_iter()
            .chain(others)
            .filter_map(|(name, value)| {
                value.map(|v| (name.trim_start_matches("temp_").to_string(), v))
            })
            .collect();
        temperatures.sort();
        temperatures
    }
}

pub struct SystemMetricMap<'a> {
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    managed_client: Option<ManagedHttpClient>,
    /// Whether the box lists its fans and sensors, see V6_SYSTEM_API_VERSION
    v6_system: bool,
    /// Read once at startup, None when the box does not tell
    model: Option<BoxModel>,
    fan_speed_metric: IntGaugeVec,
    mac_metric: IntGaugeVec,
    box_flavor_metric: IntGaugeVec,
    box_model_name_metric: IntGaugeVec,
    device_name_metric: IntGaugeVec,
    api_version_metric: IntGaugeVec,
    temp_hdd_metric: IntGauge,
    temp_t1_metric: IntGauge,
    temp_t2_metric: IntGauge,
    temp_cpu_metric: IntGaugeVec, // New: CPU temperature with core label
    temperature_metric: IntGaugeVec,
    // Legacy metrics for backward compatibility
    temp_cpub_metric: IntGauge,
    temp_cpum_metric: IntGauge,
    disk_status_metric: IntGaugeVec,
    box_authenticated_metric: IntGauge,
    board_name_metric: IntGaugeVec,
    model_info_metric: IntGaugeVec,
    fan_rpm_metric: IntGauge,
    temp_sw_metric: IntGauge,
    uptime_val_metric: IntGauge,
    user_main_storage_metric: IntGaugeVec,
    serial_metric: IntGaugeVec,
    firmware_version_metric: IntGaugeVec,
}

impl<'a> SystemMetricMap<'a> {
    pub fn new(factory: &'a AuthenticatedHttpClientFactory<'a>, prefix: String) -> Self {
        Self {
            factory,
            managed_client: None,
            v6_system: false,
            model: None,
            fan_speed_metric: register_int_gauge_vec!(
                format!("{prefix}_system_fan_speed_rpm"),
                "speed of each fan of the box",
                &["id", "name"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_fan_speed_rpm gauge: {e}")),
            mac_metric: register_int_gauge_vec!(
                format!("{prefix}_system_mac"),
                format!("{prefix}_system_mac"),
                &["mac"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_mac gauge: {e}")),
            box_flavor_metric: register_int_gauge_vec!(
                format!("{prefix}_system_box_flavor"),
                format!("{prefix}_system_box_flavor"),
                &["box_flavor"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_box_flavor gauge: {e}")),
            box_model_name_metric: register_int_gauge_vec!(
                format!("{prefix}_system_box_model_name"),
                format!("{prefix}_system_box_model_name"),
                &["model_name"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_box_model_name gauge: {e}")),
            device_name_metric: register_int_gauge_vec!(
                format!("{prefix}_system_device_name"),
                format!("{prefix}_system_device_name"),
                &["device_name"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_device_name gauge: {e}")),
            api_version_metric: register_int_gauge_vec!(
                format!("{prefix}_system_api_version"),
                format!("{prefix}_system_api_version"),
                &["api_version"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_api_version gauge: {e}")),
            temp_hdd_metric: register_int_gauge!(
                format!("{prefix}_system_temp_hdd"),
                format!("{prefix}_system_temp_hdd")
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_temp_hdd gauge: {e}")),
            temp_t1_metric: register_int_gauge!(
                format!("{prefix}_system_temp_t1"),
                format!("{prefix}_system_temp_t1")
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_temp_t1 gauge: {e}")),
            temp_t2_metric: register_int_gauge!(
                format!("{prefix}_system_temp_t2"),
                format!("{prefix}_system_temp_t2")
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_temp_t2 gauge: {e}")),
            temp_cpu_metric: register_int_gauge_vec!(
                format!("{prefix}_system_temp_cpu"),
                format!("{prefix}_system_temp_cpu CPU core temperature"),
                &["core"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_temp_cpu gauge: {e}")),
            temperature_metric: register_int_gauge_vec!(
                format!("{prefix}_system_temperature_celsius"),
                "temperature of each sensor reported by the box",
                &["sensor", "name"]
            )
            .unwrap_or_else(|e| {
                panic!("cannot create {prefix}_system_temperature_celsius gauge: {e}")
            }),
            // Legacy metrics for backward compatibility
            temp_cpub_metric: register_int_gauge!(
                format!("{prefix}_system_temp_cpub"),
                format!("{prefix}_system_temp_cpub")
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_temp_cpub gauge: {e}")),
            temp_cpum_metric: register_int_gauge!(
                format!("{prefix}_system_temp_cpum"),
                format!("{prefix}_system_temp_cpum")
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_temp_cpum gauge: {e}")),
            disk_status_metric: register_int_gauge_vec!(
                format!("{prefix}_system_disk_status"),
                format!("{prefix}_system_disk_status"),
                &["disk_status"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_disk_status gauge: {e}")),
            box_authenticated_metric: register_int_gauge!(
                format!("{prefix}_system_box_authenticated"),
                format!("{prefix}_system_box_authenticated")
            )
            .unwrap_or_else(|e| {
                panic!("cannot create {prefix}_system_box_authenticated gauge: {e}")
            }),
            board_name_metric: register_int_gauge_vec!(
                format!("{prefix}_system_board_name"),
                format!("{prefix}_system_board_name"),
                &["board_name"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_board_name gauge: {e}")),
            model_info_metric: register_int_gauge_vec!(
                format!("{prefix}_system_model_info"),
                "model of the box, from /api_version",
                &["model", "model_name"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_model_info gauge: {e}")),
            fan_rpm_metric: register_int_gauge!(
                format!("{prefix}_system_fan_rpm"),
                format!("{prefix}_system_fan_rpm")
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_fan_rpm gauge: {e}")),
            temp_sw_metric: register_int_gauge!(
                format!("{prefix}_system_temp_sw"),
                format!("{prefix}_system_temp_sw")
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_temp_sw gauge: {e}")),
            uptime_val_metric: register_int_gauge!(
                format!("{prefix}_system_uptime_val"),
                format!("{prefix}_system_uptime_val")
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_uptime_val gauge: {e}")),
            user_main_storage_metric: register_int_gauge_vec!(
                format!("{prefix}_system_user_main_storage"),
                format!("{prefix}_system_user_main_storage"),
                &["user_main_storage"]
            )
            .unwrap_or_else(|e| {
                panic!("cannot create {prefix}_system_user_main_storage gauge: {e}")
            }),
            serial_metric: register_int_gauge_vec!(
                format!("{prefix}_system_serial"),
                format!("{prefix}_system_serial"),
                &["serial"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prefix}_system_serial gauge: {e}")),
            firmware_version_metric: register_int_gauge_vec!(
                format!("{prefix}_system_firmware_version"),
                format!("{prefix}_system_firmware_version"),
                &["firmware_version"]
            )
            .unwrap_or_else(|e| {
                panic!("cannot create {prefix}_system_firmware_version gauge: {e}")
            }),
        }
    }

    /// Also reads the fans and the temperature sensors from v6/system/, for a box
    /// supporting V6_SYSTEM_API_VERSION. Otherwise, the temperatures come from the temp_*
    /// fields of v4/system and there is no fan list
    pub fn with_v6_system(mut self, v6_system: bool) -> Self {
        self.v6_system = v6_system;
        self
    }

    /// The model of the box, read at startup
    pub fn with_model(mut self, model: Option<BoxModel>) -> Self {
        self.model = model;
        self
    }

    async fn get_v6_system(
        &mut self,
    ) -> Result<V6System, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_managed_client().await?;
        let v6 = get_api_result::<V6System>(
            &client,
            &self.factory.api_url,
            &format!("v{V6_SYSTEM_API_VERSION}/system/"),
        )
        .await?
        .unwrap_or_default();
        Ok(v6)
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
        self.fan_speed_metric.reset();
        self.model_info_metric.reset();
        self.mac_metric.reset();
        self.box_flavor_metric.reset();
        self.box_model_name_metric.reset();
        self.device_name_metric.reset();
        self.api_version_metric.reset();
        self.temp_hdd_metric.set(0);
        self.temp_t1_metric.set(0);
        self.temp_t2_metric.set(0);
        self.temp_cpu_metric.reset();
        self.temperature_metric.reset();
        // Legacy metrics for backward compatibility
        self.temp_cpub_metric.set(0);
        self.temp_cpum_metric.set(0);
        self.disk_status_metric.reset();
        self.box_authenticated_metric.set(0);
        self.board_name_metric.reset();
        self.fan_rpm_metric.set(0);
        self.temp_sw_metric.set(0);
        self.uptime_val_metric.set(0);
        self.user_main_storage_metric.reset();
        self.serial_metric.reset();
        self.firmware_version_metric.reset();
    }

    async fn set_system_config(
        &mut self,
        lock: &UpdateLock,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching system config");

        let client = self.get_managed_client().await?;
        let sys_cnf: SystemConfig =
            get_required_api_result(&client, &self.factory.api_url, "v4/system").await?;

        // (sensor, name, celsius) and (id, name, rpm)
        // the source does not change between collections, as the sensor labels depend on it:
        // a failure of v6/system/ fails the collection like a failure of v4/system, with no
        // fallback to the temp_* fields of v4/system
        let (temperatures, fans): (Vec<Labeled>, Vec<Labeled>) = if self.v6_system {
            let v6 = self.get_v6_system().await?;
            (
                v6.sensors
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(Measure::labeled)
                    .map(|(id, name, celsius)| {
                        (id.trim_start_matches("temp_").to_string(), name, celsius)
                    })
                    .collect(),
                v6.fans
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(Measure::labeled)
                    .collect(),
            )
        } else {
            let temperatures = sys_cnf
                .temperatures()
                .into_iter()
                .map(|(sensor, celsius)| (sensor.clone(), sensor, celsius))
                .collect();
            (temperatures, vec![])
        };

        // reset only once the response is valid: a scrape during the request still sees
        // the previous collection, and a failed request keeps it
        // a scrape waits for the update to complete instead of returning part of it
        let _update = lock.lock();
        self.reset_all();

        self.mac_metric
            .with_label_values(&[&sys_cnf.mac.clone().unwrap_or_default()])
            .set(1);
        self.box_flavor_metric
            .with_label_values(&[&sys_cnf.box_flavor.clone().unwrap_or_default()])
            .set(1);

        // Set new metrics if available
        if let Some(model_name) = &sys_cnf.box_model_name {
            self.box_model_name_metric
                .with_label_values(&[model_name])
                .set(1);
        }

        if let Some(device_name) = &sys_cnf.device_name {
            self.device_name_metric
                .with_label_values(&[device_name])
                .set(1);
        }

        if let Some(api_version) = &sys_cnf.api_version {
            self.api_version_metric
                .with_label_values(&[api_version])
                .set(1);
        }

        // Set HDD temperature if available
        self.temp_hdd_metric
            .set(sys_cnf.temp_hdd.unwrap_or_default());

        // Set additional temperature sensors if available
        self.temp_t1_metric.set(sys_cnf.temp_t1.unwrap_or_default());
        self.temp_t2_metric.set(sys_cnf.temp_t2.unwrap_or_default());

        // Handle CPU temperatures - support both legacy and new formats
        self.handle_cpu_temperatures(&sys_cnf);
        for (sensor, name, celsius) in temperatures {
            self.temperature_metric
                .with_label_values(&[&sensor, &name])
                .set(celsius);
        }

        self.disk_status_metric
            .with_label_values(&[&sys_cnf.disk_status.clone().unwrap_or_default()])
            .set(sys_cnf.disk_status.is_some().into());
        self.box_authenticated_metric
            .set(sys_cnf.box_authenticated.unwrap_or_default().into());
        self.board_name_metric
            .with_label_values(&[&sys_cnf.board_name.clone().unwrap_or_default()])
            .set(1);
        if let Some(BoxModel { model, model_name }) = &self.model {
            self.model_info_metric
                .with_label_values(&[
                    model.as_deref().unwrap_or_default(),
                    model_name.as_deref().unwrap_or_default(),
                ])
                .set(1);
        }
        self.fan_rpm_metric.set(sys_cnf.fan_rpm.unwrap_or_default());
        for (id, name, rpm) in fans {
            self.fan_speed_metric
                .with_label_values(&[&id, &name])
                .set(rpm);
        }
        self.temp_sw_metric.set(sys_cnf.temp_sw.unwrap_or_default());
        self.uptime_val_metric
            .set(sys_cnf.uptime_val.unwrap_or_default());
        self.user_main_storage_metric
            .with_label_values(&[&sys_cnf.user_main_storage.clone().unwrap_or_default()])
            .set(sys_cnf.user_main_storage.is_some().into());
        self.serial_metric
            .with_label_values(&[&sys_cnf.serial.clone().unwrap_or_default()])
            .set(1);
        self.firmware_version_metric
            .with_label_values(&[&sys_cnf.firmware_version.clone().unwrap_or_default()])
            .set(1);
        Ok(())
    }

    /// Handles CPU temperature metrics for both legacy and new formats
    ///
    /// This function addresses issue #237: CPU properties migrated on Ultra
    ///
    /// Legacy format (older Freebox models):
    /// - temp_cpub: CPU B temperature  
    /// - temp_cpum: CPU M temperature
    ///
    /// New format (Freebox Ultra v9):
    /// - temp_cpu0, temp_cpu1, temp_cpu2, temp_cpu3: Individual core temperatures
    ///
    /// The function provides backward compatibility by:
    /// - Preserving legacy metrics for existing dashboards
    /// - Adding new labeled metrics for better granularity
    /// - Auto-mapping new format to legacy when appropriate
    fn handle_cpu_temperatures(&self, sys_cnf: &SystemConfig) {
        // Check for new format first (Freebox Ultra)
        if sys_cnf.temp_cpu0.is_some()
            || sys_cnf.temp_cpu1.is_some()
            || sys_cnf.temp_cpu2.is_some()
            || sys_cnf.temp_cpu3.is_some()
        {
            // Use new format with labeled metrics
            if let Some(temp) = sys_cnf.temp_cpu0 {
                self.temp_cpu_metric.with_label_values(&["0"]).set(temp);
            }
            if let Some(temp) = sys_cnf.temp_cpu1 {
                self.temp_cpu_metric.with_label_values(&["1"]).set(temp);
            }
            if let Some(temp) = sys_cnf.temp_cpu2 {
                self.temp_cpu_metric.with_label_values(&["2"]).set(temp);
            }
            if let Some(temp) = sys_cnf.temp_cpu3 {
                self.temp_cpu_metric.with_label_values(&["3"]).set(temp);
            }

            // For backward compatibility, also set legacy metrics if we have corresponding cores
            if let Some(temp) = sys_cnf.temp_cpu0 {
                self.temp_cpub_metric.set(temp); // Map cpu0 to cpub
            }
            if let Some(temp) = sys_cnf.temp_cpu1 {
                self.temp_cpum_metric.set(temp); // Map cpu1 to cpum
            }
        } else {
            // Use legacy format (older Freebox models)
            if let Some(temp) = sys_cnf.temp_cpub {
                self.temp_cpub_metric.set(temp);
                // Also populate new labeled metric for consistency
                self.temp_cpu_metric.with_label_values(&["b"]).set(temp);
            }
            if let Some(temp) = sys_cnf.temp_cpum {
                self.temp_cpum_metric.set(temp);
                // Also populate new labeled metric for consistency
                self.temp_cpu_metric.with_label_values(&["m"]).set(temp);
            }
        }
    }
}

#[async_trait]
impl<'a> MetricMap<'a> for SystemMetricMap<'a> {
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn set(
        &mut self,
        lock: &UpdateLock,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.set_system_config(lock).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_system_config_deserialize_legacy_format() {
        let json_data = r#"{
            "mac": "38:12:34:56:78:90",
            "box_flavor": "light",
            "temp_cpub": 45,
            "temp_cpum": 50,
            "disk_status": "active",
            "box_authenticated": true,
            "board_name": "fbxgw6r",
            "fan_rpm": 1200,
            "temp_sw": 35,
            "uptime_val": 86400,
            "user_main_storage": "Disque 1",
            "serial": "123456789",
            "firmware_version": "4.8.12"
        }"#;

        let config: SystemConfig = serde_json::from_str(json_data).unwrap();

        assert_eq!(config.mac.unwrap(), "38:12:34:56:78:90");
        assert_eq!(config.temp_cpub.unwrap(), 45);
        assert_eq!(config.temp_cpum.unwrap(), 50);
        assert_eq!(config.temp_cpu0, None);
        assert_eq!(config.temp_cpu1, None);
        assert_eq!(config.temp_cpu2, None);
        assert_eq!(config.temp_cpu3, None);
    }

    #[test]
    fn test_system_config_deserialize_new_format() {
        let json_data = r#"{
            "mac": "38:12:34:56:78:90",
            "temp_cpu0": 59,
            "temp_cpu1": 59,
            "temp_cpu2": 59,
            "temp_cpu3": 59,
            "box_flavor": "light",
            "fan_rpm": 480,
            "temp_hdd": 47,
            "disk_status": "active",
            "uptime_val": 88107,
            "user_main_storage": "Disque 1",
            "box_authenticated": true,
            "serial": "FB9XXXXXXXXXXXXXX",
            "firmware_version": "4.9.14",
            "box_model_name": "Freebox v9 (r1)",
            "device_name": "Freebox Server",
            "api_version": "15.0",
            "board_name": "fbxgw9r"
        }"#;

        let config: SystemConfig = serde_json::from_str(json_data).unwrap();

        assert_eq!(config.mac.unwrap(), "38:12:34:56:78:90");
        assert_eq!(config.temp_cpu0.unwrap(), 59);
        assert_eq!(config.temp_cpu1.unwrap(), 59);
        assert_eq!(config.temp_cpu2.unwrap(), 59);
        assert_eq!(config.temp_cpu3.unwrap(), 59);
        assert_eq!(config.temp_hdd.unwrap(), 47);
        assert_eq!(config.box_model_name.unwrap(), "Freebox v9 (r1)");
        assert_eq!(config.device_name.unwrap(), "Freebox Server");
        assert_eq!(config.api_version.unwrap(), "15.0");
        assert_eq!(config.temp_cpub, None);
        assert_eq!(config.temp_cpum, None);
    }

    #[test]
    fn test_system_config_deserialize_mixed_format() {
        // Test edge case where both formats could be present
        let json_data = r#"{
            "mac": "38:12:34:56:78:90",
            "temp_cpub": 45,
            "temp_cpum": 50,
            "temp_cpu0": 59,
            "temp_cpu1": 60,
            "box_flavor": "light",
            "disk_status": "active",
            "firmware_version": "4.9.14"
        }"#;

        let config: SystemConfig = serde_json::from_str(json_data).unwrap();

        // Should have both legacy and new format values
        assert_eq!(config.temp_cpub.unwrap(), 45);
        assert_eq!(config.temp_cpum.unwrap(), 50);
        assert_eq!(config.temp_cpu0.unwrap(), 59);
        assert_eq!(config.temp_cpu1.unwrap(), 60);
        assert_eq!(config.temp_cpu2, None);
        assert_eq!(config.temp_cpu3, None);
    }

    #[test]
    fn test_system_config_deserialize_partial_new_format() {
        // Test case where only some new CPU cores are present
        let json_data = r#"{
            "mac": "38:12:34:56:78:90",
            "temp_cpu0": 55,
            "temp_cpu2": 58,
            "box_flavor": "light"
        }"#;

        let config: SystemConfig = serde_json::from_str(json_data).unwrap();

        assert_eq!(config.temp_cpu0.unwrap(), 55);
        assert_eq!(config.temp_cpu1, None);
        assert_eq!(config.temp_cpu2.unwrap(), 58);
        assert_eq!(config.temp_cpu3, None);
    }

    #[test]
    fn test_system_config_real_freebox_ultra_data() {
        // Test with real data from GitHub issue #237
        let json_data = r#"{
            "mac": "38:XX:XX:XX:XX:XX",
            "temp_cpu0": 59,
            "temp_cpu1": 59,
            "box_flavor": "light",
            "fan_rpm": 480,
            "temp_hdd": 47,
            "temp_cpu2": 59,
            "board_name": "fbxgw9r",
            "temp_cpu3": 59,
            "disk_status": "active",
            "uptime": "1 jour 28 minutes 27 secondes",
            "uptime_val": 88107,
            "user_main_storage": "Disque 1",
            "box_authenticated": true,
            "serial": "FB9XXXXXXXXXXXXXX",
            "firmware_version": "4.9.14",
            "box_model_name": "Freebox v9 (r1)",
            "device_name": "Freebox Server",
            "api_version": "15.0"
        }"#;

        let config: SystemConfig = serde_json::from_str(json_data).unwrap();

        // Verify all 4 CPU cores are present with same temperature
        assert_eq!(config.temp_cpu0.unwrap(), 59);
        assert_eq!(config.temp_cpu1.unwrap(), 59);
        assert_eq!(config.temp_cpu2.unwrap(), 59);
        assert_eq!(config.temp_cpu3.unwrap(), 59);

        // Verify new fields specific to Freebox Ultra
        assert_eq!(config.temp_hdd.unwrap(), 47);
        assert_eq!(config.box_model_name.unwrap(), "Freebox v9 (r1)");
        assert_eq!(config.device_name.unwrap(), "Freebox Server");
        assert_eq!(config.api_version.unwrap(), "15.0");
        assert_eq!(config.board_name.unwrap(), "fbxgw9r");
        assert_eq!(config.firmware_version.unwrap(), "4.9.14");

        // Verify legacy fields are not present
        assert_eq!(config.temp_cpub, None);
        assert_eq!(config.temp_cpum, None);
    }

    #[test]
    fn test_system_config_deserialize_minimal() {
        // Test with minimal required fields
        let json_data = r#"{
            "mac": "38:12:34:56:78:90"
        }"#;

        let config: SystemConfig = serde_json::from_str(json_data).unwrap();

        assert_eq!(config.mac.unwrap(), "38:12:34:56:78:90");
        assert_eq!(config.temp_cpub, None);
        assert_eq!(config.temp_cpum, None);
        assert_eq!(config.temp_cpu0, None);
        assert_eq!(config.temp_cpu1, None);
        assert_eq!(config.temp_cpu2, None);
        assert_eq!(config.temp_cpu3, None);
    }

    #[test]
    fn test_system_config_real_freebox_gen8_data() {
        // Test with actual data from real Generation 8 Freebox (legacy format)
        let json_data = r#"{
            "mac": "38:XX:XX:XX:XX:XX",
            "temp_cpub": 62,
            "temp_t1": 59,
            "temp_t2": 46,
            "box_flavor": "full",
            "fan_rpm": 2640,
            "disk_status": "active",
            "uptime": "2 jours 15 heures 42 minutes 18 secondes",
            "uptime_val": 228138,
            "user_main_storage": "Disque 1",
            "box_authenticated": true,
            "serial": "FB8XXXXXXXXXXXXXX",
            "firmware_version": "4.8.15",
            "board_name": "fbxgw8r"
        }"#;

        let config: SystemConfig = serde_json::from_str(json_data).unwrap();

        // Verify Generation 8 uses legacy format
        assert_eq!(config.temp_cpub.unwrap(), 62);
        assert_eq!(config.temp_t1.unwrap(), 59);
        assert_eq!(config.temp_t2.unwrap(), 46);
        assert_eq!(config.fan_rpm.unwrap(), 2640);
        assert_eq!(config.board_name.unwrap(), "fbxgw8r");
        assert_eq!(config.firmware_version.unwrap(), "4.8.15");

        // Verify new CPU format fields are not present in Gen8
        assert_eq!(config.temp_cpu0, None);
        assert_eq!(config.temp_cpu1, None);
        assert_eq!(config.temp_cpu2, None);
        assert_eq!(config.temp_cpu3, None);

        // Verify Ultra-specific fields are not present
        assert_eq!(config.box_model_name, None);
        assert_eq!(config.device_name, None);
        assert_eq!(config.api_version, None);
        assert_eq!(config.temp_hdd, None);
    }

    /// v4/system of a Freebox Delta (fbxgw7r, firmware 4.13.6): the real CPU temperatures
    /// are only in fields without a dedicated member
    pub(super) const DELTA_SYSTEM: &str = r#"{
        "mac": "34:XX:XX:XX:XX:XX",
        "box_flavor": "light",
        "temp_cpub": 65,
        "temp_cpum": 60,
        "temp_sw": 58,
        "temp_cpu_cp_master": 93,
        "temp_cpu_cp_slave": 93,
        "temp_cpu_ap": 84,
        "temp_hdd0": 64,
        "temp_hdd1": 57,
        "temp_hdd2": 64,
        "temp_hdd3": 58,
        "fan_rpm": 2418,
        "board_name": "fbxgw7r",
        "disk_status": "active",
        "uptime_val": 1307,
        "user_main_storage": "Freebox_raid",
        "box_authenticated": true,
        "serial": "FB7XXXXXXXXXXXXXX",
        "firmware_version": "4.13.6"
    }"#;

    #[test]
    fn temperatures_include_the_fields_without_a_dedicated_member() {
        let config: SystemConfig = serde_json::from_str(DELTA_SYSTEM).unwrap();

        let expected: Vec<(String, i64)> = [
            ("cpu_ap", 84),
            ("cpu_cp_master", 93),
            ("cpu_cp_slave", 93),
            ("cpub", 65),
            ("cpum", 60),
            ("hdd0", 64),
            ("hdd1", 57),
            ("hdd2", 64),
            ("hdd3", 58),
            ("sw", 58),
        ]
        .into_iter()
        .map(|(sensor, celsius)| (sensor.to_string(), celsius))
        .collect();
        assert_eq!(expected, config.temperatures());
    }

    #[test]
    fn temperatures_skip_null_and_non_numeric_values() {
        let config: SystemConfig = serde_json::from_str(
            r#"{ "temp_cpum": null, "temp_t1": 59, "temp_unknown": "n/a", "uptime": "1 jour" }"#,
        )
        .unwrap();

        assert_eq!(vec![("t1".to_string(), 59)], config.temperatures());
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

    fn system(uptime_val: i64) -> Value {
        json!({ "success": true, "result": { "mac": "00:00:00:00:00:00", "uptime_val": uptime_val } })
    }

    #[tokio::test]
    async fn scrape_during_collection_sees_the_previous_collection() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_system_scrape";
        let mut map = SystemMetricMap::new(&factory, prefix.to_string());

        let first = serve_scoped(&server, "/api/v4/system", system(100)).await;
        map.set(update_lock()).await.unwrap();
        let before = scrape(prefix);
        assert!(
            before.contains(&format!("{prefix}_system_uptime_val 100")),
            "{before:?}"
        );
        drop(first);

        // the only request of the collection is slow, and returns a new uptime
        serve_slowly(&server, "/api/v4/system", system(200)).await;
        let (result, during) =
            scrape_during(&server, "/api/v4/system", map.set(update_lock()), prefix).await;
        result.unwrap();

        assert_eq!(before, during);
        assert!(scrape(prefix).contains(&format!("{prefix}_system_uptime_val 200")));
    }

    /// v6/system fans of a Freebox Delta, whose v4/system only has fan_rpm
    fn delta_fans() -> Value {
        json!({ "success": true, "result": { "fans": [
            { "id": "fan1_speed", "name": "Ventilateur 2", "value": 3157 },
            { "id": "fan0_speed", "name": "Ventilateur 1", "value": 3062 }
        ] } })
    }

    #[tokio::test]
    async fn set_exposes_every_fan_when_the_box_lists_them() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_system_fans";
        let mut map = SystemMetricMap::new(&factory, prefix.to_string()).with_v6_system(true);

        serve(&server, "/api/v4/system", system(100)).await;
        serve(&server, "/api/v6/system/", delta_fans()).await;
        map.set(update_lock()).await.unwrap();

        let metrics = scrape(prefix);
        for line in [
            "_system_fan_speed_rpm{id=\"fan0_speed\",name=\"Ventilateur 1\"} 3062",
            "_system_fan_speed_rpm{id=\"fan1_speed\",name=\"Ventilateur 2\"} 3157",
        ] {
            assert!(
                metrics.contains(&format!("{prefix}{line}")),
                "{line} in {metrics:?}"
            );
        }
    }

    #[tokio::test]
    async fn failing_v6_system_request_keeps_the_previous_collection() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_system_v6_failure";
        let mut map = SystemMetricMap::new(&factory, prefix.to_string()).with_v6_system(true);

        let first_system = serve_scoped(&server, "/api/v4/system", system(100)).await;
        let first_v6 = serve_scoped(&server, "/api/v6/system/", delta_system(6)).await;
        map.set(update_lock()).await.unwrap();
        let before = scrape(prefix);
        for metric in ["_system_fan_speed_rpm{", "_system_temperature_celsius{"] {
            assert!(
                before.iter().any(|line| line.contains(metric)),
                "{metric} in {before:?}"
            );
        }
        drop((first_system, first_v6));

        // v4/system answers, v6/system/ fails
        serve(&server, "/api/v4/system", system(200)).await;
        serve(
            &server,
            "/api/v6/system/",
            json!({ "success": false, "error_code": "invalid_request", "msg": "nope" }),
        )
        .await;
        assert!(map.set(update_lock()).await.is_err());

        // the collection fails as a whole: neither the fans and the temperatures nor the
        // other metrics are updated, and there is no fallback to v4/system
        assert_eq!(before, scrape(prefix));
    }

    #[tokio::test]
    async fn set_does_not_request_the_fans_by_default() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_system_no_fans";
        let mut map = SystemMetricMap::new(&factory, prefix.to_string());

        serve(&server, "/api/v4/system", system(100)).await;
        wiremock::Mock::given(wiremock::matchers::path("/api/v6/system/"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(delta_fans()))
            .expect(0)
            .mount(&server)
            .await;
        map.set(update_lock()).await.unwrap();

        assert!(!scrape(prefix)
            .iter()
            .any(|line| line.contains("_system_fan_speed_rpm{")));
        server.verify().await;
    }

    #[tokio::test]
    async fn set_exposes_every_temperature_sensor() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_system_temperatures";
        let mut map = SystemMetricMap::new(&factory, prefix.to_string());

        let result: Value = serde_json::from_str(super::tests::DELTA_SYSTEM).unwrap();
        serve(
            &server,
            "/api/v4/system",
            json!({ "success": true, "result": result }),
        )
        .await;
        map.set(update_lock()).await.unwrap();

        let metrics = scrape(prefix);
        for line in [
            "_system_temperature_celsius{name=\"cpu_cp_master\",sensor=\"cpu_cp_master\"} 93",
            "_system_temperature_celsius{name=\"cpu_cp_slave\",sensor=\"cpu_cp_slave\"} 93",
            "_system_temperature_celsius{name=\"cpu_ap\",sensor=\"cpu_ap\"} 84",
            "_system_temperature_celsius{name=\"hdd3\",sensor=\"hdd3\"} 58",
            "_system_temperature_celsius{name=\"cpum\",sensor=\"cpum\"} 60",
        ] {
            assert!(
                metrics.contains(&format!("{prefix}{line}")),
                "{line} in {metrics:?}"
            );
        }
    }

    /// v4/system and v6/system/ of a Freebox Delta (fbxgw7r, firmware 4.13.6), captured
    /// 2 minutes apart
    fn delta_system(version: u32) -> Value {
        let body = match version {
            4 => include_str!("specs-data/system/api_v4_system_delta-get.json"),
            6 => include_str!("specs-data/system/api_v6_system_delta-get.json"),
            _ => unreachable!(),
        };
        serde_json::from_str(body).unwrap()
    }

    #[tokio::test]
    async fn set_exposes_the_v6_sensors_when_the_box_lists_them() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_system_v6_sensors";
        let mut map = SystemMetricMap::new(&factory, prefix.to_string()).with_v6_system(true);

        serve(&server, "/api/v4/system", delta_system(4)).await;
        serve(&server, "/api/v6/system/", delta_system(6)).await;
        map.set(update_lock()).await.unwrap();

        let metrics = scrape(prefix);
        for line in [
            "_system_temperature_celsius{name=\"Température CPU CP Master\",sensor=\"cpu_cp_master\"} 82",
            "_system_temperature_celsius{name=\"Température CPU CP Slave\",sensor=\"cpu_cp_slave\"} 82",
            "_system_temperature_celsius{name=\"Température CPU AP\",sensor=\"cpu_ap\"} 71",
            "_system_temperature_celsius{name=\"Disque dur 1\",sensor=\"hdd0\"} 56",
            "_system_temperature_celsius{name=\"Température 1\",sensor=\"t1\"} 49",
            "_system_temperature_celsius{name=\"Température 3\",sensor=\"t3\"} 48",
            "_system_fan_speed_rpm{id=\"fan0_speed\",name=\"Ventilateur 1\"} 3091",
            "_system_fan_speed_rpm{id=\"fan1_speed\",name=\"Ventilateur 2\"} 3154",
            // the legacy metrics still come from v4/system
            "_system_temp_cpum 49",
            "_system_fan_rpm 3092",
        ] {
            assert!(
                metrics.contains(&format!("{prefix}{line}")),
                "{line} in {metrics:?}"
            );
        }
        let sensors = metrics
            .iter()
            .filter(|line| line.contains("_system_temperature_celsius{"))
            .count();
        assert_eq!(10, sensors, "{metrics:?}");
        // the v4 names of t1, t2 and t3 are not exposed
        assert!(!metrics.iter().any(|line| line.contains("sensor=\"cpum\"")));
    }

    #[tokio::test]
    async fn collection_waits_for_a_scrape_in_progress() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_system_lock";
        let mut map = SystemMetricMap::new(&factory, prefix.to_string());

        let first = serve_scoped(&server, "/api/v4/system", system(100)).await;
        map.set(update_lock()).await.unwrap();
        let before = scrape(prefix);
        assert!(
            before.contains(&format!("{prefix}_system_uptime_val 100")),
            "{before:?}"
        );
        drop(first);

        // a scrape holds the lock while the collection gets a new response
        serve(&server, "/api/v4/system", system(200)).await;
        let lock = new_update_lock();
        let (result, during) = collect_while_locked(&lock, map.set(&lock), prefix).await;
        result.unwrap();

        assert_eq!(before, during);
        assert!(scrape(prefix).contains(&format!("{prefix}_system_uptime_val 200")));
    }

    #[tokio::test]
    async fn set_exposes_the_model_read_at_startup() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_system_model";
        // without v6/system: the model does not depend on the sensors list
        let mut map =
            SystemMetricMap::new(&factory, prefix.to_string()).with_model(Some(BoxModel {
                model: Some("fbxgw7-r1/full".to_string()),
                model_name: Some("Freebox v7 (r1)".to_string()),
            }));

        serve(&server, "/api/v4/system", system(100)).await;
        map.set(update_lock()).await.unwrap();

        let line = format!(
            "{prefix}_system_model_info{{model=\"fbxgw7-r1/full\",model_name=\"Freebox v7 (r1)\"}} 1"
        );
        let metrics = scrape(prefix);
        assert!(metrics.contains(&line), "{line} in {metrics:?}");
    }

    /// guard: no model read at startup, no series
    #[tokio::test]
    async fn no_model_when_the_box_does_not_tell() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_system_no_model";
        let mut map = SystemMetricMap::new(&factory, prefix.to_string());

        serve(&server, "/api/v4/system", system(100)).await;
        map.set(update_lock()).await.unwrap();

        let metrics = scrape(prefix);
        assert!(
            metrics.contains(&format!("{prefix}_system_uptime_val 100")),
            "{metrics:?}"
        );
        assert!(
            !metrics.iter().any(|l| l.contains("_system_model_info")),
            "{metrics:?}"
        );
    }
}
