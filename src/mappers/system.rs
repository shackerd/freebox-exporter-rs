use async_trait::async_trait;
use log::debug;
use prometheus_exporter::prometheus::{
    register_int_gauge, register_int_gauge_vec, IntGauge, IntGaugeVec,
};
use reqwest::Client;
use serde::Deserialize;
use std::error::Error;

use super::MetricMap;
use crate::diagnostics::DryRunOutputWriter;
use crate::{
    core::common::{
        http_client_factory::{AuthenticatedHttpClientFactory, ManagedHttpClient},
        transport::{FreeboxResponse, FreeboxResponseError},
    },
    diagnostics::DryRunnable,
};

#[derive(Deserialize, Clone, Debug)]
pub struct SystemConfig {
    pub mac: Option<String>,
    pub box_flavor: Option<String>,
    pub temp_cpub: Option<i64>,
    pub disk_status: Option<String>,
    pub box_authenticated: Option<bool>,
    pub board_name: Option<String>,
    pub fan_rpm: Option<i64>,
    pub temp_sw: Option<i64>,
    pub uptime_val: Option<i64>,
    pub user_main_storage: Option<String>,
    pub temp_cpum: Option<i64>,
    pub serial: Option<String>,
    pub firmware_version: Option<String>,
}

pub struct SystemMetricMap<'a> {
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    managed_client: Option<ManagedHttpClient>,
    mac_metric: IntGaugeVec,
    box_flavor_metric: IntGaugeVec,
    temp_cpub_metric: IntGauge,
    disk_status_metric: IntGaugeVec,
    box_authenticated_metric: IntGauge,
    board_name_metric: IntGaugeVec,
    fan_rpm_metric: IntGauge,
    temp_sw_metric: IntGauge,
    uptime_val_metric: IntGauge,
    user_main_storage_metric: IntGaugeVec,
    temp_cpum_metric: IntGauge,
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
            temp_cpub_metric: register_int_gauge!(
                format!("{prefix}_system_temp_cpub"),
                format!("{prefix}_system_temp_cpub")
            )
            .expect(&format!("cannot create {prefix}_system_temp_cpub gauge")),
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
            temp_cpum_metric: register_int_gauge!(
                format!("{prefix}_system_temp_cpum"),
                format!("{prefix}_system_temp_cpum")
            )
            .expect(&format!("cannot create {prefix}_system_temp_cpum gauge")),
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
        self.temp_cpub_metric.set(0);
        self.disk_status_metric.reset();
        self.box_authenticated_metric.set(0);
        self.board_name_metric.reset();
        self.fan_rpm_metric.set(0);
        self.temp_sw_metric.set(0);
        self.uptime_val_metric.set(0);
        self.user_main_storage_metric.reset();
        self.temp_cpum_metric.set(0);
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
        self.temp_cpub_metric
            .set(sys_cnf.temp_cpub.clone().unwrap_or_default());
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
        self.temp_cpum_metric
            .set(sys_cnf.temp_cpum.unwrap_or_default());
        self.serial_metric
            .with_label_values(&[&sys_cnf.serial.clone().unwrap_or_default()])
            .set(1);
        self.firmware_version_metric
            .with_label_values(&[&sys_cnf.firmware_version.clone().unwrap_or_default()])
            .set(1);
        Ok(())
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

#[async_trait]
impl DryRunnable for SystemMetricMap<'_> {
    fn get_name(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        Ok("system".to_string())
    }

    async fn dry_run(
        &mut self,
        _writer: &mut dyn DryRunOutputWriter,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }

    fn as_dry_runnable(&mut self) -> &mut dyn DryRunnable {
        self
    }
}
