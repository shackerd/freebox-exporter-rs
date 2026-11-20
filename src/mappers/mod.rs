use async_trait::async_trait;
use chrono::Duration;
use connection::ConnectionMetricMap;
use ddns::DdnsMetricMap;
use download::DownloadMetricMap;
use health::CollectorHealth;
use lan::LanMetricMap;
use lanbrowser::LanBrowserMetricMap;
use log::{error, warn};
use nas::NasMetricMap;
use prometheus_exporter::{prometheus::Registry, UpdateLock};

use crate::core::redetection::CycleOutcome;
use raid::RaidMetricMap;
use switch::SwitchMetricMap;
use system::SystemMetricMap;
use vm::VmMetricMap;
use vpn::VpnMetricMap;

use crate::core::{
    capabilities::Capabilities,
    common::http_client_factory::AuthenticatedHttpClientFactory,
    configuration::sections::{ApiConfiguration, CapabilitiesConfiguration, PoliciesConfiguration},
};

pub mod connection;
pub mod ddns;
pub mod dhcp;
pub mod download;
pub mod health;
pub mod lan;
pub mod lanbrowser;
pub mod nas;
pub mod raid;
pub mod switch;
pub mod system;
#[cfg(test)]
pub mod test_support;
pub mod vm;
pub mod vpn;
pub mod wifi;

#[async_trait]
pub trait MetricMap<'a> {
    /// Collects the values from the box, then updates the metrics while holding `lock`,
    /// the lock the exporter takes to return them
    async fn set(
        &mut self,
        lock: &UpdateLock,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// A collector with the name used as `collector` label in health metrics
type NamedMetricMap<'a> = (&'static str, Box<dyn MetricMap<'a> + 'a>);

pub struct Mapper<'a> {
    maps: Vec<NamedMetricMap<'a>>,
    health: CollectorHealth,
    /// Checked after a failed collection, to log in again when the box restarted
    session: Option<&'a AuthenticatedHttpClientFactory<'a>>,
    /// Collectors that failed in the last run
    failed: Vec<&'static str>,
}

