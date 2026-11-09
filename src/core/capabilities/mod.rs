use crate::mappers::wifi::models::WifiConfig;

use super::common::{
    http_client_factory::AuthenticatedHttpClientFactory,
    transport::{get_api_result, FreeboxResponse, FreeboxResponseError},
};
use crate::mappers::{ddns, raid, vm, vpn};
use log::{debug, info, warn};
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
struct LanConfig {
    mode: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
struct ModelInfo {
    has_vm: Option<bool>,
}

#[derive(Deserialize, Clone, Debug)]
struct SystemConfig {
    model_info: Option<ModelInfo>,
}

#[derive(Deserialize, Clone, Debug)]
struct ApiVersionInfo {
    api_version: Option<String>,
}

/// First API version exposing SystemConfig.model_info, measured on a Freebox Delta
const MODEL_INFO_MIN_API_VERSION: u32 = 6;

pub struct CapabilitiesAgent<'a> {
    client_factory: &'a AuthenticatedHttpClientFactory<'a>,
}

impl<'a> CapabilitiesAgent<'a> {
    pub fn new(client_factory: &'a AuthenticatedHttpClientFactory) -> Self {
        CapabilitiesAgent { client_factory }
    }

    /// Load the capabilities of the Freebox
    pub async fn load(&self) -> Result<Capabilities, Box<dyn std::error::Error + Send + Sync>> {
        debug!("Loading capabilities");

        let client = self.client_factory.create_managed_client().await?;
        let url = format!("{}v4/lan/config", self.client_factory.api_url);
        let res = client
            .get()?
            .get(url)
            .send()
            .await?
            .json::<FreeboxResponse<LanConfig>>()
            .await?;

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        let lan_config = res
            .result
            .ok_or_else(|| Box::new(FreeboxResponseError::new("Missing LAN config".to_string())))?;

        let is_router = lan_config
            .mode
            .as_deref()
            .unwrap_or("")
            .eq_ignore_ascii_case("router");

        info!(
            "detected freebox network mode: {}",
            lan_config.mode.as_deref().unwrap_or("unknown")
        );

        let is_wifi_enabled = self.is_wifi_enabled().await?;
        // only vm, raid, vpn and ddns depend on these probes: their failure leaves those
        // capabilities unknown (None) instead of preventing the exporter from starting
        let (api_major, has_vm) = match self.get_api_major_version().await {
            Ok(api_major) => match self.has_vm(api_major).await {
                Ok(has_vm) => (Some(api_major), Some(has_vm)),
                Err(e) => {
                    warn!("cannot detect virtual machines support: {e}");
                    (Some(api_major), None)
                }
            },
            Err(e) => {
                warn!("cannot detect freebox api version: {e}");
                (None, None)
            }
        };

        Ok(Capabilities {
            connection: Some(true),
            system: Some(true),
            lan: Some(true),
            lan_browser: Some(is_router),
            switch: Some(is_router),
            wifi: Some(is_wifi_enabled),
            dhcp: Some(is_router),
            nas: Some(true), // NAS is always available
            vm: api_major
                .zip(has_vm)
                .map(|(major, has_vm)| major >= vm::MIN_API_VERSION && has_vm),
            raid: api_major.map(|major| major >= raid::MIN_API_VERSION),
            vpn: api_major.map(|major| major >= vpn::MIN_API_VERSION && is_router),
            ddns: api_major.map(|major| major >= ddns::MIN_API_VERSION && is_router),
            network_mode: lan_config.mode,
        })
    }

    /// Major version of the API supported by the box, e.g. 16 for "16.0".
    /// Collectors compare it with the API version prefix they call
    async fn get_api_major_version(&self) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.client_factory.create_managed_client().await?.get()?;
        // api_url is <root>/api/ while api_version is served at <root>/api_version
        let url = format!(
            "{}api_version",
            self.client_factory.api_url.trim_end_matches("api/")
        );
        let res = client.get(url).send().await?.json::<ApiVersionInfo>().await?;

