use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use log::{debug, error, info};
use prometheus_exporter::{
    prometheus::{
        core::{Collector, Desc},
        proto::MetricFamily,
        IntGauge, IntGaugeVec, Opts, Registry,
    },
    UpdateLock,
};

use crate::{
    core::{
        authenticator::Authenticator,
        capabilities::{Capabilities, CapabilitiesAgent},
        common::{
            error_chain::with_causes,
            http_client_factory::{
                login_refusal, AuthenticatedHttpClientFactory, REFUSED_LOGIN_RETRY,
            },
        },
        configuration::Configuration,
        redetection::{Decision, Redetection},
    },
    mappers::Mapper,
};

pub struct Server {
    port: u16,
    refresh_interval: u64,
    prefix: String,
}

impl Server {
    pub fn new(port: u16, refresh_interval: u64, prefix: String) -> Self {
        Self {
            port,
            refresh_interval,
            prefix,
        }
    }

    /// Serves the metrics of the box. The HTTP server starts first: while the box refuses the
    /// login, e.g. for an app revoked or not validated yet, it serves the metrics of the access,
    /// and the login is attempted again every minute.
    pub async fn run(
        &self,
        authenticator: &Authenticator,
        conf: &Configuration,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("initiating prometheus server");

        let addr_raw = format!("0.0.0.0:{}", self.port);

        info!("starting http server on {}", addr_raw);

        let addr: SocketAddr = match addr_raw.parse() {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        let served = Registry::new();
        let access = Access::register(&served, Registry::new(), &self.prefix)?;
        let exporter = match prometheus_exporter::Builder::new(addr)
            .with_registry(&served)
            .start()
        {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        let lock = exporter.update_lock();
        let duration = std::time::Duration::from_secs(self.refresh_interval);

        let factory = wait_for_login(authenticator, &access, &lock, REFUSED_LOGIN_RETRY).await?;
        let mut capabilities = CapabilitiesAgent::new(&factory)
            .load()
            .await?
            .for_configuration(&conf.metrics);
        let registry = Registry::new();
        let mut mapper = new_mapper(&factory, conf, capabilities.clone(), &registry);
        mapper.init_all().await?;
        // served once collected, as the next generations
        let mut first_generation = Some(registry);
        let mut redetection = Redetection::new(
            &capabilities,
            Instant::now(),
            REDETECTION_RETRY,
            REDETECTION_MAX_DELAY,
            REDETECTION_PERIOD,
        );

        loop {
            debug!("fetching result from mapper maps");

            let outcome = mapper.set_all(&lock).await?;
            let refusal = mapper.login_refusal();
            {
                let _update = lock.lock();
                access.update(refusal.as_deref());
                if let Some(registry) = first_generation.take() {
                    access.serve(registry);
                }
            }

            redetection.observe(outcome, Instant::now());
            let rebuilder = Rebuilder {
                factory: &factory,
                conf,
                access: &access,
                lock: &lock,
            };
            rebuilder
                .redetect(
                    &mut redetection,
                    &mut capabilities,
                    &mut mapper,
                    refusal.is_some(),
                )
                .await;

            let _guard = exporter.wait_duration(duration);
        }
    }
}

/// Rebuilds the collectors when the capabilities of the box change
struct Rebuilder<'g, 'a> {
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    conf: &'g Configuration,
    access: &'g Access,
    lock: &'g UpdateLock,
}

impl<'a> Rebuilder<'_, 'a> {
    /// Detects the capabilities again when due, and rebuilds the collectors when they changed
    async fn redetect(
        &self,
        redetection: &mut Redetection,
        capabilities: &mut Capabilities,
        mapper: &mut Mapper<'a>,
        login_refused: bool,
    ) {
        if !redetection.due(Instant::now(), login_refused) {
            return;
        }
        let detected = match CapabilitiesAgent::new(self.factory).quiet().load().await {
            Ok(detected) => Some(detected.for_configuration(&self.conf.metrics)),
            Err(e) => {
                debug!(
                    "cannot detect the capabilities: {}",
                    with_causes(e.as_ref())
                );
                None
            }
        };
        let Decision::Rebuild(detected) =
            redetection.decide(detected, capabilities, Instant::now())
        else {
            return;
        };
        let next = next_generation(
            self.factory,
            self.conf,
            detected.clone(),
            self.lock,
            &mapper.working_collectors(),
        )
        .await;
        match next {
            Ok((next, registry)) => {
                {
                    let _update = self.lock.lock();
                    self.access.serve(registry);
                }
                info!(
                    "capabilities changed ({}): collectors rebuilt",
                    capabilities.changes(&detected)
                );
                *mapper = next;
                *capabilities = detected;
            }
            Err(e) => {
                error!("cannot rebuild the collectors: {}", with_causes(e.as_ref()));
                redetection.rebuild_failed(Instant::now());
            }
        }
    }
}

/// First attempt to detect the capabilities again after a restart of the box, doubled up to
/// REDETECTION_MAX_DELAY while the detection fails
const REDETECTION_RETRY: Duration = Duration::from_secs(60);
const REDETECTION_MAX_DELAY: Duration = Duration::from_secs(15 * 60);
/// Detection without restart, e.g. for a change of the network mode
const REDETECTION_PERIOD: Duration = Duration::from_secs(3600);

fn new_mapper<'a>(
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    conf: &Configuration,
    capabilities: Capabilities,
    registry: &Registry,
) -> Mapper<'a> {
    Mapper::new(
        factory,
        conf.metrics.clone(),
        capabilities,
        conf.api.clone(),
        conf.policies.clone(),
        registry,
    )
}