impl<'a> Mapper<'a> {
    pub fn new(
        factory: &'a AuthenticatedHttpClientFactory<'a>,
        conf: CapabilitiesConfiguration,
        caps: Capabilities,
        api_conf: ApiConfiguration,
        policies: Option<PoliciesConfiguration>,
        registry: &Registry,
    ) -> Self {
        let mut maps: Vec<NamedMetricMap<'a>> = vec![];

        if let Some(e) = conf.connection {
            if e {
                maps.push((
                    "connection",
                    Box::new(ConnectionMetricMap::new_in(
                        factory,
                        conf.prefix.to_owned().unwrap(),
                        registry,
                    )),
                ));
            }
        } else {
            warn!(
                "Connection metrics are disabled by default, missing entry in the configuration file"
            );
        }
        if let Some(e) = conf.system {
            if e {
                maps.push((
                    "system",
                    Box::new(
                        SystemMetricMap::new_in(factory, conf.prefix.to_owned().unwrap(), registry)
                            .with_v6_system(caps.v6_system.unwrap_or(false))
                            .with_identity(
                                caps.api_version.clone(),
                                caps.box_model_name.clone(),
                                caps.device_name.clone(),
                            ),
                    ),
                ));
            }
        } else {
            warn!(
                "System metrics are disabled by default, missing entry in the configuration file"
            );
        }

        if let Some(e) = conf.lan {
            if e {
                maps.push((
                    "lan",
                    Box::new(LanMetricMap::new_in(
                        factory,
                        conf.prefix.to_owned().unwrap(),
                        registry,
                    )),
                ));
            }
        } else {
            warn!("LAN metrics are disabled by default, missing entry in the configuration file");
        }

        let network_mode = caps.network_mode.unwrap_or_default();

        if let Some(e) = conf.lan_browser {
            if e {
                if !caps.lan_browser.unwrap_or(false) {
                    warn!("lan_browser is incompatible with detected freebox mode ({}), the option has been disabled", network_mode);
                } else {
                    let lan_browser_map = LanBrowserMetricMap::new_in(
                        factory,
                        conf.prefix.to_owned().unwrap(),
                        registry,
                    );
                    maps.push(("lan_browser", Box::new(lan_browser_map)));
                }
            }
        } else {
            warn!("LAN browser metrics are disabled by default, missing entry in the configuration file");
        }

        if let Some(e) = conf.switch {
            if e {
                if !caps.switch.unwrap_or(false) {
                    warn!("switch is incompatible with detected freebox mode ({}), the option has been disabled", network_mode);
                } else {
                    maps.push((
                        "switch",
                        Box::new(SwitchMetricMap::new_in(
                            factory,
                            conf.prefix.to_owned().unwrap(),
                            registry,
                        )),
                    ));
                }
            }
        } else {
            warn!(
                "Switch metrics are disabled by default, missing entry in the configuration file"
            );
        }

        if let Some(e) = conf.wifi {
            if e {
                if !caps.wifi.unwrap_or(false) {
                    warn!("wifi is either disabled on the host or has been explicitly enabled with an incompatible network mode ({}). The option has been automatically disabled", network_mode);
                } else {
                    // Provide default policies if none specified
                    let default_policies = PoliciesConfiguration {
                        unresolved_station_hostnames: Some("ignore".to_string()),
                    };
                    let wifi_policies = policies.as_ref().unwrap_or(&default_policies);

                    let wifi_map = wifi::WifiMetricMap::new_in(
                        factory,
                        conf.prefix.to_owned().unwrap(),
                        Duration::seconds(api_conf.refresh.unwrap_or(5) as i64),
                        wifi_policies,
                        registry,
                    );
                    maps.push(("wifi", Box::new(wifi_map)));
                }
            }
        } else {
            warn!("Wifi metrics are disabled by default, missing entry in the configuration file");
        }

        if let Some(e) = conf.dhcp {
            if e {
                if !caps.dhcp.unwrap_or(false) {
                    warn!("dhcp is incompatible with detected freebox mode ({}), the option has been disabled", network_mode);
                } else {
                    maps.push((
                        "dhcp",
                        Box::new(dhcp::DhcpMetricMap::new_in(
                            factory,
                            conf.prefix.to_owned().unwrap(),
                            registry,
                        )),
                    ));
                }
            }
        } else {
            warn!("DHCP metrics are disabled by default, missing entry in the configuration file");
        }

        if let Some(e) = conf.nas {
            if e {
                if !caps.nas.unwrap_or(false) {
                    warn!("NAS is incompatible with detected freebox mode ({}), the option has been disabled", network_mode);
                } else {
                    maps.push((
                        "nas",
                        Box::new(NasMetricMap::new_in(
                            factory,
                            conf.prefix.to_owned().unwrap(),
                            registry,
                        )),
                    ));
                }
            }
        } else {
            warn!("NAS metrics are disabled by default, missing entry in the configuration file");
        }

        if let Some(e) = conf.download {
            if e {
                // The Freebox downloads stats endpoint is available across supported modes.
                maps.push((
                    "download",
                    Box::new(DownloadMetricMap::new_in(
                        factory,
                        conf.prefix.to_owned().unwrap(),
                        registry,
                    )),
                ));
            }
        } else {
            warn!(
                "Download metrics are disabled by default, missing entry in the configuration file"
            );
        }

        // collectors requested but whose support is unknown, exposed as failing
        let mut undetected: Vec<&'static str> = vec![];

        if let Some(e) = conf.vm {
            if e {
                match caps.vm {
                    Some(true) => maps.push((
                        "vm",
                        Box::new(VmMetricMap::new_in(
                            factory,
                            conf.prefix.to_owned().unwrap(),
                            registry,
                        )),
                    )),
                    Some(false) => warn!(
                        "vm is not supported by this freebox (requires API v{} and VM capable hardware), the option has been disabled",
                        vm::MIN_API_VERSION
                    ),
                    None => {
                        error!("vm support could not be detected at startup, the option is disabled until the capabilities are detected again");
                        undetected.push("vm");
                    }
                }
            }
        } else {
            warn!("VM metrics are disabled by default, missing entry in the configuration file");
        }

        if let Some(e) = conf.raid {
            if e {
                match caps.raid {
                    Some(true) => maps.push((
                        "raid",
                        Box::new(RaidMetricMap::new_in(
                            factory,
                            conf.prefix.to_owned().unwrap(),
                            registry,
                        )),
                    )),
                    Some(false) => warn!(
                        "raid is not supported by this freebox (requires API v{}), the option has been disabled",
                        raid::MIN_API_VERSION
                    ),
                    None => {
                        error!("raid support could not be detected at startup, the option is disabled until the capabilities are detected again");
                        undetected.push("raid");
                    }
                }
            }
        } else {
            warn!("RAID metrics are disabled by default, missing entry in the configuration file");
        }

        if let Some(e) = conf.vpn {
            if e {
                match caps.vpn {
                    Some(true) => maps.push((
                        "vpn",
                        Box::new(VpnMetricMap::new_in(
                            factory,
                            conf.prefix.to_owned().unwrap(),
                            registry,
                        )),
                    )),
                    Some(false) => warn!(
                        "vpn is not supported by this freebox (requires API v{} in router mode), the option has been disabled",
                        vpn::MIN_API_VERSION
                    ),
                    None => {
                        error!("vpn support could not be detected at startup, the option is disabled until the capabilities are detected again");
                        undetected.push("vpn");
                    }
                }
            }
        } else {
            warn!("VPN metrics are disabled by default, missing entry in the configuration file");
        }

        if let Some(e) = conf.ddns {
            if e {
                match caps.ddns {
                    Some(true) => maps.push((
                        "ddns",
                        Box::new(DdnsMetricMap::new_in(
                            factory,
                            conf.prefix.to_owned().unwrap(),
                            registry,
                        )),
                    )),
                    Some(false) => warn!(
                        "ddns is not supported by this freebox (requires API v{} in router mode), the option has been disabled",
                        ddns::MIN_API_VERSION
                    ),
                    None => {
                        error!("ddns support could not be detected at startup, the option is disabled until the capabilities are detected again");
                        undetected.push("ddns");
                    }
                }
            }
        } else {
            warn!(
                "DynDNS metrics are disabled by default, missing entry in the configuration file"
            );
        }

        let health = CollectorHealth::register_in(conf.prefix.as_deref().unwrap(), registry);
        for name in undetected {
            health.observe(
                name,
                false,
                std::time::Duration::ZERO,
                std::time::SystemTime::now(),
            );
        }

        Self {
            maps,
            health,
            session: Some(factory),
            failed: vec![],
        }
    }

