use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use log::{debug, info};
use prometheus_exporter::prometheus::{
    core::{Collector, Desc},
    proto::MetricFamily,
    IntGauge, IntGaugeVec, Opts, Registry,
};

use crate::mappers::Mapper;

pub struct Server<'a> {
    port: u16,
    refresh_interval: u64,
    mapper: Mapper<'a>,
    prefix: String,
}

impl<'a> Server<'a> {
    pub fn new(port: u16, refresh_interval: u64, mapper: Mapper<'a>, prefix: String) -> Self {
        Self {
            port,
            refresh_interval,
            mapper,
            prefix,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

        self.mapper.init_all().await?;

        loop {
            debug!("fetching result from mapper maps");

            self.mapper.set_all(&lock).await?;
            let refusal = self.mapper.login_refusal();
            {
                let _update = lock.lock();
                access.update(refusal.as_deref());
            }

            let _guard = exporter.wait_duration(duration);
        }
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
}
