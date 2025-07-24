use crate::mappers::wifi::models::WifiConfig;

use super::common::{
    http_client_factory::AuthenticatedHttpClientFactory,
    transport::{FreeboxResponse, FreeboxResponseError},
};
use log::{debug, info};
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
struct LanConfig {
    mode: Option<String>,
}

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

        Ok(Capabilities {
            connection: Some(true),
            system: Some(true),
            lan: Some(true),
            lan_browser: Some(is_router),
            switch: Some(is_router),
            wifi: Some(is_wifi_enabled),
            dhcp: Some(is_router),
            network_mode: lan_config.mode,
        })
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
    pub network_mode: Option<String>,
}
