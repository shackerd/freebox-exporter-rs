use async_trait::async_trait;
use log::debug;
use prometheus_exporter::prometheus::{
    register_int_gauge, register_int_gauge_vec, IntGauge, IntGaugeVec,
};
use reqwest::Client;
use serde::Deserialize;

use super::MetricMap;
use crate::core::common::{
    http_client_factory::{AuthenticatedHttpClientFactory, ManagedHttpClient},
    transport::{FreeboxResponse, FreeboxResponseError},
};

/// Download metrics mapper for Freebox `/api/v4/downloads/stats`.
///
/// This mapper exposes global downloader counters and states as Prometheus gauges.
#[derive(Deserialize, Clone, Debug, Default)]
struct DlRate {
    rx_rate: Option<i64>,
    tx_rate: Option<i64>,
}

#[derive(Deserialize, Clone, Debug)]
struct DownloadStats {
    nb_tasks: Option<i64>,
    nb_tasks_stopped: Option<i64>,
    nb_tasks_checking: Option<i64>,
    nb_tasks_queued: Option<i64>,
    nb_tasks_extracting: Option<i64>,
    nb_tasks_done: Option<i64>,
    nb_tasks_repairing: Option<i64>,
    nb_tasks_seeding: Option<i64>,
    nb_tasks_downloading: Option<i64>,
    nb_tasks_error: Option<i64>,
    nb_tasks_stopping: Option<i64>,
    nb_tasks_active: Option<i64>,
    nb_rss_items_unread: Option<i64>,
    rx_rate: Option<i64>,
    tx_rate: Option<i64>,
    throttling_mode: Option<String>,
    throttling_is_scheduled: Option<bool>,
    throttling_rate: Option<DlRate>,
    conn_ready: Option<bool>,
    nb_peer: Option<i64>,
}

pub struct DownloadMetricMap<'a> {
    factory: &'a AuthenticatedHttpClientFactory<'a>,
    managed_client: Option<ManagedHttpClient>,
    rx_rate_metric: IntGauge,
    tx_rate_metric: IntGauge,
    nb_tasks_metric: IntGauge,
    nb_tasks_active_metric: IntGauge,
    nb_tasks_downloading_metric: IntGauge,
    nb_tasks_seeding_metric: IntGauge,
    nb_tasks_stopped_metric: IntGauge,
    nb_tasks_queued_metric: IntGauge,
    nb_tasks_checking_metric: IntGauge,
    nb_tasks_extracting_metric: IntGauge,
    nb_tasks_done_metric: IntGauge,
    nb_tasks_repairing_metric: IntGauge,
    nb_tasks_error_metric: IntGauge,
    nb_tasks_stopping_metric: IntGauge,
    nb_rss_items_unread_metric: IntGauge,
    nb_peer_metric: IntGauge,
    conn_ready_metric: IntGauge,
    throttling_is_scheduled_metric: IntGauge,
    throttling_mode_metric: IntGaugeVec,
    throttling_rx_rate_metric: IntGauge,
    throttling_tx_rate_metric: IntGauge,
}

