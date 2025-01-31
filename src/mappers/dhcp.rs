use async_trait::async_trait;
use prometheus_exporter::prometheus::{register_int_gauge_vec, IntGaugeVec};
use serde::Deserialize;

use crate::core::common::{AuthenticatedHttpClientFactory, FreeboxResponse};
use crate::mappers::MetricMap;

#[derive(Debug, Deserialize, Clone)]
struct StaticDhcpLease {
    id: Option<String>,
    hostname: Option<String>,
    ip: Option<String>,
    mac: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct DynamicDhcpLease {
    id: Option<String>,
    hostname: Option<String>,
    ip: Option<String>,
    mac: Option<String>,
    assign_time: Option<u64>,
    lease_remaining: Option<u64>,
    refresh_time: Option<u64>,
}

trait DhcpLease: std::fmt::Debug + Send {
    fn get_id(&self) -> Option<String>;
    fn get_hostname(&self) -> Option<String>;
    fn get_ip(&self) -> Option<String>;
    fn get_mac(&self) -> Option<String>;
    fn get_is_static(&self) -> Option<bool>;
    fn get_lease_remaining(&self) -> Option<i64>;
    fn get_assign_time(&self) -> Option<u64>;
    fn get_refresh_time(&self) -> Option<i64>;
}

impl DhcpLease for StaticDhcpLease {
    fn get_id(&self) -> Option<String> {
        self.id.clone()
    }

    fn get_hostname(&self) -> Option<String> {
        self.hostname.clone()
    }

    fn get_ip(&self) -> Option<String> {
        self.ip.clone()
    }

    fn get_mac(&self) -> Option<String> {
        self.mac.clone()
    }

    fn get_is_static(&self) -> Option<bool> {
        Some(true)
    }

    fn get_lease_remaining(&self) -> Option<i64> {
        Some(-1)
    }

    fn get_assign_time(&self) -> Option<u64> {
        Some(0)
    }

    fn get_refresh_time(&self) -> Option<i64> {
        Some(-1)
    }
}

impl DhcpLease for DynamicDhcpLease {
    fn get_id(&self) -> Option<String> {
        self.id.clone()
    }

    fn get_hostname(&self) -> Option<String> {
        self.hostname.clone()
    }

    fn get_ip(&self) -> Option<String> {
        self.ip.clone()
    }

    fn get_mac(&self) -> Option<String> {
        self.mac.clone()
    }

    fn get_is_static(&self) -> Option<bool> {
        Some(false)
    }

    fn get_lease_remaining(&self) -> Option<i64> {
        self.lease_remaining.clone().map(|v| v as i64)
    }

    fn get_assign_time(&self) -> Option<u64> {
        self.assign_time.clone()
    }

    fn get_refresh_time(&self) -> Option<i64> {
        self.refresh_time.clone().map(|v| v as i64)
    }
}

pub struct DhcpMetricMap {
    factory: AuthenticatedHttpClientFactory,
    lease_remaining_gauge: IntGaugeVec,
    refresh_time_gauge: IntGaugeVec,
    assign_time_gauge: IntGaugeVec,
}

impl DhcpMetricMap {
    pub fn new(factory: AuthenticatedHttpClientFactory, prefix: String) -> Self {
        let prfx: String = format!("{prefix}_dhcp");

        Self {
            factory,
            lease_remaining_gauge: register_int_gauge_vec!(
                format!("{prfx}_lease_remaining",),
                "Lease remaining time in milliseconds".to_string(),
                &["id", "hostname", "ip", "mac", "is_static"],
            )
            .expect(&format!(
                "Failed to create gauge for {prfx}_lease_remaining"
            )),
            refresh_time_gauge: register_int_gauge_vec!(
                format!("{prfx}_refresh_time"),
                "Refresh time in milliseconds".to_string(),
                &["id", "hostname", "ip", "mac", "is_static"],
            )
            .expect(&format!("Failed to create gauge for {prfx}_refresh_time")),
            assign_time_gauge: register_int_gauge_vec!(
                format!("{prfx}_assign_time"),
                "Assign time in milliseconds".to_string(),
                &["id", "hostname", "ip", "mac", "is_static"],
            )
            .expect(&format!("Failed to create gauge for {prfx}_assign_time")),
        }
    }