        let version = res.api_version.ok_or_else(|| {
            Box::new(FreeboxResponseError::new("Missing api_version".to_string()))
        })?;
        let major = version
            .split('.')
            .next()
            .and_then(|m| m.parse::<u32>().ok())
            .ok_or_else(|| {
                Box::new(FreeboxResponseError::new(format!(
                    "cannot parse api_version {version}"
                )))
            })?;

        info!("detected freebox api version: {version}");
        Ok(major)
    }

    /// Whether the hardware supports virtual machines, from SystemConfig.model_info
    async fn has_vm(&self, api_major: u32) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if api_major < MODEL_INFO_MIN_API_VERSION {
            return Ok(false);
        }

        let client = self.client_factory.create_managed_client().await?.get()?;
        let system = get_api_result::<SystemConfig>(
            &client,
            &self.client_factory.api_url,
            &format!("v{MODEL_INFO_MIN_API_VERSION}/system/"),
        )
        .await?;

        let has_vm = system
            .and_then(|s| s.model_info)
            .and_then(|m| m.has_vm)
            .unwrap_or(false);

        info!("detected freebox virtual machines support: {has_vm}");
        Ok(has_vm)
    }

    async fn is_wifi_enabled(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        debug!("Checking if WiFi is enabled");
        let client = self.client_factory.create_managed_client().await?;
        let url = format!("{}v4/wifi/config", self.client_factory.api_url);
        let res = client
            .get()?
            .get(url)
            .send()
            .await?
            .json::<FreeboxResponse<WifiConfig>>()
            .await?;

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }
        let wifi_config = res.result.ok_or_else(|| {
            Box::new(FreeboxResponseError::new("Missing WiFi config".to_string()))
        })?;

        Ok(wifi_config.enabled.unwrap_or(false))
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Capabilities {
    #[allow(dead_code)] // maybe use it in the future
    pub connection: Option<bool>,
    #[allow(dead_code)] // maybe use it in the future
    pub system: Option<bool>,
    #[allow(dead_code)] // maybe use it in the future
    pub lan: Option<bool>,
    pub lan_browser: Option<bool>,
    pub switch: Option<bool>,
    pub wifi: Option<bool>,
    pub dhcp: Option<bool>,
    pub nas: Option<bool>,
    /// None when support could not be detected
    pub vm: Option<bool>,
    pub raid: Option<bool>,
    pub vpn: Option<bool>,
    pub ddns: Option<bool>,
    pub network_mode: Option<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::{matchers::path, Mock, ResponseTemplate};

    use super::*;
    use crate::mappers::test_support::{mock_api, serve};

    /// Loads capabilities of a box in `mode` announcing `api_version`, with or without VM hardware
    async fn load_mode(mode: &str, api_version: &str, has_vm: bool) -> Capabilities {
        let (server, authenticator) = mock_api().await;
        serve(&server, "/api/v4/lan/config", json!({ "success": true, "result": { "mode": mode } })).await;
        serve(&server, "/api/v4/wifi/config", json!({ "success": true, "result": { "enabled": true } })).await;
        serve(&server, "/api_version", json!({ "api_version": api_version })).await;
        serve(
            &server,
            "/api/v6/system/",
            json!({ "success": true, "result": { "model_info": { "has_vm": has_vm } } }),
        )
        .await;

        let factory = authenticator.login().await.unwrap();
        CapabilitiesAgent::new(&factory).load().await.unwrap()
    }

    async fn load(api_version: &str, has_vm: bool) -> Capabilities {
        load_mode("router", api_version, has_vm).await
    }

    #[tokio::test]
    async fn vm_requires_api_7_and_vm_hardware() {
        assert_eq!(Some(true), load("7.0", true).await.vm);
        assert_eq!(Some(false), load("6.4", true).await.vm);
        assert_eq!(Some(false), load("16.0", false).await.vm);
    }

    #[tokio::test]
    async fn raid_follows_its_minimum_api_version() {
        assert_eq!(Some(false), load("7.0", true).await.raid);
        assert_eq!(Some(true), load("8.0", false).await.raid);
    }

    #[tokio::test]
    async fn vpn_requires_api_4_and_router_mode() {
        assert_eq!(Some(false), load("3.0", false).await.vpn);
        assert_eq!(Some(true), load("4.0", false).await.vpn);
        assert_eq!(Some(false), load_mode("bridge", "16.0", false).await.vpn);
    }

    #[tokio::test]
    async fn ddns_requires_api_4_and_router_mode() {
        assert_eq!(Some(false), load("3.0", false).await.ddns);
        assert_eq!(Some(true), load("4.0", false).await.ddns);
        assert_eq!(Some(false), load_mode("bridge", "16.0", false).await.ddns);
    }

    #[tokio::test]
    async fn model_info_is_not_requested_below_api_6() {
        let (server, authenticator) = mock_api().await;
        serve(&server, "/api/v4/lan/config", json!({ "success": true, "result": { "mode": "router" } })).await;
        serve(&server, "/api/v4/wifi/config", json!({ "success": true, "result": { "enabled": true } })).await;
        serve(&server, "/api_version", json!({ "api_version": "5.0" })).await;
        Mock::given(path("/api/v6/system/"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let factory = authenticator.login().await.unwrap();
        let caps = CapabilitiesAgent::new(&factory).load().await.unwrap();

        assert_eq!(Some(false), caps.vm);
    }

    #[tokio::test]
    async fn failing_api_version_probe_leaves_new_capabilities_unknown() {
        let (server, authenticator) = mock_api().await;
        serve(&server, "/api/v4/lan/config", json!({ "success": true, "result": { "mode": "router" } })).await;
        serve(&server, "/api/v4/wifi/config", json!({ "success": true, "result": { "enabled": true } })).await;
        serve(&server, "/api_version", json!({ "api_version": "garbage" })).await;

        let factory = authenticator.login().await.unwrap();
        let caps = CapabilitiesAgent::new(&factory).load().await.unwrap();

        assert_eq!((Some(true), Some(true)), (caps.wifi, caps.switch));
        assert_eq!(None, caps.vm);
        assert_eq!(None, caps.raid);
        assert_eq!(None, caps.vpn);
        assert_eq!(None, caps.ddns);
    }

    #[tokio::test]
    async fn unresponsive_api_version_probe_does_not_stall_startup() {
        let (server, authenticator) = mock_api().await;
        serve(&server, "/api/v4/lan/config", json!({ "success": true, "result": { "mode": "router" } })).await;
        serve(&server, "/api/v4/wifi/config", json!({ "success": true, "result": { "enabled": true } })).await;
        Mock::given(path("/api_version"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "api_version": "16.0" }))
                    .set_delay(std::time::Duration::from_secs(5)),
            )
            .mount(&server)
            .await;

        let mut factory = authenticator.login().await.unwrap();
        factory.set_request_timeout(std::time::Duration::from_millis(200));
        let started = std::time::Instant::now();
        let caps = CapabilitiesAgent::new(&factory).load().await.unwrap();

        assert!(started.elapsed() < std::time::Duration::from_secs(3), "{:?}", started.elapsed());
        assert_eq!(Some(true), caps.wifi);
        assert_eq!((None, None, None, None), (caps.vm, caps.raid, caps.vpn, caps.ddns));
    }

    #[tokio::test]
    async fn failing_model_info_probe_leaves_vm_unknown_only() {
        let (server, authenticator) = mock_api().await;
        serve(&server, "/api/v4/lan/config", json!({ "success": true, "result": { "mode": "router" } })).await;
        serve(&server, "/api/v4/wifi/config", json!({ "success": true, "result": { "enabled": true } })).await;
        serve(&server, "/api_version", json!({ "api_version": "16.0" })).await;
        serve(&server, "/api/v6/system/", json!({ "success": false, "error_code": "internal_error" })).await;

        let factory = authenticator.login().await.unwrap();
        let caps = CapabilitiesAgent::new(&factory).load().await.unwrap();

        assert_eq!(None, caps.vm);
        assert_eq!(Some(true), caps.raid);
        assert_eq!(Some(true), caps.vpn);
        assert_eq!(Some(true), caps.ddns);
    }
}