impl<'a> DownloadMetricMap<'a> {
    pub fn new(factory: &'a AuthenticatedHttpClientFactory<'a>, prefix: String) -> Self {
        Self {
            factory,
            managed_client: None,
            rx_rate_metric: register_int_gauge!(
                format!("{prefix}_download_rx_rate_bytes_per_second"),
                "Current download receive rate in bytes per second"
            )
            .expect(&format!(
                "cannot create {prefix}_download_rx_rate_bytes_per_second gauge"
            )),
            tx_rate_metric: register_int_gauge!(
                format!("{prefix}_download_tx_rate_bytes_per_second"),
                "Current download transmit rate in bytes per second"
            )
            .expect(&format!(
                "cannot create {prefix}_download_tx_rate_bytes_per_second gauge"
            )),
            nb_tasks_metric: register_int_gauge!(
                format!("{prefix}_download_tasks_total"),
                "Total number of download tasks"
            )
            .expect(&format!("cannot create {prefix}_download_tasks_total gauge")),
            nb_tasks_active_metric: register_int_gauge!(
                format!("{prefix}_download_tasks_active"),
                "Number of active download tasks"
            )
            .expect(&format!("cannot create {prefix}_download_tasks_active gauge")),
            nb_tasks_downloading_metric: register_int_gauge!(
                format!("{prefix}_download_tasks_downloading"),
                "Number of downloading tasks"
            )
            .expect(&format!("cannot create {prefix}_download_tasks_downloading gauge")),
            nb_tasks_seeding_metric: register_int_gauge!(
                format!("{prefix}_download_tasks_seeding"),
                "Number of seeding tasks"
            )
            .expect(&format!("cannot create {prefix}_download_tasks_seeding gauge")),
            nb_tasks_stopped_metric: register_int_gauge!(
                format!("{prefix}_download_tasks_stopped"),
                "Number of stopped tasks"
            )
            .expect(&format!("cannot create {prefix}_download_tasks_stopped gauge")),
            nb_tasks_queued_metric: register_int_gauge!(
                format!("{prefix}_download_tasks_queued"),
                "Number of queued tasks"
            )
            .expect(&format!("cannot create {prefix}_download_tasks_queued gauge")),
            nb_tasks_checking_metric: register_int_gauge!(
                format!("{prefix}_download_tasks_checking"),
                "Number of checking tasks"
            )
            .expect(&format!("cannot create {prefix}_download_tasks_checking gauge")),
            nb_tasks_extracting_metric: register_int_gauge!(
                format!("{prefix}_download_tasks_extracting"),
                "Number of extracting tasks"
            )
            .expect(&format!("cannot create {prefix}_download_tasks_extracting gauge")),
            nb_tasks_done_metric: register_int_gauge!(
                format!("{prefix}_download_tasks_done"),
                "Number of completed tasks"
            )
            .expect(&format!("cannot create {prefix}_download_tasks_done gauge")),
            nb_tasks_repairing_metric: register_int_gauge!(
                format!("{prefix}_download_tasks_repairing"),
                "Number of repairing tasks"
            )
            .expect(&format!("cannot create {prefix}_download_tasks_repairing gauge")),
            nb_tasks_error_metric: register_int_gauge!(
                format!("{prefix}_download_tasks_error"),
                "Number of tasks in error state"
            )
            .expect(&format!("cannot create {prefix}_download_tasks_error gauge")),
            nb_tasks_stopping_metric: register_int_gauge!(
                format!("{prefix}_download_tasks_stopping"),
                "Number of tasks stopping"
            )
            .expect(&format!("cannot create {prefix}_download_tasks_stopping gauge")),
            nb_rss_items_unread_metric: register_int_gauge!(
                format!("{prefix}_download_rss_items_unread"),
                "Number of unread RSS download items"
            )
            .expect(&format!("cannot create {prefix}_download_rss_items_unread gauge")),
            nb_peer_metric: register_int_gauge!(
                format!("{prefix}_download_peers_total"),
                "Number of connected download peers"
            )
            .expect(&format!("cannot create {prefix}_download_peers_total gauge")),
            conn_ready_metric: register_int_gauge!(
                format!("{prefix}_download_connection_ready"),
                "Download connection readiness (1 ready, 0 not ready)"
            )
            .expect(&format!("cannot create {prefix}_download_connection_ready gauge")),
            throttling_is_scheduled_metric: register_int_gauge!(
                format!("{prefix}_download_throttling_is_scheduled"),
                "Download throttling schedule status (1 scheduled, 0 not scheduled)"
            )
            .expect(&format!(
                "cannot create {prefix}_download_throttling_is_scheduled gauge"
            )),
            throttling_mode_metric: register_int_gauge_vec!(
                format!("{prefix}_download_throttling_mode_info"),
                "Download throttling mode information",
                &["mode"]
            )
            .expect(&format!(
                "cannot create {prefix}_download_throttling_mode_info gauge"
            )),
            throttling_rx_rate_metric: register_int_gauge!(
                format!("{prefix}_download_throttling_rx_rate_bytes_per_second"),
                "Download throttling receive limit in bytes per second"
            )
            .expect(&format!(
                "cannot create {prefix}_download_throttling_rx_rate_bytes_per_second gauge"
            )),
            throttling_tx_rate_metric: register_int_gauge!(
                format!("{prefix}_download_throttling_tx_rate_bytes_per_second"),
                "Download throttling transmit limit in bytes per second"
            )
            .expect(&format!(
                "cannot create {prefix}_download_throttling_tx_rate_bytes_per_second gauge"
            )),
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

        let client = self.managed_client.as_ref().clone().unwrap();
        let res = client.get();

        if res.is_ok() {
            return Ok(res.unwrap());
        } else {
            debug!("renewing managed client");

            let client = self.factory.create_managed_client().await;
            self.managed_client = Some(client.unwrap());

            return self.managed_client.as_ref().unwrap().get();
        }
    }

    async fn set_download_metrics(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let body = self
            .get_managed_client()
            .await?
            .get(format!("{}v4/downloads/stats", self.factory.api_url))
            .send()
            .await?
            .text()
            .await?;

        let res = match serde_json::from_str::<FreeboxResponse<DownloadStats>>(&body) {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                res.msg.unwrap_or_default(),
            )));
        }

        let stats = match res.result {
            Some(s) => s,
            None => {
                return Err(Box::new(FreeboxResponseError::new(
                    "v4/downloads/stats response was empty".to_string(),
                )))
            }
        };

        self.rx_rate_metric.set(stats.rx_rate.unwrap_or_default());
        self.tx_rate_metric.set(stats.tx_rate.unwrap_or_default());
        self.nb_tasks_metric.set(stats.nb_tasks.unwrap_or_default());
        self.nb_tasks_active_metric
            .set(stats.nb_tasks_active.unwrap_or_default());
        self.nb_tasks_downloading_metric
            .set(stats.nb_tasks_downloading.unwrap_or_default());
        self.nb_tasks_seeding_metric
            .set(stats.nb_tasks_seeding.unwrap_or_default());
        self.nb_tasks_stopped_metric
            .set(stats.nb_tasks_stopped.unwrap_or_default());
        self.nb_tasks_queued_metric
            .set(stats.nb_tasks_queued.unwrap_or_default());
        self.nb_tasks_checking_metric
            .set(stats.nb_tasks_checking.unwrap_or_default());
        self.nb_tasks_extracting_metric
            .set(stats.nb_tasks_extracting.unwrap_or_default());
        self.nb_tasks_done_metric
            .set(stats.nb_tasks_done.unwrap_or_default());
        self.nb_tasks_repairing_metric
            .set(stats.nb_tasks_repairing.unwrap_or_default());
        self.nb_tasks_error_metric
            .set(stats.nb_tasks_error.unwrap_or_default());
        self.nb_tasks_stopping_metric
            .set(stats.nb_tasks_stopping.unwrap_or_default());
        self.nb_rss_items_unread_metric
            .set(stats.nb_rss_items_unread.unwrap_or_default());
        self.nb_peer_metric.set(stats.nb_peer.unwrap_or_default());

        self.conn_ready_metric
            .set(if stats.conn_ready.unwrap_or(false) {
                1
            } else {
                0
            });
        self.throttling_is_scheduled_metric.set(
            if stats.throttling_is_scheduled.unwrap_or(false) {
                1
            } else {
                0
            },
        );

        self.throttling_mode_metric.reset();
        self.throttling_mode_metric
            .with_label_values(&[stats.throttling_mode.as_deref().unwrap_or("unknown")])
            .set(1);

        let throttling_rate = stats.throttling_rate.unwrap_or_default();
        self.throttling_rx_rate_metric
            .set(throttling_rate.rx_rate.unwrap_or_default());
        self.throttling_tx_rate_metric
            .set(throttling_rate.tx_rate.unwrap_or_default());

        Ok(())
    }
}