/// Collectors for `capabilities`, in a registry of their own, already collected once. Refused
/// when the box refuses the login, or when a collector that works in the current generation
/// (`working`) fails: served, it would expose its gauges at 0 instead of its last values. A new
/// collector may fail, it has no previous values
async fn next_generation<'a>(
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    conf: &Configuration,
    capabilities: Capabilities,
    lock: &UpdateLock,
    working: &[&'static str],
) -> Result<(Mapper<'a>, Registry), Box<dyn std::error::Error + Send + Sync>> {
    let registry = Registry::new();
    let mut mapper = new_mapper(factory, conf, capabilities, &registry);
    mapper.init_all().await?;
    mapper.set_all(lock).await?;
    if let Some(code) = factory.login_refusal() {
        return Err(format!("the box refuses the login: {code}").into());
    }
    let newly_failed: Vec<&str> = mapper
        .failed_collectors()
        .iter()
        .filter(|name| working.contains(name))
        .copied()
        .collect();
    if !newly_failed.is_empty() {
        return Err(format!("first collection failed: {}", newly_failed.join(", ")).into());
    }
    Ok((mapper, registry))
}

/// Logs in, attempting again every `retry` while the box refuses the login; any other error, e.g.
/// an unreachable box, is returned
async fn wait_for_login<'a>(
    authenticator: &'a Authenticator,
    access: &Access,
    lock: &UpdateLock,
    retry: std::time::Duration,
) -> Result<AuthenticatedHttpClientFactory<'a>, Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let refusal = match authenticator.login().await {
            Ok(factory) => {
                let _update = lock.lock();
                access.update(None);
                return Ok(factory);
            }
            Err(e) => match login_refusal(e.as_ref()) {
                Some(code) => {
                    error!(
                        "the box refuses the login, next attempt in {} s: {e}",
                        retry.as_secs()
                    );
                    code
                }
                None => return Err(e),
            },
        };
        {
            let _update = lock.lock();
            access.update(Some(&refusal));
        }
        tokio::time::sleep(retry).await;
    }
}

/// Access of the exporter to the box. While the box refuses the login, e.g. after the app was
/// revoked, the metrics of the box are no longer served: they describe a box the exporter can no
/// longer read. Only the metrics of the access are, with the error_code of the box.
struct Access {
    open: Arc<AtomicBool>,
    /// Metrics of the current generation of collectors
    generation: Arc<Mutex<Registry>>,
    authenticated: IntGauge,
    error: IntGaugeVec,
}

impl Access {
    /// Registers into `served` the metrics of `box_metrics`, let through while the login is
    /// accepted, and the metrics of the access
    fn register(
        served: &Registry,
        box_metrics: Registry,
        prefix: &str,
    ) -> Result<Self, prometheus_exporter::prometheus::Error> {
        let open = Arc::new(AtomicBool::new(true));
        let generation = Arc::new(Mutex::new(box_metrics));
        served.register(Box::new(Gate {
            inner: generation.clone(),
            open: open.clone(),
        }))?;

        let authenticated = IntGauge::new(
            format!("{prefix}_authenticated"),
            "1 while the box accepts the login of the exporter, 0 while it refuses it",
        )?;
        let error = IntGaugeVec::new(
            Opts::new(
                format!("{prefix}_authentication_error"),
                "1 for the error_code of the box while it refuses the login of the exporter",
            ),
            &["error_code"],
        )?;
        served.register(Box::new(authenticated.clone()))?;
        served.register(Box::new(error.clone()))?;
        authenticated.set(1);

        Ok(Self {
            open,
            generation,
            authenticated,
            error,
        })
    }

