use super::MetricMap;
use crate::core::common::{AuthenticatedHttpClientFactory, FreeboxResponse, FreeboxResponseError};
use async_trait::async_trait;
use log::{debug, error};
use prometheus_exporter::prometheus::{register_int_gauge_vec, IntGaugeVec};
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct LanBrowserInterface {
    name: Option<String>,
    host_count: Option<i32>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LanHost {
    id: Option<String>,
    primary_name: Option<String>,
    host_type: Option<String>,
    primary_name_manual: Option<bool>,
    l2ident: Option<LanHostL2Ident>,
    vendor_name: Option<String>,
    active: Option<bool>,
    last_activity: Option<i64>,
    names: Option<Vec<LanHostName>>,
    l3connectivities: Option<Vec<LanHostL3Connectivity>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LanHostName {
    pub name: Option<String>,
    pub source: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LanHostL2Ident {
    pub id: Option<String>,
    #[serde(alias = "type")]
    pub _type: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LanHostL3Connectivity {
    pub addr: Option<String>,
    pub af: Option<String>,
    pub active: Option<bool>,
    // pub reachable: Option<bool>,
    // pub last_activity: Option<i64>,
    // pub last_time_reachable: Option<i64>,
}

pub struct LanBrowserMetricMap {
    factory: AuthenticatedHttpClientFactory,
    device_gauge: IntGaugeVec,
    device_l3_connectivity_gauge: IntGaugeVec,
    device_last_activity: IntGaugeVec,
    device_name_gauge: IntGaugeVec,
    iface_gauge: IntGaugeVec,
}

impl LanBrowserMetricMap {
    pub fn new(factory: AuthenticatedHttpClientFactory, prefix: String) -> Self {
        let prfx = format!("{prefix}_lan_browser");

        Self {
            factory,
            device_gauge: register_int_gauge_vec!(
                format!("{prfx}_device"),
                "device, 1 for active",
                &[
                    "iface",
                    "primary_name",
                    "id",
                    "type",
                    "primary_name_manual",
                    "l2ident_id",
                    "l2ident_type",
                    "vendor_name"
                ]
            )
            .expect(&format!("cannot create {prfx}_devices gauge")),
            device_l3_connectivity_gauge: register_int_gauge_vec!(
                format!("{prfx}_device_l3_connectivity"),
                "device l3 connectivity, 1 for active",
                &["ident", "iface", "addr", "name", "af"]
            )
            .expect("cannot create {prfx}_device_l3 gauge"),
            device_last_activity: register_int_gauge_vec!(
                format!("{prfx}_device_last_activity"),
                "device last activity timestamp",
                &["iface", "name"]
            )
            .expect(&format!("cannot create {prfx}_device_last_activity gauge")),
            device_name_gauge: register_int_gauge_vec!(
                format!("{prfx}_device_name"),
                "device name",
                &["name", "source", "ident", "iface"]
            )
            .expect(&format!("cannot create {prfx}_name gauge")),
            iface_gauge: register_int_gauge_vec!(
                format!("{prfx}_iface_hosts"),
                "network interfaces",
                &["name"]
            )
            .expect(&format!("cannot create {prfx}_ifaces gauge")),
        }
    }

    async fn get_devices(
        &self,
        interface: &LanBrowserInterface,
    ) -> Result<Vec<LanHost>, Box<dyn std::error::Error>> {
        let iface = interface.name.as_ref().unwrap();

        debug!("fetching {} interface devices", iface);

        let body = self
            .factory
            .create_client()
            .await
            .unwrap()
            .get(format!("{}v4/lan/browser/{}", self.factory.api_url, iface))
            .send()
            .await?
            .text()
            .await?;

        let res = match serde_json::from_str::<FreeboxResponse<Vec<LanHost>>>(&body) {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        match res.result {
            Some(r) => Ok(r),
            None => {
                return Err(Box::new(FreeboxResponseError::new(format!(
                    "v4/lan/browser/{} response was empty",
                    iface
                ))))
            }
        }
    }

    async fn get_ifaces(&self) -> Result<Vec<LanBrowserInterface>, Box<dyn std::error::Error>> {
        debug!("fetching ifaces & devices");

        let body = self
            .factory
            .create_client()
            .await
            .unwrap()
            .get(format!("{}v4/lan/browser/interfaces", self.factory.api_url))
            .send()
            .await?
            .text()
            .await?;

        let res = match serde_json::from_str::<FreeboxResponse<Vec<LanBrowserInterface>>>(&body) {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        match res.result {
            Some(r) => Ok(r),
            None => Err(Box::new(FreeboxResponseError::new(
                "v4/lan/browser/interfaces response was empty".to_string(),
            ))),
        }
    }

    fn reset_all(&self) {
        self.device_gauge.reset();
        self.device_l3_connectivity_gauge.reset();
        self.device_last_activity.reset();
        self.device_name_gauge.reset();
        self.iface_gauge.reset();
    }

    async fn set_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_all();

        let ifaces = match self.get_ifaces().await {
            Err(e) => return Err(e),
            Ok(r) => r,
        };

        for iface in ifaces {
            self.iface_gauge
                .with_label_values(&[&iface.name.to_owned().unwrap_or_default()])
                .set(iface.host_count.unwrap_or(0).into());

            if iface.host_count.unwrap_or(0) == 0 {
                continue;
            }

            match self.get_devices(&iface).await {
                Err(e) => error!("{e:#?}"),
                Ok(devs) => {
                    for dev in devs {
                        let l2ident = dev.l2ident.unwrap_or(LanHostL2Ident {
                            id: None,
                            _type: None,
                        });

                        self.device_gauge
                            .with_label_values(&[
                                &iface.name.to_owned().unwrap_or_default(),
                                &dev.primary_name.to_owned().unwrap_or_default(),
                                &dev.id.unwrap_or_default(),
                                &dev.host_type.unwrap_or_default(),
                                &dev.primary_name_manual.unwrap_or_default().to_string(),
                                &l2ident.id.to_owned().unwrap_or_default(),
                                &l2ident._type.to_owned().unwrap_or_default(),
                                &dev.vendor_name.unwrap_or_default(),
                            ])
                            .set(dev.active.unwrap_or_default().into());

                        self.device_last_activity
                            .with_label_values(&[
                                &iface.to_owned().name.unwrap_or_default(),
                                &dev.primary_name.to_owned().unwrap_or_default(),
                            ])
                            .set(dev.last_activity.unwrap_or_default());

                        let l3s = dev.l3connectivities.unwrap_or(vec![]);

                        for l3 in l3s {
                            self.device_l3_connectivity_gauge
                                .with_label_values(&[
                                    &l2ident.id.to_owned().unwrap_or_default(),
                                    &iface.name.to_owned().unwrap_or_default(),
                                    &l3.addr.unwrap_or_default(),
                                    &dev.primary_name.to_owned().unwrap_or_default(),
                                    &l3.af.unwrap_or_default(),
                                ])
                                .set(l3.active.unwrap_or_default().into());
                        }

                        let names = dev.names.unwrap_or(vec![]);

                        for name in names {
                            self.device_name_gauge
                                .with_label_values(&[
                                    &name.name.unwrap_or_default(),
                                    &name.source.unwrap_or_default(),
                                    &l2ident.id.to_owned().unwrap_or_default(),
                                    &iface.name.to_owned().unwrap_or_default(),
                                ])
                                .set(1);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

#[async_trait]
impl MetricMap for LanBrowserMetricMap {
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.init().await {
            Err(e) => return Err(e),
            _ => {}
        };
        Ok(())
    }

    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.set_all().await {
            Err(e) => return Err(e),
            _ => {}
        };
        Ok(())
    }
}
