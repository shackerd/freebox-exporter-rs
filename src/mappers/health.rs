use std::time::{Duration, SystemTime, UNIX_EPOCH};

use prometheus_exporter::prometheus::{self, GaugeVec, IntGaugeVec, Opts};

/// Self-monitoring metrics describing the outcome of each collector run,
/// so a failing collector is visible in Prometheus instead of only in the logs
pub struct CollectorHealth {
    success: IntGaugeVec,
    last_success: IntGaugeVec,
    duration: GaugeVec,
}

impl CollectorHealth {
    /// Builds the metrics without registering them
    pub fn new(prefix: &str) -> Self {
        Self {
            success: IntGaugeVec::new(
                Opts::new(
                    format!("{prefix}_collector_success"),
                    "1 if the last run of the collector succeeded, 0 otherwise",
                ),
                &["collector"],
            )
            .expect(&format!("cannot create {prefix}_collector_success gauge")),
            last_success: IntGaugeVec::new(
                Opts::new(
                    format!("{prefix}_collector_last_success_timestamp_seconds"),
                    "unix timestamp of the last successful run of the collector",
                ),
                &["collector"],
            )
            .expect(&format!(
                "cannot create {prefix}_collector_last_success_timestamp_seconds gauge"
            )),
            duration: GaugeVec::new(
                Opts::new(
                    format!("{prefix}_collector_duration_seconds"),
                    "duration of the last run of the collector",
                ),
                &["collector"],
            )
            .expect(&format!(
                "cannot create {prefix}_collector_duration_seconds gauge"
            )),
        }
    }

    /// Builds the metrics and registers them into the default registry
    pub fn register(prefix: &str) -> Self {
        let health = Self::new(prefix);
        prometheus::register(Box::new(health.success.clone()))
            .expect(&format!("cannot register {prefix}_collector_success gauge"));
        prometheus::register(Box::new(health.last_success.clone())).expect(&format!(
            "cannot register {prefix}_collector_last_success_timestamp_seconds gauge"
        ));
        prometheus::register(Box::new(health.duration.clone())).expect(&format!(
            "cannot register {prefix}_collector_duration_seconds gauge"
        ));
        health
    }

    /// Records the outcome of one collector run; the last success timestamp is kept on failure
    pub fn observe(&self, collector: &str, succeeded: bool, elapsed: Duration, now: SystemTime) {
        self.success
            .with_label_values(&[collector])
            .set(succeeded as i64);
        self.duration
            .with_label_values(&[collector])
            .set(elapsed.as_secs_f64());

        if succeeded {
            if let Ok(since_epoch) = now.duration_since(UNIX_EPOCH) {
                self.last_success
                    .with_label_values(&[collector])
                    .set(since_epoch.as_secs() as i64);
            }
        }
    }

    /// Value exposed for a collector, None when the series does not exist
    #[cfg(test)]
    fn exposed(vec: &impl prometheus::core::Collector, collector: &str) -> Option<f64> {
        vec.collect()
            .iter()
            .flat_map(|family| family.get_metric())
            .find(|m| m.get_label().iter().any(|l| l.value() == collector))
            .map(|m| m.get_gauge().get_value())
    }

    #[cfg(test)]
    pub fn success(&self, collector: &str) -> Option<f64> {
        Self::exposed(&self.success, collector)
    }

    #[cfg(test)]
    pub fn last_success(&self, collector: &str) -> Option<f64> {
        Self::exposed(&self.last_success, collector)
    }

    #[cfg(test)]
    pub fn duration(&self, collector: &str) -> Option<f64> {
        Self::exposed(&self.duration, collector)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn observe_keeps_last_success_timestamp_on_failure() {
        let health = CollectorHealth::new("test");

        health.observe("vm", true, Duration::from_millis(5), at(100));
        health.observe("vm", false, Duration::from_millis(5), at(200));

        assert_eq!(Some(0.0), health.success("vm"));
        assert_eq!(Some(100.0), health.last_success("vm"));
    }

    #[test]
    fn observe_updates_last_success_timestamp_on_success() {
        let health = CollectorHealth::new("test");

        health.observe("vm", true, Duration::from_millis(5), at(100));
        health.observe("vm", true, Duration::from_millis(5), at(200));

        assert_eq!(Some(1.0), health.success("vm"));
        assert_eq!(Some(200.0), health.last_success("vm"));
    }

    #[test]
    fn observe_exposes_duration_in_seconds_on_success_and_failure() {
        let health = CollectorHealth::new("test");

        health.observe("vm", true, Duration::from_millis(5), at(100));
        assert_eq!(Some(0.005), health.duration("vm"));

        health.observe("vm", false, Duration::from_millis(7), at(200));
        assert_eq!(Some(0.007), health.duration("vm"));
    }

    #[test]
    fn register_exposes_prefixed_gauges_labelled_by_collector() {
        let health = CollectorHealth::register("health_register_test");
        health.observe("vm", true, Duration::from_millis(5), at(100));

        let families = prometheus::gather();
        for name in [
            "health_register_test_collector_success",
            "health_register_test_collector_last_success_timestamp_seconds",
            "health_register_test_collector_duration_seconds",
        ] {
            let family = families
                .iter()
                .find(|f| f.name() == name)
                .unwrap_or_else(|| panic!("{name} is not registered"));
            assert_eq!(
                prometheus::proto::MetricType::GAUGE,
                family.get_field_type()
            );
            let labels = family.get_metric()[0].get_label();
            assert_eq!(1, labels.len());
            assert_eq!("collector", labels[0].name());
            assert_eq!("vm", labels[0].value());
        }
    }
}
