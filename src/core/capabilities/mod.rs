use crate::core::configuration::sections::CapabilitiesConfiguration;
use crate::mappers::wifi::models::WifiConfig;

use super::common::{
    error_chain::with_causes,
    http_client_factory::AuthenticatedHttpClientFactory,
    transport::{get_api_result, FreeboxResponse, FreeboxResponseError},
};
use crate::mappers::{ddns, raid, system, vm, vpn};
use log::{debug, warn};
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
    sensors: Option<Vec<serde::de::IgnoredAny>>,
}

/// What v6/system/ tells about the hardware of the box
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SystemProbe {
    has_vm: bool,
    /// Whether v6/system/ lists the temperature sensors
    has_sensors: bool,
}

#[derive(Deserialize, Clone, Debug)]
struct ApiVersionInfo {
    api_version: Option<String>,
    // any type: an unexpected one must not prevent reading api_version
    box_model: Option<serde_json::Value>,
    box_model_name: Option<serde_json::Value>,
    device_name: Option<serde_json::Value>,
}

/// A non empty text, None for any other value
fn text(value: Option<serde_json::Value>) -> Option<String> {
    match value {
        Some(serde_json::Value::String(text)) if !text.is_empty() => Some(text),
        _ => None,
    }
}

/// What /api_version tells about the box
struct ApiVersion {
    major: u32,
    version: String,
    model: Option<system::BoxModel>,
    device_name: Option<String>,
}

/// First API version exposing SystemConfig.model_info, measured on a Freebox Delta
const MODEL_INFO_MIN_API_VERSION: u32 = 6;

pub struct CapabilitiesAgent<'a> {
    client_factory: &'a AuthenticatedHttpClientFactory<'a>,
    /// Logs what is detected at debug level rather than info, for a detection done again
    quiet: bool,
}

impl<'a> CapabilitiesAgent<'a> {
    pub fn new(client_factory: &'a AuthenticatedHttpClientFactory) -> Self {
        CapabilitiesAgent {
            client_factory,
            quiet: false,
        }
    }

    /// Logs what is detected at debug level, as for a periodic detection
    pub fn quiet(mut self) -> Self {
        self.quiet = true;
        self
    }

    fn detected_level(&self) -> log::Level {
        if self.quiet {
            log::Level::Debug
        } else {
            log::Level::Info
        }
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

        log::log!(
            self.detected_level(),
            "detected freebox network mode: {}",
            lan_config.mode.as_deref().unwrap_or("unknown")
        );

        let is_wifi_enabled = self.is_wifi_enabled().await?;
        // only vm, raid, vpn and ddns depend on these probes: their failure leaves those
        // capabilities unknown (None) instead of preventing the exporter from starting
        let (api, probe) = match self.get_api_version().await {
            Ok(api) => match self.probe_system(api.major).await {
                Ok(probe) => (Some(api), Some(probe)),
                Err(e) => {
                    warn!(
                        "cannot detect virtual machines and sensors support: {}",
                        with_causes(e.as_ref())
                    );
                    (Some(api), None)
                }
            },
            Err(e) => {
                warn!(
                    "cannot detect freebox api version: {}",
                    with_causes(e.as_ref())
                );
                (None, None)
            }
        };
        let api_major = api.as_ref().map(|api| api.major);

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
                .zip(probe)
                .map(|(major, probe)| major >= vm::MIN_API_VERSION && probe.has_vm),
            raid: api_major.map(|major| major >= raid::MIN_API_VERSION),
            vpn: api_major.map(|major| major >= vpn::MIN_API_VERSION && is_router),
            ddns: api_major.map(|major| major >= ddns::MIN_API_VERSION && is_router),
            // selected once: the sensor labels depend on the source of the temperatures
            v6_system: api_major
                .zip(probe)
                .map(|(major, probe)| major >= system::V6_SYSTEM_API_VERSION && probe.has_sensors),
            network_mode: lan_config.mode,
            api_version: api.as_ref().map(|api| api.version.clone()),
            model: api.as_ref().and_then(|api| api.model.clone()),
            device_name: api.and_then(|api| api.device_name),
        })
    }

    /// Version of the API supported by the box, e.g. "16.1", its major version, 16, and the
    /// model of the box. Collectors compare the major version with the API version prefix they
    /// call
    async fn get_api_version(
        &self,
    ) -> Result<ApiVersion, Box<dyn std::error::Error + Send + Sync>> {
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

        log::log!(
            self.detected_level(),
            "detected freebox api version: {version}"
        );
        let model = system::BoxModel {
            model: text(res.box_model),
            model_name: text(res.box_model_name),
        };
        Ok(ApiVersion {
            major,
            version,
            model: (model != system::BoxModel::default()).then_some(model),
            device_name: text(res.device_name),
        })
    }

    /// Whether the hardware supports virtual machines, from SystemConfig.model_info, and
    /// whether v6/system/ lists the temperature sensors
    async fn probe_system(
        &self,
        api_major: u32,
    ) -> Result<SystemProbe, Box<dyn std::error::Error + Send + Sync>> {
        if api_major < MODEL_INFO_MIN_API_VERSION {
            return Ok(SystemProbe::default());
        }

        let client = self.client_factory.create_managed_client().await?.get()?;
        let system = get_api_result::<SystemConfig>(
            &client,
            &self.client_factory.api_url,
            &format!("v{MODEL_INFO_MIN_API_VERSION}/system/"),
        )
        .await?;

        let has_sensors = system.as_ref().is_some_and(|s| s.sensors.is_some());
        let has_vm = system
            .and_then(|s| s.model_info)
            .and_then(|m| m.has_vm)
            .unwrap_or(false);

        log::log!(
            self.detected_level(),
            "detected freebox virtual machines support: {has_vm}"
        );
        log::log!(
            self.detected_level(),
            "detected freebox sensors list: {has_sensors}"
        );
        Ok(SystemProbe { has_vm, has_sensors })
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

#[derive(Deserialize, Clone, Debug, PartialEq)]
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
    /// Whether the box lists every fan and temperature sensor, None when its API version
    /// is unknown
    pub v6_system: Option<bool>,
    pub network_mode: Option<String>,
    /// Version of the API, e.g. "16.1", None when it could not be read
    pub api_version: Option<String>,
    /// From /api_version, None when the box does not tell or its API version is unknown
    pub model: Option<system::BoxModel>,
    /// From /api_version, e.g. "Freebox Server"
    pub device_name: Option<String>,
}

