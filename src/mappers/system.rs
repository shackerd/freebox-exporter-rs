use async_trait::async_trait;
use log::debug;
use prometheus_exporter::prometheus::{
    register_int_gauge, register_int_gauge_vec, IntGauge, IntGaugeVec,
};
use reqwest::Client;
use serde::Deserialize;


use super::MetricMap;
use crate::core::common::{
    http_client_factory::{AuthenticatedHttpClientFactory, ManagedHttpClient},
    transport::{FreeboxResponse, FreeboxResponseError},
};

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
}

pub struct SystemMetricMap<'a> {
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    managed_client: Option<ManagedHttpClient>,
    mac_metric: IntGaugeVec,
    box_flavor_metric: IntGaugeVec,
    box_model_name_metric: IntGaugeVec,
    device_name_metric: IntGaugeVec,
    api_version_metric: IntGaugeVec,
    temp_hdd_metric: IntGauge,
    temp_t1_metric: IntGauge,
    temp_t2_metric: IntGauge,
    temp_cpu_metric: IntGaugeVec, // New: CPU temperature with core label
    // Legacy metrics for backward compatibility
    temp_cpub_metric: IntGauge,
    temp_cpum_metric: IntGauge,
    disk_status_metric: IntGaugeVec,
    box_authenticated_metric: IntGauge,
    board_name_metric: IntGaugeVec,
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
            mac_metric: register_int_gauge_vec!(
                format!("{prefix}_system_mac"),
                format!("{prefix}_system_mac"),
                &["mac"]
            )
            .expect(&format!("cannot create {prefix}_system_mac gauge")),
            box_flavor_metric: register_int_gauge_vec!(
                format!("{prefix}_system_box_flavor"),
                format!("{prefix}_system_box_flavor"),
                &["box_flavor"]
            )
            .expect(&format!("cannot create {prefix}_system_box_flavor gauge")),
            box_model_name_metric: register_int_gauge_vec!(
                format!("{prefix}_system_box_model_name"),
                format!("{prefix}_system_box_model_name"),
                &["model_name"]
            )
            .expect(&format!("cannot create {prefix}_system_box_model_name gauge")),
            device_name_metric: register_int_gauge_vec!(
                format!("{prefix}_system_device_name"),
                format!("{prefix}_system_device_name"),
                &["device_name"]
            )
            .expect(&format!("cannot create {prefix}_system_device_name gauge")),
            api_version_metric: register_int_gauge_vec!(
                format!("{prefix}_system_api_version"),
                format!("{prefix}_system_api_version"),
                &["api_version"]
            )
            .expect(&format!("cannot create {prefix}_system_api_version gauge")),
            temp_hdd_metric: register_int_gauge!(
                format!("{prefix}_system_temp_hdd"),
                format!("{prefix}_system_temp_hdd")
            )
            .expect(&format!("cannot create {prefix}_system_temp_hdd gauge")),
            temp_t1_metric: register_int_gauge!(
                format!("{prefix}_system_temp_t1"),
                format!("{prefix}_system_temp_t1")
            )
            .expect(&format!("cannot create {prefix}_system_temp_t1 gauge")),
            temp_t2_metric: register_int_gauge!(
                format!("{prefix}_system_temp_t2"),
                format!("{prefix}_system_temp_t2")
            )
            .expect(&format!("cannot create {prefix}_system_temp_t2 gauge")),
            temp_cpu_metric: register_int_gauge_vec!(
                format!("{prefix}_system_temp_cpu"),
                format!("{prefix}_system_temp_cpu CPU core temperature"),
                &["core"]
            )
            .expect(&format!("cannot create {prefix}_system_temp_cpu gauge")),
            // Legacy metrics for backward compatibility
            temp_cpub_metric: register_int_gauge!(
                format!("{prefix}_system_temp_cpub"),
                format!("{prefix}_system_temp_cpub")
            )
            .expect(&format!("cannot create {prefix}_system_temp_cpub gauge")),
            temp_cpum_metric: register_int_gauge!(
                format!("{prefix}_system_temp_cpum"),
                format!("{prefix}_system_temp_cpum")
            )
            .expect(&format!("cannot create {prefix}_system_temp_cpum gauge")),
            disk_status_metric: register_int_gauge_vec!(
                format!("{prefix}_system_disk_status"),
                format!("{prefix}_system_disk_status"),
                &["disk_status"]
            )
            .expect(&format!("cannot create {prefix}_system_disk_status gauge")),
            box_authenticated_metric: register_int_gauge!(
                format!("{prefix}_system_box_authenticated"),
                format!("{prefix}_system_box_authenticated")
            )
            .expect(&format!(
                "cannot create {prefix}_system_box_authenticated gauge"
            )),
            board_name_metric: register_int_gauge_vec!(
                format!("{prefix}_system_board_name"),
                format!("{prefix}_system_board_name"),
                &["board_name"]
            )
            .expect(&format!("cannot create {prefix}_system_board_name gauge")),
            fan_rpm_metric: register_int_gauge!(
                format!("{prefix}_system_fan_rpm"),
                format!("{prefix}_system_fan_rpm")
            )
            .expect(&format!("cannot create {prefix}_system_fan_rpm gauge")),
            temp_sw_metric: register_int_gauge!(
                format!("{prefix}_system_temp_sw"),
                format!("{prefix}_system_temp_sw")
            )
            .expect(&format!("cannot create {prefix}_system_temp_sw gauge")),
            uptime_val_metric: register_int_gauge!(
                format!("{prefix}_system_uptime_val"),
                format!("{prefix}_system_uptime_val")
            )
            .expect(&format!("cannot create {prefix}_system_uptime_val gauge")),
            user_main_storage_metric: register_int_gauge_vec!(
                format!("{prefix}_system_user_main_storage"),
                format!("{prefix}_system_user_main_storage"),
                &["user_main_storage"]
            )
            .expect(&format!(
                "cannot create {prefix}_system_user_main_storage gauge"
            )),
            serial_metric: register_int_gauge_vec!(
                format!("{prefix}_system_serial"),
                format!("{prefix}_system_serial"),
                &["serial"]
            )
            .expect(&format!("cannot create {prefix}_system_serial gauge")),
            firmware_version_metric: register_int_gauge_vec!(
                format!("{prefix}_system_firmware_version"),
                format!("{prefix}_system_firmware_version"),
                &["firmware_version"]
            )
            .expect(&format!(
                "cannot create {prefix}_system_firmware_version gauge"
            )),
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
        self.mac_metric.reset();
        self.box_flavor_metric.reset();
        self.box_model_name_metric.reset();
        self.device_name_metric.reset();
        self.api_version_metric.reset();
        self.temp_hdd_metric.set(0);
        self.temp_t1_metric.set(0);
        self.temp_t2_metric.set(0);
        self.temp_cpu_metric.reset();
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

    async fn set_system_config(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching system config");

        let body = self
            .get_managed_client()
            .await
            .unwrap()
            .get(format!("{}v4/system", self.factory.api_url))
            .send()
            .await?
            .text()
            .await?;

        let res = match serde_json::from_str::<FreeboxResponse<SystemConfig>>(&body) {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        let sys_cnf: SystemConfig = match res.result {
            None => {
                return Err(Box::new(FreeboxResponseError::new(
                    "v4/system response was empty".to_string(),
                )))
            }
            Some(r) => r,
        };

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
        self.temp_hdd_metric.set(sys_cnf.temp_hdd.unwrap_or_default());
        
        // Set additional temperature sensors if available
        self.temp_t1_metric.set(sys_cnf.temp_t1.unwrap_or_default());
        self.temp_t2_metric.set(sys_cnf.temp_t2.unwrap_or_default());

        // Handle CPU temperatures - support both legacy and new formats
        self.handle_cpu_temperatures(&sys_cnf);

        self.disk_status_metric
            .with_label_values(&[&sys_cnf.disk_status.clone().unwrap_or_default()])
            .set(sys_cnf.disk_status.is_some().into());
        self.box_authenticated_metric
            .set(sys_cnf.box_authenticated.unwrap_or_default().into());
        self.board_name_metric
            .with_label_values(&[&sys_cnf.board_name.clone().unwrap_or_default()])
            .set(1);
        self.fan_rpm_metric.set(sys_cnf.fan_rpm.unwrap_or_default());
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
        if sys_cnf.temp_cpu0.is_some() || sys_cnf.temp_cpu1.is_some() || 
           sys_cnf.temp_cpu2.is_some() || sys_cnf.temp_cpu3.is_some() {
            
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

    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.reset_all();

        match self.set_system_config().await {
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
}