    async fn fetch_dhcp_static_leases(
        &self,
    ) -> Result<Vec<StaticDhcpLease>, Box<dyn std::error::Error + Send>> {
        let client = self.factory.create_client().await?;

        let res = client
            .get(format!("{}v4/dhcp/static_lease/", self.factory.api_url))
            .send()
            .await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res
            .unwrap()
            .json::<FreeboxResponse<Vec<StaticDhcpLease>>>()
            .await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res.unwrap();

        if res.success.unwrap_or_default() {
            Ok(res.result.unwrap_or(vec![]))
        } else {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                res.msg.unwrap_or("Unknown error".to_string()),
            )))
        }
    }

    async fn fetch_dhcp_dynamic_leases(
        &self,
    ) -> Result<Vec<DynamicDhcpLease>, Box<dyn std::error::Error + Send>> {
        let client = self.factory.create_client().await?;

        let res = client
            .get(format!("{}v4/dhcp/dynamic_lease/", self.factory.api_url))
            .send()
            .await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res
            .unwrap()
            .json::<FreeboxResponse<Vec<DynamicDhcpLease>>>()
            .await;

        if let Err(e) = res {
            return Err(Box::new(e));
        }

        let res = res.unwrap();

        if res.success.unwrap_or_default() {
            Ok(res.result.unwrap_or(vec![]))
        } else {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                res.msg.unwrap_or("Unknown error".to_string()),
            )))
        }
    }

    async fn fetch_dhcp_leases(
        &self,
    ) -> Result<Vec<Box<dyn DhcpLease>>, Box<dyn std::error::Error + Send>> {
        let mut leases: Vec<Box<dyn DhcpLease>> = vec![];

        let dyn_leases = self.fetch_dhcp_dynamic_leases().await;

        if let Err(e) = dyn_leases {
            return Err(e);
        }

        let dynamics = dyn_leases.unwrap();

        for lease in dynamics {
            leases.push(Box::new(lease));
        }

        let sta_leases = self.fetch_dhcp_static_leases().await;

        if let Err(e) = sta_leases {
            return Err(e);
        }

        let sta_leases = sta_leases.unwrap();

        for lease in sta_leases {
            leases.push(Box::new(lease));
        }

        Ok(leases)
    }

    async fn set_all(&self) -> Result<(), Box<dyn std::error::Error + Send>> {
        let leases = self.fetch_dhcp_leases().await;

        if let Err(e) = leases {
            return Err(e);
        }

        let leases = leases.unwrap();

        for lease in leases {
            self.lease_remaining_gauge
                .with_label_values(&[
                    &lease.get_id().unwrap_or_default(),
                    &lease.get_hostname().unwrap_or_default(),
                    &lease.get_ip().unwrap_or_default(),
                    &lease.get_mac().unwrap_or_default(),
                    &lease.get_is_static().unwrap_or_default().to_string(),
                ])
                .set(lease.get_lease_remaining().unwrap_or_default());

            self.refresh_time_gauge
                .with_label_values(&[
                    &lease.get_id().unwrap_or_default(),
                    &lease.get_hostname().unwrap_or_default(),
                    &lease.get_ip().unwrap_or_default(),
                    &lease.get_mac().unwrap_or_default(),
                    &lease.get_is_static().unwrap_or_default().to_string(),
                ])
                .set(lease.get_refresh_time().unwrap_or_default());

            self.assign_time_gauge
                .with_label_values(&[
                    &lease.get_id().unwrap_or_default(),
                    &lease.get_hostname().unwrap_or_default(),
                    &lease.get_ip().unwrap_or_default(),
                    &lease.get_mac().unwrap_or_default(),
                    &lease.get_is_static().unwrap_or_default().to_string(),
                ])
                .set(lease.get_assign_time().unwrap_or_default() as i64);
        }

        Ok(())
    }

    fn reset_all(&self) {
        self.lease_remaining_gauge.reset();
        self.refresh_time_gauge.reset();
        self.assign_time_gauge.reset();
    }
}

#[async_trait]
impl MetricMap for DhcpMetricMap {
    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_all();

        if let Err(e) = self.set_all().await {
            return Err(e);
        }

        Ok(())
    }

    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Initialize any necessary state or metrics
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mappers::api_specs_provider::get_specs_data;
    use serde_json::from_str;

    #[tokio::test]
    async fn test_deserialize_dhcp_static_leases() {
        let json_data = get_specs_data("dhcp", "api_v2_dhcp_static_lease-get")
            .await
            .unwrap();

        let data: Result<FreeboxResponse<Vec<StaticDhcpLease>>, _> = from_str(&json_data);

        if let Ok(e) = &data {
            println!("{:?}", e);
        }

        assert!(data.is_ok());
    }

    #[tokio::test]
    async fn test_deserialize_dhcp_dynamic_leases() {
        let json_data = get_specs_data("dhcp", "api_v2_dhcp_dynamic_lease-get")
            .await
            .unwrap();

        let data: Result<FreeboxResponse<Vec<DynamicDhcpLease>>, _> = from_str(&json_data);

        if let Ok(e) = &data {
            println!("{:?}", e);
        }

        assert!(data.is_ok());
    }
}