impl Capabilities {
    /// Whether every capability depending on a detection is known: the API version, read for
    /// vm, raid, vpn and ddns, and the sensors list of v6/system/
    pub fn is_complete(&self) -> bool {
        [self.vm, self.raid, self.vpn, self.ddns, self.v6_system]
            .iter()
            .all(Option::is_some)
    }

    /// The capabilities that differ from `other`, e.g. "raid: Some(false) -> Some(true)"
    pub fn changes(&self, other: &Capabilities) -> String {
        let fields = [
            ("connection", self.connection, other.connection),
            ("system", self.system, other.system),
            ("lan", self.lan, other.lan),
            ("lan_browser", self.lan_browser, other.lan_browser),
            ("switch", self.switch, other.switch),
            ("wifi", self.wifi, other.wifi),
            ("dhcp", self.dhcp, other.dhcp),
            ("nas", self.nas, other.nas),
            ("vm", self.vm, other.vm),
            ("raid", self.raid, other.raid),
            ("vpn", self.vpn, other.vpn),
            ("ddns", self.ddns, other.ddns),
            ("v6_system", self.v6_system, other.v6_system),
        ];
        let mut changes: Vec<String> = fields
            .iter()
            .filter(|(_, before, after)| before != after)
            .map(|(name, before, after)| format!("{name}: {before:?} -> {after:?}"))
            .collect();
        if self.api_version != other.api_version {
            changes.push(format!(
                "api_version: {:?} -> {:?}",
                self.api_version, other.api_version
            ));
        }
        if self.model != other.model {
            changes.push(format!("model: {:?} -> {:?}", self.model, other.model));
        }
        if self.device_name != other.device_name {
            changes.push(format!(
                "device_name: {:?} -> {:?}",
                self.device_name, other.device_name
            ));
        }
        if self.network_mode != other.network_mode {
            changes.push(format!(
                "network_mode: {:?} -> {:?}",
                self.network_mode, other.network_mode
            ));
        }
        changes.join(", ")
    }

