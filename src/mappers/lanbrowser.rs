use super::MetricMap;
use crate::core::common::{
    http_client_factory::{AuthenticatedHttpClientFactory, ManagedHttpClient},
    transport::{FreeboxResponse, FreeboxResponseError},
};
use async_trait::async_trait;
use log::{debug, warn};
use prometheus_exporter::prometheus::{register_int_gauge_vec, IntGaugeVec};
use prometheus_exporter::UpdateLock;
use reqwest::Client;
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

pub struct LanBrowserMetricMap<'a> {
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    managed_client: Option<ManagedHttpClient>,
    device_gauge: IntGaugeVec,
    device_l3_connectivity_gauge: IntGaugeVec,
    device_last_activity: IntGaugeVec,
    device_name_gauge: IntGaugeVec,
    iface_gauge: IntGaugeVec,
}

impl<'a> LanBrowserMetricMap<'a> {
    pub fn new(factory: &'a AuthenticatedHttpClientFactory<'a>, prefix: String) -> Self {
        let prfx = format!("{prefix}_lan_browser");

        Self {
            factory,
            managed_client: None,
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
            .unwrap_or_else(|e| panic!("cannot create {prfx}_devices gauge: {e}")),
            device_l3_connectivity_gauge: register_int_gauge_vec!(
                format!("{prfx}_device_l3_connectivity"),
                "device l3 connectivity, 1 for active",
                &["ident", "iface", "addr", "name", "af"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prfx}_device_l3 gauge: {e}")),
            device_last_activity: register_int_gauge_vec!(
                format!("{prfx}_device_last_activity"),
                "device last activity timestamp",
                &["iface", "name"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prfx}_device_last_activity gauge: {e}")),
            device_name_gauge: register_int_gauge_vec!(
                format!("{prfx}_device_name"),
                "device name",
                &["name", "source", "ident", "iface"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prfx}_name gauge: {e}")),
            iface_gauge: register_int_gauge_vec!(
                format!("{prfx}_iface_hosts"),
                "network interfaces",
                &["name"]
            )
            .unwrap_or_else(|e| panic!("cannot create {prfx}_ifaces gauge: {e}")),
        }
    }

    async fn get_devices(
        &mut self,
        interface: &LanBrowserInterface,
    ) -> Result<Vec<LanHost>, Box<dyn std::error::Error + Send + Sync>> {
        let iface = interface.name.as_ref().ok_or_else(|| {
            Box::new(FreeboxResponseError::new(
                "interface has no name".to_string(),
            ))
        })?;

        debug!("fetching {} interface devices", iface);

        let body = self
            .get_managed_client()
            .await?
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
            None => Err(Box::new(FreeboxResponseError::new(format!(
                "v4/lan/browser/{} response was empty",
                iface
            )))),
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

    async fn get_ifaces(
        &mut self,
    ) -> Result<Vec<LanBrowserInterface>, Box<dyn std::error::Error + Send + Sync>> {
        debug!("fetching ifaces & devices");

        let body = self
            .get_managed_client()
            .await?
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

    async fn set_all(
        &mut self,
        lock: &UpdateLock,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ifaces = self.get_ifaces().await?;

        // fetch every interface before touching the metrics: a scrape during the requests
        // still sees the previous collection, and a failed listing keeps it
        let mut fetched = vec![];
        let mut failed_ifaces = vec![];

        for iface in ifaces {
            if iface.host_count.unwrap_or(0) == 0 {
                fetched.push((iface, vec![]));
                continue;
            }

            match self.get_devices(&iface).await {
                Err(e) => {
                    let name = iface
                        .name
                        .to_owned()
                        .unwrap_or_else(|| "<unnamed>".to_string());
                    warn!("cannot fetch devices of interface {name}: {e}");
                    failed_ifaces.push(name);
                    fetched.push((iface, vec![]));
                }
                Ok(devs) => fetched.push((iface, devs)),
            }
        }

        // a scrape waits for the update to complete instead of returning part of it
        let _update = lock.lock();
        self.reset_all();

        for (iface, devs) in fetched {
            self.iface_gauge
                .with_label_values(&[&iface.name.to_owned().unwrap_or_default()])
                .set(iface.host_count.unwrap_or(0).into());

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

        // devices of the other interfaces stay exposed, but the collection is incomplete
        if !failed_ifaces.is_empty() {
            return Err(Box::new(FreeboxResponseError::new(format!(
                "cannot fetch devices of interface(s) {}",
                failed_ifaces.join(", ")
            ))));
        }

        Ok(())
    }
}

#[async_trait]
impl<'a> MetricMap<'a> for LanBrowserMetricMap<'a> {
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn set(
        &mut self,
        lock: &UpdateLock,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.set_all(lock).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use prometheus_exporter::prometheus::core::Collector;
    use serde_json::json;

    use super::*;
    use crate::mappers::test_support::{
        collect_while_locked, mock_api, new_update_lock, scrape, scrape_during, serve,
        serve_scoped, serve_slowly, update_lock,
    };

    async fn serve_interfaces(server: &wiremock::MockServer) {
        serve(
            server,
            "/api/v4/lan/browser/interfaces",
            json!({ "success": true, "result": [
                { "name": "pub", "host_count": 1 },
                { "name": "wifiguest", "host_count": 1 }
            ] }),
        )
        .await;
        serve(
            server,
            "/api/v4/lan/browser/pub",
            json!({ "success": true, "result": [
                { "id": "ether-1", "primary_name": "nas", "active": true, "l2ident": { "id": "mac-1", "type": "mac_address" } }
            ] }),
        )
        .await;
    }

    fn device_active(map: &LanBrowserMetricMap, iface: &str, name: &str) -> Option<i64> {
        map.device_gauge
            .collect()
            .iter()
            .flat_map(|family| family.get_metric())
            .find(|m| {
                let labels: Vec<_> = m
                    .get_label()
                    .iter()
                    .map(|l| (l.name(), l.value()))
                    .collect();
                labels.contains(&("iface", iface)) && labels.contains(&("primary_name", name))
            })
            .map(|m| m.get_gauge().get_value() as i64)
    }

    #[tokio::test]
    async fn set_reports_interface_whose_devices_cannot_be_fetched() {
        let (server, authenticator) = mock_api().await;
        serve_interfaces(&server).await;
        serve(
            &server,
            "/api/v4/lan/browser/wifiguest",
            json!({ "success": false, "error_code": "internal_error", "msg": "boom" }),
        )
        .await;

        let factory = authenticator.login().await.unwrap();
        let mut map = LanBrowserMetricMap::new(&factory, "test_lanbrowser_partial".to_string());

        let err = map.set(update_lock()).await.unwrap_err();
        assert!(err.to_string().contains("wifiguest"), "{err}");
        assert!(!err.to_string().contains("pub"), "{err}");
        assert_eq!(Some(1), device_active(&map, "pub", "nas"));
    }

    #[tokio::test]
    async fn set_succeeds_when_every_interface_is_fetched() {
        let (server, authenticator) = mock_api().await;
        serve_interfaces(&server).await;
        serve(
            &server,
            "/api/v4/lan/browser/wifiguest",
            json!({ "success": true, "result": [] }),
        )
        .await;

        let factory = authenticator.login().await.unwrap();
        let mut map = LanBrowserMetricMap::new(&factory, "test_lanbrowser_complete".to_string());

        map.set(update_lock()).await.unwrap();
        assert_eq!(Some(1), device_active(&map, "pub", "nas"));
    }

    #[tokio::test]
    async fn set_reports_unnamed_interface_instead_of_panicking() {
        let (server, authenticator) = mock_api().await;
        serve(
            &server,
            "/api/v4/lan/browser/interfaces",
            json!({ "success": true, "result": [
                { "host_count": 1 },
                { "name": "pub", "host_count": 1 }
            ] }),
        )
        .await;
        serve(
            &server,
            "/api/v4/lan/browser/pub",
            json!({ "success": true, "result": [
                { "id": "ether-1", "primary_name": "nas", "active": true }
            ] }),
        )
        .await;

        let factory = authenticator.login().await.unwrap();
        let mut map = LanBrowserMetricMap::new(&factory, "test_lanbrowser_unnamed".to_string());

        let err = map.set(update_lock()).await.unwrap_err();
        assert_eq!(
            "cannot fetch devices of interface(s) <unnamed>",
            err.to_string()
        );
        assert_eq!(Some(1), device_active(&map, "pub", "nas"));
        // the unnamed interface must not turn into a request on the interfaces root
        let requests = server.received_requests().await.unwrap();
        assert!(requests
            .iter()
            .all(|r| r.url.path() != "/api/v4/lan/browser/"));
    }

    #[tokio::test]
    async fn scrape_during_collection_sees_the_previous_collection() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_lanbrowser_scrape";
        let mut map = LanBrowserMetricMap::new(&factory, prefix.to_string());
        let interfaces = json!({ "success": true, "result": [
            { "name": "pub", "host_count": 1 },
            { "name": "wifiguest", "host_count": 1 }
        ] });
        let devices = |name: &str, active: bool| {
            json!({ "success": true, "result": [
            { "id": name, "primary_name": name, "active": active }
        ] })
        };

        let first = [
            serve_scoped(
                &server,
                "/api/v4/lan/browser/interfaces",
                interfaces.clone(),
            )
            .await,
            serve_scoped(&server, "/api/v4/lan/browser/pub", devices("nas", true)).await,
            serve_scoped(
                &server,
                "/api/v4/lan/browser/wifiguest",
                devices("phone", true),
            )
            .await,
        ];
        map.set(update_lock()).await.unwrap();
        let before = scrape(prefix);
        assert!(
            before
                .iter()
                .any(|l| l.contains("\"phone\"") && l.ends_with(" 1")),
            "{before:?}"
        );
        drop(first);

        serve(&server, "/api/v4/lan/browser/interfaces", interfaces).await;
        serve(&server, "/api/v4/lan/browser/pub", devices("nas", false)).await;
        serve_slowly(
            &server,
            "/api/v4/lan/browser/wifiguest",
            devices("phone", false),
        )
        .await;
        let (result, during) = scrape_during(
            &server,
            "/api/v4/lan/browser/wifiguest",
            map.set(update_lock()),
            prefix,
        )
        .await;
        result.unwrap();

        assert_eq!(before, during);
        assert_eq!(Some(0), device_active(&map, "wifiguest", "phone"));
        assert_eq!(Some(0), device_active(&map, "pub", "nas"));
    }

    #[tokio::test]
    async fn collection_waits_for_a_scrape_in_progress() {
        let (server, authenticator) = mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_lanbrowser_lock";
        let mut map = LanBrowserMetricMap::new(&factory, prefix.to_string());
        let interfaces = json!({ "success": true, "result": [
            { "name": "pub", "host_count": 1 },
            { "name": "wifiguest", "host_count": 1 }
        ] });
        let devices = |name: &str, active: bool| {
            json!({ "success": true, "result": [
            { "id": name, "primary_name": name, "active": active }
        ] })
        };

        let first = [
            serve_scoped(
                &server,
                "/api/v4/lan/browser/interfaces",
                interfaces.clone(),
            )
            .await,
            serve_scoped(&server, "/api/v4/lan/browser/pub", devices("nas", true)).await,
            serve_scoped(
                &server,
                "/api/v4/lan/browser/wifiguest",
                devices("phone", true),
            )
            .await,
        ];
        map.set(update_lock()).await.unwrap();
        let before = scrape(prefix);
        assert!(
            before
                .iter()
                .any(|l| l.contains("\"phone\"") && l.ends_with(" 1")),
            "{before:?}"
        );
        drop(first);

        serve(&server, "/api/v4/lan/browser/interfaces", interfaces).await;
        serve(&server, "/api/v4/lan/browser/pub", devices("nas", false)).await;
        serve(
            &server,
            "/api/v4/lan/browser/wifiguest",
            devices("phone", false),
        )
        .await;
        let lock = new_update_lock();
        let (result, during) = collect_while_locked(&lock, map.set(&lock), prefix).await;
        result.unwrap();

        assert_eq!(before, during);
        assert_eq!(Some(0), device_active(&map, "wifiguest", "phone"));
        assert_eq!(Some(0), device_active(&map, "pub", "nas"));
    }
}
