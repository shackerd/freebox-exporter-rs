use async_trait::async_trait;
use chrono::Duration;
use connection::ConnectionMetricMap;
use download::DownloadMetricMap;
use health::CollectorHealth;
use lan::LanMetricMap;
use lanbrowser::LanBrowserMetricMap;
use log::{error, warn};
use nas::NasMetricMap;
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
    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// A collector with the name used as `collector` label in health metrics
type NamedMetricMap<'a> = (&'static str, Box<dyn MetricMap<'a> + 'a>);

pub struct Mapper<'a> {
    maps: Vec<NamedMetricMap<'a>>,
    health: CollectorHealth,
}

impl<'a> Mapper<'a> {
    pub fn new(
        factory: &'a AuthenticatedHttpClientFactory<'a>,
        conf: CapabilitiesConfiguration,
        caps: Capabilities,
        api_conf: ApiConfiguration,
        policies: Option<PoliciesConfiguration>,
    ) -> Self {
        let mut maps: Vec<NamedMetricMap<'a>> = vec![];

        if let Some(e) = conf.connection {
            if e {
                maps.push((
                    "connection",
                    Box::new(ConnectionMetricMap::new(
                        factory,
                        conf.prefix.to_owned().unwrap(),
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
                    Box::new(SystemMetricMap::new(
                        factory,
                        conf.prefix.to_owned().unwrap(),
                    )),
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
                    Box::new(LanMetricMap::new(
                        factory,
                        conf.prefix.to_owned().unwrap(),
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
                    let lan_browser_map =
                        LanBrowserMetricMap::new(factory, conf.prefix.to_owned().unwrap());
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
                        Box::new(SwitchMetricMap::new(
                            factory,
                            conf.prefix.to_owned().unwrap(),
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

                    let wifi_map = wifi::WifiMetricMap::new(
                        factory,
                        conf.prefix.to_owned().unwrap(),
                        Duration::seconds(api_conf.refresh.unwrap_or(5) as i64),
                        wifi_policies,
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
                        Box::new(dhcp::DhcpMetricMap::new(
                            factory,
                            conf.prefix.to_owned().unwrap(),
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
                        Box::new(NasMetricMap::new(
                            factory,
                            conf.prefix.to_owned().unwrap(),
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
                    Box::new(DownloadMetricMap::new(
                        factory,
                        conf.prefix.to_owned().unwrap(),
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
                        Box::new(VmMetricMap::new(
                            factory,
                            conf.prefix.to_owned().unwrap(),
                        )),
                    )),
                    Some(false) => warn!(
                        "vm is not supported by this freebox (requires API v{} and VM capable hardware), the option has been disabled",
                        vm::MIN_API_VERSION
                    ),
                    None => {
                        error!("vm support could not be detected at startup, the option is disabled until the exporter restarts");
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
                        Box::new(RaidMetricMap::new(
                            factory,
                            conf.prefix.to_owned().unwrap(),
                        )),
                    )),
                    Some(false) => warn!(
                        "raid is not supported by this freebox (requires API v{}), the option has been disabled",
                        raid::MIN_API_VERSION
                    ),
                    None => {
                        error!("raid support could not be detected at startup, the option is disabled until the exporter restarts");
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
                        Box::new(VpnMetricMap::new(
                            factory,
                            conf.prefix.to_owned().unwrap(),
                        )),
                    )),
                    Some(false) => warn!(
                        "vpn is not supported by this freebox (requires API v{} in router mode), the option has been disabled",
                        vpn::MIN_API_VERSION
                    ),
                    None => {
                        error!("vpn support could not be detected at startup, the option is disabled until the exporter restarts");
                        undetected.push("vpn");
                    }
                }
            }
        } else {
            warn!("VPN metrics are disabled by default, missing entry in the configuration file");
        }

        let health = CollectorHealth::register(conf.prefix.as_deref().unwrap());
        for name in undetected {
            health.observe(name, false, std::time::Duration::ZERO, std::time::SystemTime::now());
        }

        Self { maps, health }
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

    /// Runs every collector; a failing collector does not stop the others nor the exporter,
    /// its outcome is exposed through the health metrics
    pub async fn set_all(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for (name, map) in self.maps.iter_mut() {
            let started = std::time::Instant::now();
            let res = map.set().await;

            if let Err(e) = &res {
                error!("collector {name} failed: {e}");
            }

            self.health.observe(
                name,
                res.is_ok(),
                started.elapsed(),
                std::time::SystemTime::now(),
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use super::*;

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
        async fn set(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
        }
    }

    #[tokio::test]
    async fn requested_collector_with_undetected_support_is_exposed_as_failing() {
        let (_server, authenticator) = crate::mappers::test_support::mock_api().await;
        let factory = authenticator.login().await.unwrap();
        let conf: CapabilitiesConfiguration =
            toml::from_str("vm = true\nprefix = \"test_mapper_undetected\"").unwrap();
        let caps: Capabilities = serde_json::from_str(r#"{ "vm": null }"#).unwrap();

        let mapper = Mapper::new(&factory, conf, caps, ApiConfiguration { refresh: Some(5) }, None);

        assert!(mapper.maps.is_empty());
        assert_eq!(Some(0.0), mapper.health.success("vm"));
        assert_eq!(None, mapper.health.last_success("vm"));
    }

    #[tokio::test]
    async fn set_all_exposes_success_and_failure_per_collector() {
        let (ok, _) = ScriptedMap::new(&[true]);
        let (ko, _) = ScriptedMap::new(&[false]);
        let mut mapper = mapper(vec![("ok", Box::new(ok)), ("ko", Box::new(ko))]);

        assert!(mapper.set_all().await.is_ok());

        assert_eq!(Some(1.0), mapper.health.success("ok"));
        assert_eq!(Some(0.0), mapper.health.success("ko"));
        assert!(mapper.health.last_success("ok").unwrap() > 0.0);
        assert_eq!(None, mapper.health.last_success("ko"));
    }

    #[tokio::test]
    async fn set_all_keeps_last_success_timestamp_when_collector_starts_failing() {
        let (flaky, _) = ScriptedMap::new(&[true, false]);
        let mut mapper = mapper(vec![("flaky", Box::new(flaky))]);

        mapper.set_all().await.unwrap();
        let first_success = mapper.health.last_success("flaky");
        mapper.set_all().await.unwrap();

        assert_eq!(Some(0.0), mapper.health.success("flaky"));
        assert!(first_success.is_some());
        assert_eq!(first_success, mapper.health.last_success("flaky"));
    }

    #[tokio::test]
    async fn set_all_runs_remaining_collectors_after_a_failure() {
        let (ko, ko_runs) = ScriptedMap::new(&[false]);
        let (ok, ok_runs) = ScriptedMap::new(&[true]);
        let mut mapper = mapper(vec![("ko", Box::new(ko)), ("ok", Box::new(ok))]);

        mapper.set_all().await.unwrap();

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
        let mut mapper = mapper(vec![("broken", Box::new(broken)), ("healthy", Box::new(healthy))]);

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
            ("connection", Box::new(ConnectionMetricMap::new(factory, prefix.clone()))),
            ("system", Box::new(SystemMetricMap::new(factory, prefix.clone()))),
            ("lan", Box::new(LanMetricMap::new(factory, prefix.clone()))),
            ("lan_browser", Box::new(LanBrowserMetricMap::new(factory, prefix.clone()))),
            ("switch", Box::new(SwitchMetricMap::new(factory, prefix.clone()))),
            ("dhcp", Box::new(dhcp::DhcpMetricMap::new(factory, prefix.clone()))),
            ("nas", Box::new(NasMetricMap::new(factory, prefix.clone()))),
            ("download", Box::new(DownloadMetricMap::new(factory, prefix))),
        ]
    }

    #[tokio::test]
    async fn existing_collectors_return_an_error_when_the_renewed_client_is_expired() {
        let (_server, authenticator) = crate::mappers::test_support::mock_api().await;
        let mut factory = authenticator.login().await.unwrap();
        // every managed client is created already expired
        factory.expiration = chrono::TimeDelta::seconds(-1);

        // a panic here would stop every collector of the exporter
        for (name, mut map) in existing_collectors(&factory, "test_expired_client") {
            assert!(map.set().await.is_err(), "{name} succeeded with an expired client");
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
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    json!({ "success": true, "result": { "challenge": "1234" } }),
                ))
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

            assert!(map.set().await.is_err(), "{name} succeeded without session");
            let challenges = server.received_requests().await.unwrap().iter()
                .filter(|r| r.url.path() == "/api/v4/login/").count();
            assert_eq!(2, challenges, "{name} did not try to renew its session");
        }
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