    /// The capabilities that matter for `conf`: those of the collectors it does not enable are
    /// set to `Some(false)`, so that their change neither rebuilds the collectors nor keeps a
    /// detection pending
    pub fn for_configuration(&self, conf: &CapabilitiesConfiguration) -> Capabilities {
        let keep = |enabled: Option<bool>, capability: Option<bool>| {
            if enabled == Some(true) {
                capability
            } else {
                Some(false)
            }
        };
        Capabilities {
            connection: keep(conf.connection, self.connection),
            system: keep(conf.system, self.system),
            lan: keep(conf.lan, self.lan),
            lan_browser: keep(conf.lan_browser, self.lan_browser),
            switch: keep(conf.switch, self.switch),
            wifi: keep(conf.wifi, self.wifi),
            dhcp: keep(conf.dhcp, self.dhcp),
            nas: keep(conf.nas, self.nas),
            vm: keep(conf.vm, self.vm),
            raid: keep(conf.raid, self.raid),
            vpn: keep(conf.vpn, self.vpn),
            ddns: keep(conf.ddns, self.ddns),
            v6_system: keep(conf.system, self.v6_system),
            network_mode: self.network_mode.clone(),
            api_version: self.api_version.clone(),
            model: self.model.clone(),
            device_name: self.device_name.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::{matchers::path, Mock, ResponseTemplate};

    use super::*;
    use crate::mappers::test_support::{mock_api, serve};

    /// Loads capabilities of a box in `mode`, whose /api_version and v6/system/ responses are
    /// `api_version` and `v6_system`
    async fn load_responses(
        mode: &str,
        api_version: serde_json::Value,
        v6_system: serde_json::Value,
    ) -> Capabilities {
        let (server, authenticator) = mock_api().await;
        serve(
            &server,
            "/api/v4/lan/config",
            json!({ "success": true, "result": { "mode": mode } }),
        )
        .await;
        serve(
            &server,
            "/api/v4/wifi/config",
            json!({ "success": true, "result": { "enabled": true } }),
        )
        .await;
        serve(&server, "/api_version", api_version).await;
        serve(&server, "/api/v6/system/", v6_system).await;

        let factory = authenticator.login().await.unwrap();
        CapabilitiesAgent::new(&factory).load().await.unwrap()
    }

    /// Loads capabilities of a box in `mode` announcing `api_version`, whose v6/system/
    /// response is `system`
    async fn load_system(mode: &str, api_version: &str, system: serde_json::Value) -> Capabilities {
        load_responses(
            mode,
            json!({ "api_version": api_version }),
            json!({ "success": true, "result": system }),
        )
        .await
    }

    /// /api_version of a Freebox Delta
    fn delta_api_version() -> serde_json::Value {
        json!({
            "api_version": "16.1",
            "box_model": "fbxgw7-r1/full",
            "box_model_name": "Freebox v7 (r1)",
        })
    }

    fn delta_model() -> Option<system::BoxModel> {
        Some(system::BoxModel {
            model: Some("fbxgw7-r1/full".to_string()),
            model_name: Some("Freebox v7 (r1)".to_string()),
        })
    }

    /// Loads capabilities of a box in `mode` announcing `api_version`, with or without VM
    /// hardware, and listing its sensors
    async fn load_mode(mode: &str, api_version: &str, has_vm: bool) -> Capabilities {
        let system = json!({
            "model_info": { "has_vm": has_vm },
            "sensors": [{ "id": "temp_t1", "name": "Température 1", "value": 49 }],
        });
        load_system(mode, api_version, system).await
    }

    async fn load(api_version: &str, has_vm: bool) -> Capabilities {
        load_mode("router", api_version, has_vm).await
    }

    #[tokio::test]
    async fn the_identity_of_the_box_comes_from_api_version() {
        let ok = json!({ "success": true, "result": {} });
        // /api_version of a Freebox Delta; an unexpected type is ignored
        let api_version = json!({
            "api_version": "16.1",
            "box_model_name": "Freebox v7 (r1)",
            "device_name": 7,
        });
        let caps = load_responses("router", api_version, ok).await;

        assert_eq!(Some("16.1".to_string()), caps.api_version);
        assert_eq!(
            Some(system::BoxModel {
                model: None,
                model_name: Some("Freebox v7 (r1)".to_string()),
            }),
            caps.model
        );
        assert_eq!(None, caps.device_name);
        assert_eq!(Some(true), caps.raid, "the api version is still read");
    }

    #[tokio::test]
    async fn a_change_of_model_is_named() {
        let before = load("16.0", true).await;
        let mut after = before.clone();
        after.model = delta_model();

        assert_eq!(
            format!("model: None -> {:?}", delta_model()),
            before.changes(&after)
        );
    }

    #[tokio::test]
    async fn the_api_version_is_kept() {
        assert_eq!(
            Some("16.0".to_string()),
            load("16.0", true).await.api_version
        );
    }

    #[tokio::test]
    async fn only_the_capabilities_of_the_enabled_collectors_count() {
        let caps = load("16.0", true).await;
        let conf: CapabilitiesConfiguration =
            toml::from_str("system = true\nwifi = true\nprefix = \"test\"").unwrap();

        let relevant = caps.for_configuration(&conf);

        // enabled: as detected
        assert_eq!(caps.system, relevant.system);
        assert_eq!(caps.wifi, relevant.wifi);
        assert_eq!(caps.v6_system, relevant.v6_system);
        // not enabled: the same whatever the box answers
        for (name, value) in [
            ("vm", relevant.vm),
            ("raid", relevant.raid),
            ("vpn", relevant.vpn),
            ("ddns", relevant.ddns),
            ("switch", relevant.switch),
        ] {
            assert_eq!(Some(false), value, "{name}");
        }
        let mut other = caps.clone();
        other.raid = Some(false);
        other.vm = None;
        assert_eq!(relevant, other.for_configuration(&conf));
        assert!(relevant.is_complete());
    }

    /// the identity of the box is exposed by the system collector, whatever the configuration:
    /// it is kept as detected
    #[tokio::test]
    async fn the_identity_of_the_box_is_kept_for_the_configuration() {
        let ok = json!({ "success": true, "result": {} });
        let mut api_version = delta_api_version();
        api_version["device_name"] = json!("Freebox Server");
        let caps = load_responses("router", api_version, ok).await;
        let conf: CapabilitiesConfiguration =
            toml::from_str("system = true\nprefix = \"test\"").unwrap();

        let relevant = caps.for_configuration(&conf);

        assert_eq!(Some("16.1".to_string()), relevant.api_version);
        assert_eq!(delta_model(), relevant.model);
        assert_eq!(Some("Freebox Server".to_string()), relevant.device_name);
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
    async fn the_model_comes_from_api_version() {
        // a v6/system/ without sensors list: the model does not depend on it
        let ok = json!({ "success": true, "result": {} });
        let caps = load_responses("router", delta_api_version(), ok).await;
        assert_eq!(caps.model, delta_model());
        assert_eq!(caps.v6_system, Some(false));
    }

    #[tokio::test]
    async fn the_model_is_kept_when_the_system_probe_fails() {
        let failed = json!({ "success": false, "error_code": "internal_error" });
        let caps = load_responses("router", delta_api_version(), failed).await;
        assert_eq!(caps.model, delta_model());
        assert_eq!(caps.v6_system, None);
    }

    #[tokio::test]
    async fn no_model_when_api_version_does_not_tell() {
        let ok = json!({ "success": true, "result": {} });
        let caps = load_responses("router", json!({ "api_version": "16.1" }), ok).await;
        assert_eq!(caps.model, None);
    }

    #[tokio::test]
    async fn an_unexpected_model_leaves_the_api_version_readable() {
        let ok = json!({ "success": true, "result": {} });
        let api_version = json!({ "api_version": "16.1", "box_model": 7, "box_model_name": "" });
        let caps = load_responses("router", api_version, ok).await;
        assert_eq!(caps.model, None);
        assert_eq!(caps.raid, Some(true));
    }

    #[tokio::test]
    async fn v6_system_requires_api_6() {
        assert_eq!(Some(false), load("5.0", false).await.v6_system);
        assert_eq!(Some(true), load("6.0", false).await.v6_system);
    }

    #[tokio::test]
    async fn v6_system_requires_the_sensors_list() {
        // a box with API 16.0, but without sensors in v6/system/, keeps the temperatures
        // of v4/system
        let system = json!({ "model_info": { "has_vm": true } });
        let caps = load_system("router", "16.0", system).await;
        assert_eq!(Some(false), caps.v6_system);
        assert_eq!(Some(true), caps.vm);
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
        assert_eq!(None, caps.v6_system);
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
    async fn failing_model_info_probe_leaves_vm_and_v6_system_unknown_only() {
        let (server, authenticator) = mock_api().await;
        serve(&server, "/api/v4/lan/config", json!({ "success": true, "result": { "mode": "router" } })).await;
        serve(&server, "/api/v4/wifi/config", json!({ "success": true, "result": { "enabled": true } })).await;
        serve(&server, "/api_version", json!({ "api_version": "16.0" })).await;
        serve(&server, "/api/v6/system/", json!({ "success": false, "error_code": "internal_error" })).await;

        let factory = authenticator.login().await.unwrap();
        let caps = CapabilitiesAgent::new(&factory).load().await.unwrap();

        assert_eq!(None, caps.vm);
        assert_eq!(None, caps.v6_system);
        assert_eq!(Some(true), caps.raid);
        assert_eq!(Some(true), caps.vpn);
        assert_eq!(Some(true), caps.ddns);
    }
}
