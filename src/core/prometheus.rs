use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
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
        capabilities::CapabilitiesAgent,
        common::http_client_factory::{
            login_refusal, AuthenticatedHttpClientFactory, REFUSED_LOGIN_RETRY,
        },
        configuration::Configuration,
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
        let access = Access::register(
            &served,
            prometheus_exporter::prometheus::default_registry().clone(),
            &self.prefix,
        )?;
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
        let capabilities = CapabilitiesAgent::new(&factory).load().await?;
        let mut mapper = Mapper::new(
            &factory,
            conf.metrics.clone(),
            capabilities,
            conf.api.clone(),
            conf.policies.clone(),
            prometheus_exporter::prometheus::default_registry(),
        );

        mapper.init_all().await?;

        loop {
            debug!("fetching result from mapper maps");

            mapper.set_all(&lock).await?;
            let refusal = mapper.login_refusal();
            {
                let _update = lock.lock();
                access.update(refusal.as_deref());
            }

            let _guard = exporter.wait_duration(duration);
        }
    }
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
        served.register(Box::new(Gate {
            inner: box_metrics,
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
            authenticated,
            error,
        })
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
    inner: Registry,
    open: Arc<AtomicBool>,
}

impl Collector for Gate {
    /// None: the metrics are described, and checked, by `inner`
    fn desc(&self) -> Vec<&Desc> {
        vec![]
    }

    fn collect(&self) -> Vec<MetricFamily> {
        if self.open.load(Ordering::SeqCst) {
            self.inner.gather()
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