    /// Initializes every collector; a failing init is exposed as an unsuccessful run,
    /// a successful one is not, since no data has been collected yet
    pub async fn init_all(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for (name, map) in self.maps.iter_mut() {
            let started = std::time::Instant::now();

            if let Err(e) = map.init().await {
                error!("collector {name} failed to initialize: {e}");
                self.health
                    .observe(name, false, started.elapsed(), std::time::SystemTime::now());
            }
        }
        Ok(())
    }

    /// Collectors that failed in the last run
    pub fn failed_collectors(&self) -> &[&'static str] {
        &self.failed
    }

    /// Collectors that did not fail in the last run
    pub fn working_collectors(&self) -> Vec<&'static str> {
        self.maps
            .iter()
            .map(|(name, _)| *name)
            .filter(|name| !self.failed.contains(name))
            .collect()
    }

    /// error_code of the login while the box refuses it, e.g. invalid_token for a revoked app
    pub fn login_refusal(&self) -> Option<String> {
        self.session.and_then(|factory| factory.login_refusal())
    }

    /// Runs every collector; a failing collector does not stop the others nor the exporter,
    /// its outcome is exposed through the health metrics
    pub async fn set_all(
        &mut self,
        lock: &UpdateLock,
    ) -> Result<CycleOutcome, Box<dyn std::error::Error + Send + Sync>> {
        let mut outcome = CycleOutcome::default();
        self.failed.clear();
        for (name, map) in self.maps.iter_mut() {
            let started = std::time::Instant::now();
            let res = map.set(lock).await;

            // while the box refuses the login, the factory attempts one login per period and
            // fails the others at once: the other collectors would fail alike
            if self
                .session
                .is_some_and(|factory| factory.login_refusal().is_some())
            {
                return Ok(CycleOutcome::default());
            }

            if let Err(e) = &res {
                error!("collector {name} failed: {e}");
                outcome.failed += 1;
                self.failed.push(name);
            } else {
                outcome.succeeded += 1;
            }

            // a scrape waits for the health update to complete instead of returning part of it
            let _update = lock.lock();
            self.health.observe(
                name,
                res.is_ok(),
                started.elapsed(),
                std::time::SystemTime::now(),
            );
        }

        // a restarted box rejects the previous sessions: one request, after a failure only
        if let Some(factory) = self.session.filter(|_| outcome.failed > 0) {
            if factory.expire_clients_if_session_lost().await {
                warn!("the box no longer knows the session, probably after a restart: logging in again");
                outcome.session_lost = true;
            }
        }

        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use super::*;
    use crate::mappers::test_support::{
        collect_while_locked, new_update_lock, scrape, update_lock,
    };

    /// Collector returning a scripted sequence of outcomes and counting its runs
    struct ScriptedMap {
        init_fails: bool,
        outcomes: VecDeque<bool>,
        runs: Arc<Mutex<u32>>,
        inits: Arc<Mutex<u32>>,
    }

    impl ScriptedMap {
        fn new(outcomes: &[bool]) -> (Self, Arc<Mutex<u32>>) {
            let runs = Arc::new(Mutex::new(0));
            let map = Self {
                init_fails: false,
                outcomes: outcomes.iter().copied().collect(),
                runs: runs.clone(),
                inits: Arc::new(Mutex::new(0)),
            };
            (map, runs)
        }
    }

    #[async_trait]
    impl<'a> MetricMap<'a> for ScriptedMap {
        async fn set(
            &mut self,
            _lock: &UpdateLock,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            *self.runs.lock().unwrap() += 1;
            match self.outcomes.pop_front() {
                Some(true) => Ok(()),
                _ => Err("scripted failure".into()),
            }
        }

        async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            *self.inits.lock().unwrap() += 1;
            match self.init_fails {
                true => Err("scripted init failure".into()),
                false => Ok(()),
            }
        }
    }

    fn mapper<'a>(maps: Vec<NamedMetricMap<'a>>) -> Mapper<'a> {
        // unregistered metrics: tests must not collide in the global registry
        Mapper {
            maps,
            health: CollectorHealth::new("test"),
            session: None,
            failed: vec![],
        }
    }

    #[tokio::test]
    async fn requested_collector_with_undetected_support_is_exposed_as_failing() {
        let (_server, authenticator) = crate::mappers::test_support::mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let conf: CapabilitiesConfiguration = toml::from_str(
            "vm = true\nraid = true\nvpn = true\nddns = true\nprefix = \"test_mapper_undetected\"",
        )
        .unwrap();
        let caps: Capabilities =
            serde_json::from_str(r#"{ "vm": null, "raid": null, "vpn": null, "ddns": null }"#)
                .unwrap();

        let mapper = Mapper::new(
            &factory,
            conf,
            caps,
            ApiConfiguration {
                refresh: Some(5),
                connect_timeout: None,
                request_timeout: None,
                address: None,
            },
            None,
            prometheus_exporter::prometheus::default_registry(),
        );

        assert!(mapper.maps.is_empty());
        for name in ["vm", "raid", "vpn", "ddns"] {
            assert_eq!(Some(0.0), mapper.health.success(name), "{name}");
            assert_eq!(None, mapper.health.last_success(name), "{name}");
        }
    }

    const NEW_COLLECTORS_CONF: &str = "vm = true\nraid = true\nvpn = true\nddns = true\n";

    #[tokio::test]
    async fn requested_collectors_are_created_only_when_supported() {
        let (_server, authenticator) = crate::mappers::test_support::mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let conf: CapabilitiesConfiguration = toml::from_str(&format!(
            "{NEW_COLLECTORS_CONF}prefix = \"test_mapper_supported\""
        ))
        .unwrap();
        let caps: Capabilities =
            serde_json::from_str(r#"{ "vm": true, "raid": true, "vpn": true, "ddns": true }"#)
                .unwrap();

        let mapper = Mapper::new(
            &factory,
            conf,
            caps,
            ApiConfiguration {
                refresh: Some(5),
                connect_timeout: None,
                request_timeout: None,
                address: None,
            },
            None,
            prometheus_exporter::prometheus::default_registry(),
        );

        let names: Vec<&str> = mapper.maps.iter().map(|(name, _)| *name).collect();
        assert_eq!(vec!["vm", "raid", "vpn", "ddns"], names);
    }

    #[tokio::test]
    async fn the_system_collector_exposes_the_identity_read_from_api_version() {
        use crate::mappers::test_support::{scrape, serve, update_lock};

        let (server, authenticator) = crate::mappers::test_support::mock_api().await;
        // v4/system of a Freebox Delta: no api_version, box_model_name nor device_name
        serve(
            &server,
            "/api/v4/system",
            serde_json::json!({ "success": true, "result": { "uptime_val": 1 } }),
        )
        .await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_mapper_identity";
        let conf: CapabilitiesConfiguration =
            toml::from_str(&format!("system = true\nprefix = \"{prefix}\"")).unwrap();
        let caps: Capabilities = serde_json::from_str(
            r#"{ "system": true, "api_version": "16.1",
                 "box_model_name": "Freebox v7 (r1)", "device_name": "Freebox Server" }"#,
        )
        .unwrap();

        let mut mapper = Mapper::new(
            &factory,
            conf,
            caps,
            ApiConfiguration {
                refresh: Some(5),
                connect_timeout: None,
                request_timeout: None,
                address: None,
            },
            None,
            prometheus_exporter::prometheus::default_registry(),
        );
        mapper.set_all(update_lock()).await.unwrap();

        let metrics = scrape(prefix);
        for line in [
            format!("{prefix}_system_api_version{{api_version=\"16.1\"}} 1"),
            format!("{prefix}_system_box_model_name{{model_name=\"Freebox v7 (r1)\"}} 1"),
            format!("{prefix}_system_device_name{{device_name=\"Freebox Server\"}} 1"),
        ] {
            assert!(metrics.contains(&line), "{line} in {metrics:?}");
        }
    }

    #[tokio::test]
    async fn requested_collectors_are_skipped_silently_when_unsupported() {
        let (_server, authenticator) = crate::mappers::test_support::mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let conf: CapabilitiesConfiguration = toml::from_str(&format!(
            "{NEW_COLLECTORS_CONF}prefix = \"test_mapper_unsupported\""
        ))
        .unwrap();
        let caps: Capabilities =
            serde_json::from_str(r#"{ "vm": false, "raid": false, "vpn": false, "ddns": false }"#)
                .unwrap();

        let mapper = Mapper::new(
            &factory,
            conf,
            caps,
            ApiConfiguration {
                refresh: Some(5),
                connect_timeout: None,
                request_timeout: None,
                address: None,
            },
            None,
            prometheus_exporter::prometheus::default_registry(),
        );

        assert!(mapper.maps.is_empty());
        // unlike undetected support, a known lack of support is not a failure
        for name in ["vm", "raid", "vpn", "ddns"] {
            assert_eq!(None, mapper.health.success(name), "{name}");
        }
    }

    #[tokio::test]
    async fn set_all_exposes_success_and_failure_per_collector() {
        let (ok, _) = ScriptedMap::new(&[true]);
        let (ko, _) = ScriptedMap::new(&[false]);
        let mut mapper = mapper(vec![("ok", Box::new(ok)), ("ko", Box::new(ko))]);

        assert!(mapper.set_all(update_lock()).await.is_ok());

        assert_eq!(Some(1.0), mapper.health.success("ok"));
        assert_eq!(Some(0.0), mapper.health.success("ko"));
        assert!(mapper.health.last_success("ok").unwrap() > 0.0);
        assert_eq!(None, mapper.health.last_success("ko"));
    }

    #[tokio::test]
    async fn health_update_waits_for_a_scrape_in_progress() {
        let prefix = "test_mapper_lock";
        let (map, _) = ScriptedMap::new(&[true, false]);
        let mut mapper = Mapper {
            maps: vec![("scripted", Box::new(map))],
            health: CollectorHealth::register(prefix),
            session: None,
            failed: vec![],
        };

        mapper.set_all(update_lock()).await.unwrap();
        let before = scrape(prefix);
        assert!(
            before
                .iter()
                .any(|l| l.contains("_collector_success{") && l.ends_with(" 1")),
            "{before:?}"
        );

        // the collector now fails
        let lock = new_update_lock();
        let (result, during) = collect_while_locked(&lock, mapper.set_all(&lock), prefix).await;
        result.unwrap();

        assert_eq!(before, during);
        assert_eq!(Some(0.0), mapper.health.success("scripted"));
    }

    #[tokio::test]
    async fn set_all_keeps_last_success_timestamp_when_collector_starts_failing() {
        let (flaky, _) = ScriptedMap::new(&[true, false]);
        let mut mapper = mapper(vec![("flaky", Box::new(flaky))]);

        mapper.set_all(update_lock()).await.unwrap();
        let first_success = mapper.health.last_success("flaky");
        mapper.set_all(update_lock()).await.unwrap();

        assert_eq!(Some(0.0), mapper.health.success("flaky"));
        assert!(first_success.is_some());
        assert_eq!(first_success, mapper.health.last_success("flaky"));
    }

    #[tokio::test]
    async fn set_all_runs_remaining_collectors_after_a_failure() {
        let (ko, ko_runs) = ScriptedMap::new(&[false]);
        let (ok, ok_runs) = ScriptedMap::new(&[true]);
        let mut mapper = mapper(vec![("ko", Box::new(ko)), ("ok", Box::new(ok))]);

        mapper.set_all(update_lock()).await.unwrap();

        assert_eq!(1, *ko_runs.lock().unwrap());
        assert_eq!(1, *ok_runs.lock().unwrap());
        assert_eq!(Some(1.0), mapper.health.success("ok"));
    }

    #[tokio::test]
    async fn init_all_exposes_init_failure_only() {
        let (mut broken, _) = ScriptedMap::new(&[]);
        broken.init_fails = true;
        let (healthy, _) = ScriptedMap::new(&[]);
        let healthy_inits = healthy.inits.clone();
        let mut mapper = mapper(vec![
            ("broken", Box::new(broken)),
            ("healthy", Box::new(healthy)),
        ]);

        assert!(mapper.init_all().await.is_ok());

        assert_eq!(1, *healthy_inits.lock().unwrap());
        assert_eq!(Some(0.0), mapper.health.success("broken"));
        assert_eq!(None, mapper.health.success("healthy"));
        assert_eq!(None, mapper.health.last_success("healthy"));
    }

    fn existing_collectors<'a>(
        factory: &'a AuthenticatedHttpClientFactory<'a>,
        prefix: &str,
    ) -> Vec<NamedMetricMap<'a>> {
        let prefix = prefix.to_string();
        vec![
            (
                "connection",
                Box::new(ConnectionMetricMap::new(factory, prefix.clone())),
            ),
            (
                "system",
                Box::new(SystemMetricMap::new(factory, prefix.clone())),
            ),
            ("lan", Box::new(LanMetricMap::new(factory, prefix.clone()))),
            (
                "lan_browser",
                Box::new(LanBrowserMetricMap::new(factory, prefix.clone())),
            ),
            (
                "switch",
                Box::new(SwitchMetricMap::new(factory, prefix.clone())),
            ),
            (
                "dhcp",
                Box::new(dhcp::DhcpMetricMap::new(factory, prefix.clone())),
            ),
            ("nas", Box::new(NasMetricMap::new(factory, prefix.clone()))),
            (
                "download",
                Box::new(DownloadMetricMap::new(factory, prefix)),
            ),
        ]
    }

    #[tokio::test]
    async fn collectors_report_the_error_of_the_box() {
        use serde_json::json;
        use wiremock::{matchers::method, Mock, ResponseTemplate};

        let (server, authenticator) = crate::mappers::test_support::mock_api().await;
        // every request but the login is refused, with the login challenge as result, as by a
        // box which no longer knows the session
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({
                "success": false, "error_code": "auth_required", "msg": "login",
                "result": { "password_salt": "fake", "challenge": "fake" },
            })))
            .with_priority(10)
            .mount(&server)
            .await;
        let factory = authenticator.login().await.unwrap();
        let prefix = "test_box_error";
        let policies = PoliciesConfiguration {
            unresolved_station_hostnames: None,
        };
        let mut collectors = existing_collectors(&factory, prefix);
        // connection names its errors in its own change
        collectors.retain(|(name, _)| *name != "connection");
        collectors.push((
            "wifi",
            Box::new(wifi::WifiMetricMap::new(
                &factory,
                prefix.to_string(),
                Duration::seconds(5),
                &policies,
            )),
        ));

        for (name, mut map) in collectors {
            let error = map.set(update_lock()).await.expect_err(name).to_string();
            assert!(
                error.contains(" failed: auth_required (login)"),
                "{name}: {error}"
            );
        }
    }

    #[tokio::test]
    async fn existing_collectors_return_an_error_when_the_renewed_client_is_expired() {
        let (_server, authenticator) = crate::mappers::test_support::mock_api().await;
        let mut factory = authenticator.login().await.unwrap();
        // every managed client is created already expired
        factory.expiration = chrono::TimeDelta::seconds(-1);

        // a panic here would stop every collector of the exporter
        for (name, mut map) in existing_collectors(&factory, "test_expired_client") {
            assert!(
                map.set(update_lock()).await.is_err(),
                "{name} succeeded with an expired client"
            );
        }
    }

    #[tokio::test]
    async fn existing_collectors_return_an_error_when_the_session_cannot_be_renewed() {
        use serde_json::json;
        use wiremock::{
            matchers::{method, path},
            Mock, ResponseTemplate,
        };

        let (server, authenticator) = crate::mappers::test_support::mock_api().await;
        let mut factory = authenticator.login().await.unwrap();
        // the client expires at once, so it is renewed right after its creation
        factory.expiration = chrono::TimeDelta::seconds(-1);

        for (name, mut map) in existing_collectors(&factory, "test_failed_renewal") {
            // the box accepts the first login of the collector, then refuses the renewal
            server.reset().await;
            Mock::given(path("/api/v4/login/"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(
                        json!({ "success": true, "result": { "challenge": "1234" } }),
                    ),
                )
                .up_to_n_times(1)
                .with_priority(1)
                .mount(&server)
                .await;
            Mock::given(path("/api/v4/login/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    json!({ "success": false, "error_code": "internal_error", "msg": "boom" }),
                ))
                .with_priority(2)
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path("/api/v4/login/session"))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    json!({ "success": true, "result": { "session_token": "4321" } }),
                ))
                .mount(&server)
                .await;

            assert!(
                map.set(update_lock()).await.is_err(),
                "{name} succeeded without session"
            );
            let challenges = server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .filter(|r| r.url.path() == "/api/v4/login/")
                .count();
            assert_eq!(2, challenges, "{name} did not try to renew its session");
        }
    }

    #[tokio::test]
    async fn slow_session_renewal_times_out_instead_of_stalling_collection() {
        use serde_json::json;
        use wiremock::{
            matchers::{method, path},
            Mock, ResponseTemplate,
        };

        let (server, authenticator) = crate::mappers::test_support::mock_api().await;
        let mut factory = authenticator.login().await.unwrap();
        factory.set_request_timeout(std::time::Duration::from_millis(200));
        // the client expires at once, so it is renewed right after its creation
        factory.expiration = chrono::TimeDelta::seconds(-1);
        // the box answers the first login challenge, then takes 5 seconds
        server.reset().await;
        let challenge = json!({ "success": true, "result": { "challenge": "1234" } });
        Mock::given(path("/api/v4/login/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(challenge.clone()))
            .up_to_n_times(1)
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(path("/api/v4/login/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(challenge)
                    .set_delay(std::time::Duration::from_secs(5)),
            )
            .with_priority(2)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v4/login/session"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    json!({ "success": true, "result": { "session_token": "4321" } }),
                ),
            )
            .mount(&server)
            .await;
        let mut map = LanMetricMap::new(&factory, "test_slow_renewal".to_string());

        let started = std::time::Instant::now();
        assert!(map.set(update_lock()).await.is_err());
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(
            2,
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .filter(|r| r.url.path() == "/api/v4/login/")
                .count()
        );
    }
}