#[async_trait]
impl<'a> MetricMap<'a> for DownloadMetricMap<'a> {
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.set_download_metrics().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_deserialize_download_stats() {
        let payload = r#"{
            "nb_tasks": 13,
            "nb_tasks_stopped": 1,
            "nb_tasks_checking": 0,
            "nb_tasks_queued": 0,
            "nb_tasks_extracting": 4,
            "nb_tasks_done": 1,
            "nb_tasks_repairing": 0,
            "nb_tasks_active": 11,
            "nb_tasks_downloading": 4,
            "nb_tasks_error": 0,
            "nb_tasks_stopping": 0,
            "nb_tasks_seeding": 3,
            "nb_rss_items_unread": 5,
            "rx_rate": 14222,
            "tx_rate": 4294,
            "throttling_mode": "normal",
            "throttling_is_scheduled": true,
            "throttling_rate": {
                "rx_rate": 0,
                "tx_rate": 0
            },
            "conn_ready": true,
            "nb_peer": 42
        }"#;

        let stats: DownloadStats = serde_json::from_str(payload).unwrap();

        assert_eq!(stats.nb_tasks.unwrap(), 13);
        assert_eq!(stats.nb_tasks_stopped.unwrap(), 1);
        assert_eq!(stats.nb_tasks_checking.unwrap(), 0);
        assert_eq!(stats.nb_tasks_queued.unwrap(), 0);
        assert_eq!(stats.nb_tasks_extracting.unwrap(), 4);
        assert_eq!(stats.nb_tasks_done.unwrap(), 1);
        assert_eq!(stats.nb_tasks_repairing.unwrap(), 0);
        assert_eq!(stats.nb_tasks_active.unwrap(), 11);
        assert_eq!(stats.nb_tasks_downloading.unwrap(), 4);
        assert_eq!(stats.nb_tasks_error.unwrap(), 0);
        assert_eq!(stats.nb_tasks_stopping.unwrap(), 0);
        assert_eq!(stats.nb_tasks_seeding.unwrap(), 3);
        assert_eq!(stats.nb_rss_items_unread.unwrap(), 5);
        assert_eq!(stats.rx_rate.unwrap(), 14222);
        assert_eq!(stats.tx_rate.unwrap(), 4294);
        assert_eq!(stats.throttling_mode.unwrap(), "normal");
        assert_eq!(stats.throttling_is_scheduled.unwrap(), true);
        assert_eq!(stats.conn_ready.unwrap(), true);
        assert_eq!(stats.nb_peer.unwrap(), 42);

        let rate = stats.throttling_rate.unwrap();
        assert_eq!(rate.rx_rate.unwrap(), 0);
        assert_eq!(rate.tx_rate.unwrap(), 0);
    }
}