    /// Serves the metrics of `registry` instead of those of the previous generation of collectors
    fn serve(&self, registry: Registry) {
        match self.generation.lock() {
            Ok(mut generation) => *generation = registry,
            Err(poisoned) => *poisoned.into_inner() = registry,
        }
    }

    /// `refusal` is the error_code of the login while the box refuses it
    fn update(&self, refusal: Option<&str>) {
        self.open.store(refusal.is_none(), Ordering::SeqCst);
        self.authenticated.set(i64::from(refusal.is_none()));
        self.error.reset();
        if let Some(code) = refusal {
            self.error.with_label_values(&[code]).set(1);
        }
    }
}

/// The metrics of `inner`, or none while `open` is false
struct Gate {
    inner: Arc<Mutex<Registry>>,
    open: Arc<AtomicBool>,
}

impl Collector for Gate {
    /// None: the metrics are described, and checked, by `inner`
    fn desc(&self) -> Vec<&Desc> {
        vec![]
    }

    fn collect(&self) -> Vec<MetricFamily> {
        if self.open.load(Ordering::SeqCst) {
            match self.inner.lock() {
                Ok(inner) => inner.gather(),
                Err(poisoned) => poisoned.into_inner().gather(),
            }
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use prometheus_exporter::prometheus::{Encoder, TextEncoder};

    use super::*;

    fn exposed(registry: &Registry) -> Vec<String> {
        let mut buffer = vec![];
        TextEncoder::new()
            .encode(&registry.gather(), &mut buffer)
            .unwrap();
        String::from_utf8(buffer)
            .unwrap()
            .lines()
            .filter(|line| !line.starts_with('#'))
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn a_new_generation_of_collectors_replaces_the_metrics_of_the_previous_one() {
        let gauge = |registry: &Registry, name: &str, value: i64| {
            let gauge = IntGauge::new(name, "a metric of the box").unwrap();
            registry.register(Box::new(gauge.clone())).unwrap();
            gauge.set(value);
        };
        let first = Registry::new();
        gauge(&first, "test_generation_old", 1);
        // the next generation registers the same metric again, and a new one
        let next = Registry::new();
        gauge(&next, "test_generation_old", 2);
        gauge(&next, "test_generation_new", 3);
        let served = Registry::new();
        let access = Access::register(&served, first, "test_generation").unwrap();

        access.serve(next);

        let metrics = exposed(&served);
        assert!(
            metrics.contains(&"test_generation_old 2".to_string()),
            "{metrics:?}"
        );
        assert!(
            metrics.contains(&"test_generation_new 3".to_string()),
            "{metrics:?}"
        );
        assert!(
            !metrics.contains(&"test_generation_old 1".to_string()),
            "{metrics:?}"
        );

        // still hidden while the box refuses the login
        access.update(Some("invalid_token"));
        assert!(!exposed(&served)
            .iter()
            .any(|l| l.starts_with("test_generation_new")));
    }

    #[tokio::test]
    async fn a_new_generation_is_served_with_its_first_collection() {
        use crate::mappers::test_support::{mock_api, new_update_lock, serve};
        use serde_json::json;

        let (server, authenticator) = mock_api().await;
        serve(
            &server,
            "/api/v4/system",
            json!({ "success": true, "result": { "uptime_val": 100 } }),
        )
        .await;
        let factory = authenticator.login().await.unwrap();
        let conf: Configuration = toml::from_str(
            "[api]\n[metrics]\nsystem = true\nprefix = \"test_next_generation\"\n[core]\n[log]\n",
        )
        .unwrap();
        let caps: Capabilities = serde_json::from_str(r#"{ "system": true }"#).unwrap();
        let lock = new_update_lock();

        let (_mapper, registry) = next_generation(&factory, &conf, caps, &lock, &[])
            .await
            .unwrap();
        let served = Registry::new();
        let access = Access::register(&served, Registry::new(), "test_next_access").unwrap();
        access.serve(registry);

        // never the 0 of a gauge registered but not collected yet
        let metrics = exposed(&served);
        assert!(
            metrics.contains(&"test_next_generation_system_uptime_val 100".to_string()),
            "{metrics:?}"
        );
    }

    #[tokio::test]
    async fn a_restart_announcing_a_new_api_version_adds_its_collectors() {
        use crate::core::redetection::CycleOutcome;
        use crate::mappers::test_support::{mock_api, new_update_lock, serve};
        use serde_json::json;

        // after its restart, the box announces API 8, which supports raid
        let (server, authenticator) = mock_api().await;
        let ok = |result| json!({ "success": true, "result": result });
        serve(
            &server,
            "/api/v4/lan/config",
            ok(json!({ "mode": "router" })),
        )
        .await;
        serve(
            &server,
            "/api/v4/wifi/config",
            ok(json!({ "enabled": true })),
        )
        .await;
        serve(&server, "/api_version", json!({ "api_version": "8.0" })).await;
        let sensors = json!([{ "id": "temp_t1", "name": "T1", "value": 40 }]);
        serve(
            &server,
            "/api/v6/system/",
            ok(json!({ "sensors": sensors })),
        )
        .await;
        let factory = authenticator.login().await.unwrap();
        let conf: Configuration = toml::from_str(
            "[api]\n[metrics]\nraid = true\nprefix = \"test_redetect\"\n[core]\n[log]\n",
        )
        .unwrap();

        // the capabilities detected before the restart: no raid
        let mut capabilities = CapabilitiesAgent::new(&factory).load().await.unwrap();
        capabilities.raid = Some(false);
        let registry = Registry::new();
        let mut mapper = new_mapper(&factory, &conf, capabilities.clone(), &registry);
        let served = Registry::new();
        let access = Access::register(&served, registry, "test_redetect_access").unwrap();
        let lock = new_update_lock();
        let raid_health = "test_redetect_collector_success{collector=\"raid\"}";
        assert!(!exposed(&served).iter().any(|l| l.starts_with(raid_health)));

        let mut redetection = Redetection::new(
            &capabilities,
            Instant::now(),
            Duration::ZERO,
            Duration::ZERO,
            Duration::from_secs(3600),
        );
        let lost = CycleOutcome {
            session_lost: true,
            succeeded: 0,
            failed: 1,
        };
        redetection.observe(lost, Instant::now());
        let rebuilder = Rebuilder {
            factory: &factory,
            conf: &conf,
            access: &access,
            lock: &lock,
        };

        // detected: not applied before it is confirmed
        rebuilder
            .redetect(&mut redetection, &mut capabilities, &mut mapper, false)
            .await;
        assert_eq!(Some(false), capabilities.raid);
        assert!(!exposed(&served).iter().any(|l| l.starts_with(raid_health)));

        // confirmed
        rebuilder
            .redetect(&mut redetection, &mut capabilities, &mut mapper, false)
            .await;

        assert_eq!(Some(true), capabilities.raid);
        let metrics = exposed(&served);
        assert!(
            metrics.iter().any(|l| l.starts_with(raid_health)),
            "{metrics:?}"
        );
    }

    #[tokio::test]
    async fn a_generation_is_not_served_when_a_working_collector_fails() {
        use crate::mappers::test_support::{mock_api, new_update_lock, serve};
        use serde_json::json;

        // right after a restart, the system collector still fails
        let (server, authenticator) = mock_api().await;
        serve(
            &server,
            "/api/v4/system",
            json!({ "success": false, "error_code": "internal_error" }),
        )
        .await;
        let factory = authenticator.login().await.unwrap();
        let conf: Configuration = toml::from_str(
            "[api]\n[metrics]\nsystem = true\nprefix = \"test_refused_generation\"\n[core]\n[log]\n",
        )
        .unwrap();
        let caps: Capabilities = serde_json::from_str(r#"{ "system": true }"#).unwrap();
        let lock = new_update_lock();

        // it works in the current generation: its last values are kept
        let refused = next_generation(&factory, &conf, caps.clone(), &lock, &["system"]).await;
        assert!(refused.is_err());

        // it already fails in the current generation, or is new: nothing to keep
        let accepted = next_generation(&factory, &conf, caps, &lock, &[]).await;
        assert!(accepted.is_ok());
    }

    #[test]
    fn the_metrics_of_the_box_are_served_only_while_the_login_is_accepted() {
        let box_metrics = Registry::new();
        let uptime = IntGauge::new("test_access_uptime", "uptime of the box").unwrap();
        box_metrics.register(Box::new(uptime.clone())).unwrap();
        uptime.set(42);
        let served = Registry::new();
        let access = Access::register(&served, box_metrics, "test_access").unwrap();

        assert_eq!(
            vec!["test_access_authenticated 1", "test_access_uptime 42"],
            exposed(&served)
        );

        access.update(Some("invalid_token"));
        assert_eq!(
            vec![
                "test_access_authenticated 0",
                "test_access_authentication_error{error_code=\"invalid_token\"} 1",
            ],
            exposed(&served)
        );

        access.update(None);
        assert_eq!(
            vec!["test_access_authenticated 1", "test_access_uptime 42"],
            exposed(&served)
        );
    }

    mod startup {
        use std::time::Duration;

        use serde_json::json;
        use wiremock::{
            matchers::{method, path},
            Mock, MockServer, ResponseTemplate,
        };

        use super::*;
        use crate::{
            core::authenticator::application_token_provider::MockApplicationTokenProvider,
            mappers::test_support::update_lock,
        };

        fn refused(code: &str) -> ResponseTemplate {
            ResponseTemplate::new(403)
                .set_body_json(json!({ "success": false, "error_code": code, "msg": "refused" }))
        }

        fn accepted() -> ResponseTemplate {
            ResponseTemplate::new(200)
                .set_body_json(json!({ "success": true, "result": { "session_token": "s" } }))
        }

        /// A box answering the logins with `answers` in order, the last one forever
        async fn box_answering(answers: Vec<ResponseTemplate>) -> (MockServer, Authenticator) {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/api/v4/login/"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(
                        json!({ "success": true, "result": { "challenge": "1234" } }),
                    ),
                )
                .mount(&server)
                .await;
            let last = answers.len() - 1;
            for (i, answer) in answers.into_iter().enumerate() {
                let mock = Mock::given(method("POST"))
                    .and(path("/api/v4/login/session"))
                    .respond_with(answer)
                    .with_priority(u8::try_from(i + 1).unwrap());
                let mock = if i < last {
                    mock.up_to_n_times(1)
                } else {
                    mock
                };
                mock.mount(&server).await;
            }
            let mut store = MockApplicationTokenProvider::new();
            store.expect_get().returning(|| Ok("foo.bar".to_string()));
            let authenticator =
                Authenticator::new(format!("{}/api/", server.uri()), Box::new(store));
            (server, authenticator)
        }

        async fn attempts(server: &MockServer) -> usize {
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .filter(|r| r.url.path() == "/api/v4/login/session")
                .count()
        }

        /// Access whose box metrics are a single gauge, `test_<prefix>_uptime 42`
        fn access(prefix: &str) -> (Registry, Access) {
            let box_metrics = Registry::new();
            let uptime = IntGauge::new(format!("{prefix}_uptime"), "uptime").unwrap();
            box_metrics.register(Box::new(uptime.clone())).unwrap();
            uptime.set(42);
            let served = Registry::new();
            let access = Access::register(&served, box_metrics, prefix).unwrap();
            (served, access)
        }

        #[tokio::test]
        async fn the_login_is_attempted_again_until_the_box_accepts_it() {
            let (server, authenticator) = box_answering(vec![
                refused("pending_token"),
                refused("pending_token"),
                accepted(),
            ])
            .await;
            let (served, access) = access("test_startup_accepted");

            let factory = wait_for_login(
                &authenticator,
                &access,
                update_lock(),
                Duration::from_millis(50),
            )
            .await;

            assert!(factory.is_ok());
            assert_eq!(3, attempts(&server).await);
            assert_eq!(
                vec![
                    "test_startup_accepted_authenticated 1",
                    "test_startup_accepted_uptime 42"
                ],
                exposed(&served)
            );
        }

        #[tokio::test]
        async fn while_the_box_refuses_the_login_the_access_tells_why() {
            let (server, authenticator) = box_answering(vec![refused("invalid_token")]).await;
            let (served, access) = access("test_startup_refused");
            let retry = Duration::from_millis(300);

            // the box keeps refusing: the wait goes on, one login per retry
            let waiting = tokio::time::timeout(
                retry + retry / 2,
                wait_for_login(&authenticator, &access, update_lock(), retry),
            )
            .await;

            assert!(waiting.is_err(), "the login was accepted");
            assert_eq!(2, attempts(&server).await);
            assert_eq!(
                vec![
                    "test_startup_refused_authenticated 0",
                    "test_startup_refused_authentication_error{error_code=\"invalid_token\"} 1",
                ],
                exposed(&served)
            );
        }

        #[tokio::test]
        async fn another_login_error_is_returned_at_once() {
            let (server, authenticator) = box_answering(vec![refused("internal_error")]).await;
            let (_served, access) = access("test_startup_failed");

            // a wait would last a minute per attempt: the test fails instead of hanging
            let factory = tokio::time::timeout(
                Duration::from_secs(5),
                wait_for_login(
                    &authenticator,
                    &access,
                    update_lock(),
                    Duration::from_secs(60),
                ),
            )
            .await
            .expect("the error was taken for a refusal");

            assert!(factory.is_err());
            assert_eq!(1, attempts(&server).await);
        }
    }
}