#[cfg(test)]
mod api_specs_provider {
    /// Get the API specs data from the file system
    pub async fn get_specs_data(
        api: &'static str,
        endpoint: &'static str,
    ) -> Result<String, std::io::Error> {
        let path = format!("src/mappers/specs-data/{}/{}.json", api, endpoint);
        tokio::fs::read_to_string(path).await
    }
}

#[cfg(test)]
mod session_renewal {
    use std::{
        collections::HashSet,
        sync::{Arc, Mutex},
    };

    use serde_json::json;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, Request, ResponseTemplate,
    };

    use super::*;
    use crate::{
        core::authenticator::{
            application_token_provider::MockApplicationTokenProvider, Authenticator,
        },
        mappers::test_support::update_lock,
    };

    /// A box keeping the sessions it gave, and forgetting them all when it restarts
    #[derive(Clone, Default)]
    struct FakeBox {
        sessions: Arc<Mutex<HashSet<String>>>,
        logins: Arc<Mutex<u32>>,
        /// GET v4/login/ with a session token, to check it
        checks: Arc<Mutex<u32>>,
        /// v4/lan/config fails for another reason than the session
        lan_down: Arc<Mutex<bool>>,
        /// error_code of every login while the box refuses it, e.g. after the app was revoked
        refusal: Arc<Mutex<Option<&'static str>>>,
        /// POST v4/login/session, accepted or refused
        attempts: Arc<Mutex<u32>>,
    }

    impl FakeBox {
        fn restart(&self) {
            self.sessions.lock().unwrap().clear();
        }

        fn knows(&self, request: &Request) -> bool {
            request
                .headers
                .get("X-Fbx-App-Auth")
                .and_then(|token| token.to_str().ok())
                .is_some_and(|token| self.sessions.lock().unwrap().contains(token))
        }

        async fn serve(&self) -> (MockServer, Authenticator) {
            let server = MockServer::start().await;

            let fake = self.clone();
            Mock::given(method("GET"))
                .and(path("/api/v4/login/"))
                .respond_with(move |request: &Request| {
                    if request.headers.contains_key("X-Fbx-App-Auth") {
                        *fake.checks.lock().unwrap() += 1;
                    }
                    ResponseTemplate::new(200).set_body_json(json!({
                        "success": true,
                        "result": { "logged_in": fake.knows(request), "challenge": "1234" },
                    }))
                })
                .mount(&server)
                .await;

            let fake = self.clone();
            Mock::given(method("POST"))
                .and(path("/api/v4/login/session"))
                .respond_with(move |_: &Request| {
                    *fake.attempts.lock().unwrap() += 1;
                    if let Some(code) = *fake.refusal.lock().unwrap() {
                        return ResponseTemplate::new(403).set_body_json(json!({
                            "success": false, "error_code": code, "msg": "refused",
                        }));
                    }
                    let mut logins = fake.logins.lock().unwrap();
                    *logins += 1;
                    let token = format!("session-{logins}");
                    fake.sessions.lock().unwrap().insert(token.clone());
                    ResponseTemplate::new(200).set_body_json(
                        json!({ "success": true, "result": { "session_token": token } }),
                    )
                })
                .mount(&server)
                .await;

            let fake = self.clone();
            Mock::given(method("GET"))
                .and(path("/api/v4/lan/config"))
                .respond_with(move |request: &Request| {
                    if !fake.knows(request) {
                        return ResponseTemplate::new(403).set_body_json(json!({
                            "success": false, "error_code": "auth_required",
                            "msg": "Vous devez vous connecter pour accéder à cette fonction",
                        }));
                    }
                    if *fake.lan_down.lock().unwrap() {
                        return ResponseTemplate::new(500);
                    }
                    ResponseTemplate::new(200).set_body_json(json!({
                        "success": true, "result": { "name": "fake", "mode": "router" },
                    }))
                })
                .mount(&server)
                .await;

            let mut store = MockApplicationTokenProvider::new();
            store.expect_get().returning(|| Ok("foo.bar".to_string()));
            let authenticator =
                Authenticator::new(format!("{}/api/", server.uri()), Box::new(store));
            (server, authenticator)
        }

        fn logins(&self) -> u32 {
            *self.logins.lock().unwrap()
        }

        fn checks(&self) -> u32 {
            *self.checks.lock().unwrap()
        }

        fn attempts(&self) -> u32 {
            *self.attempts.lock().unwrap()
        }
    }

    /// Collector succeeding every time, counting its runs
    struct CountingMap(Arc<Mutex<u32>>);

    #[async_trait]
    impl<'a> MetricMap<'a> for CountingMap {
        async fn set(
            &mut self,
            _: &UpdateLock,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            *self.0.lock().unwrap() += 1;
            Ok(())
        }

        async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
    }

    fn mapper<'a>(factory: &'a AuthenticatedHttpClientFactory<'a>, prefix: &str) -> Mapper<'a> {
        Mapper {
            maps: vec![(
                "lan",
                Box::new(LanMetricMap::new(factory, prefix.to_string())),
            )],
            health: CollectorHealth::new("test"),
            session: Some(factory),
            failed: vec![],
        }
    }

    #[tokio::test]
    async fn collectors_log_in_again_when_the_box_has_forgotten_the_session() {
        let fake = FakeBox::default();
        let (_server, authenticator) = fake.serve().await;
        let factory = authenticator.login().await.unwrap();
        let mut mapper = mapper(&factory, "test_session_restart");

        let outcome = mapper.set_all(update_lock()).await.unwrap();
        assert_eq!(Some(1.0), mapper.health.success("lan"));
        assert_eq!(
            (false, 1, 0),
            (outcome.session_lost, outcome.succeeded, outcome.failed)
        );

        // the box restarts: the session of the collector is no longer valid
        fake.restart();
        let outcome = mapper.set_all(update_lock()).await.unwrap();
        assert_eq!(Some(0.0), mapper.health.success("lan"));
        // told, to detect the capabilities again
        assert_eq!(
            (true, 0, 1),
            (outcome.session_lost, outcome.succeeded, outcome.failed)
        );

        // the collector does not wait for its client to expire, 30 minutes later
        mapper.set_all(update_lock()).await.unwrap();
        assert_eq!(Some(1.0), mapper.health.success("lan"));
    }

    #[tokio::test]
    async fn a_failure_with_a_valid_session_does_not_log_in_again() {
        let fake = FakeBox::default();
        let (_server, authenticator) = fake.serve().await;
        let factory = authenticator.login().await.unwrap();
        let mut mapper = mapper(&factory, "test_session_still_valid");

        mapper.set_all(update_lock()).await.unwrap();
        let logins = fake.logins();

        // the box keeps the session, but the collector fails, as during a fibre outage
        *fake.lan_down.lock().unwrap() = true;
        let outcome = mapper.set_all(update_lock()).await.unwrap();
        assert_eq!(Some(0.0), mapper.health.success("lan"));
        assert!(!outcome.session_lost);
        *fake.lan_down.lock().unwrap() = false;
        mapper.set_all(update_lock()).await.unwrap();

        assert_eq!(Some(1.0), mapper.health.success("lan"));
        assert_eq!(
            1,
            fake.checks(),
            "the failed collection checks the session once"
        );
        assert_eq!(logins, fake.logins(), "the valid session is kept");
    }

    #[tokio::test]
    async fn the_session_is_not_checked_while_every_collector_succeeds() {
        let fake = FakeBox::default();
        let (_server, authenticator) = fake.serve().await;
        let factory = authenticator.login().await.unwrap();
        let mut mapper = mapper(&factory, "test_session_no_check");

        for _ in 0..3 {
            mapper.set_all(update_lock()).await.unwrap();
        }

        assert_eq!(Some(1.0), mapper.health.success("lan"));
        assert_eq!(0, fake.checks());
    }

    #[tokio::test]
    async fn a_refused_login_suspends_the_collections_and_is_retried_once_per_period() {
        let fake = FakeBox::default();
        let (_server, authenticator) = fake.serve().await;
        let mut factory = authenticator.login().await.unwrap();
        let retry = std::time::Duration::from_millis(300);
        factory.refused_login_retry = retry;
        let runs = Arc::new(Mutex::new(0));
        let mut mapper = Mapper {
            maps: vec![
                (
                    "lan",
                    Box::new(LanMetricMap::new(&factory, "test_refused".to_string())),
                ),
                ("counting", Box::new(CountingMap(runs.clone()))),
            ],
            health: CollectorHealth::new("test"),
            session: Some(&factory),
            failed: vec![],
        };
        mapper.set_all(update_lock()).await.unwrap();
        assert_eq!(None, mapper.login_refusal());

        // the app is revoked on the box: its sessions end, and its logins are refused
        fake.restart();
        *fake.refusal.lock().unwrap() = Some("invalid_token");
        mapper.set_all(update_lock()).await.unwrap(); // auth_required, the session is lost
        let runs_before = *runs.lock().unwrap();
        let attempts = fake.attempts();
        mapper.set_all(update_lock()).await.unwrap(); // the login is refused
        assert_eq!(Some("invalid_token".to_string()), mapper.login_refusal());
        assert_eq!(
            attempts + 1,
            fake.attempts(),
            "a single login per collection"
        );
        assert_eq!(runs_before, *runs.lock().unwrap(), "the collection stops");

        // until the next attempt, no login and no collection
        mapper.set_all(update_lock()).await.unwrap();
        assert_eq!(attempts + 1, fake.attempts());
        assert_eq!(runs_before, *runs.lock().unwrap());

        tokio::time::sleep(retry).await;
        mapper.set_all(update_lock()).await.unwrap();
        assert_eq!(attempts + 2, fake.attempts());
        assert_eq!(Some("invalid_token".to_string()), mapper.login_refusal());

        // the app is allowed again: the next attempt logs in, and the collections resume
        *fake.refusal.lock().unwrap() = None;
        tokio::time::sleep(retry).await;
        mapper.set_all(update_lock()).await.unwrap();
        assert_eq!(None, mapper.login_refusal());
        assert_eq!(Some(1.0), mapper.health.success("lan"));
        assert_eq!(runs_before + 1, *runs.lock().unwrap());
    }

    #[tokio::test]
    async fn a_transient_login_error_is_not_a_refused_login() {
        let fake = FakeBox::default();
        let (_server, authenticator) = fake.serve().await;
        let factory = authenticator.login().await.unwrap();
        let mut mapper = mapper(&factory, "test_transient_login_error");
        mapper.set_all(update_lock()).await.unwrap();

        fake.restart();
        *fake.refusal.lock().unwrap() = Some("internal_error");
        for _ in 0..3 {
            mapper.set_all(update_lock()).await.unwrap();
        }

        assert_eq!(None, mapper.login_refusal());
        assert_eq!(Some(0.0), mapper.health.success("lan"));
    }

    #[tokio::test]
    async fn an_unreachable_box_is_not_a_refused_login() {
        let fake = FakeBox::default();
        let (server, authenticator) = fake.serve().await;
        let factory = authenticator.login().await.unwrap();
        let mut mapper = mapper(&factory, "test_unreachable");
        mapper.set_all(update_lock()).await.unwrap();

        // the box restarts and does not answer yet
        fake.restart();
        server.reset().await;
        mapper.set_all(update_lock()).await.unwrap();
        mapper.set_all(update_lock()).await.unwrap();

        assert_eq!(None, mapper.login_refusal());
        assert_eq!(Some(0.0), mapper.health.success("lan"));
    }
}
