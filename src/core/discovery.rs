use log::info;
use serde::{Deserialize, Serialize};

use crate::{
    core::common::{
        http_client_factory::AuthenticatedHttpClientFactory,
        transport::{FreeboxResponse, FreeboxResponseError},
    },
    mappers::lan::LanConfig,
};

use super::common::http_client_factory::http_client_factory;

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiVersion {
    box_model_name: String,
    api_base_url: String,
    https_port: i32,
    device_name: String,
    https_available: bool,
    box_model: String,
    api_domain: String,
    uid: String,
    api_version: String,
    device_type: String,
}
pub const DEFAULT_FBX_HOST: &str = "mafreebox.freebox.fr";

/// Get the API URL for the Freebox
/// This function retrieves the API URL for the Freebox by making a request to the `/api_version` endpoint.
/// It constructs the URL based on the response, which includes the API domain, port, and base URL.
/// ## Arguments
/// * `host` - The host of the Freebox (e.g., "mafreebox.freebox.fr").
/// * `port` - The port number to connect to the Freebox.
/// * `use_ssl` - A boolean indicating whether to use SSL (HTTPS) or not.
/// ## Returns
/// * `Result<String, Box<dyn std::error::Error + Send + Sync>> - The API URL as a string if the request is successful.
/// ## Errors
/// * `Box<dyn std::error::Error + Send + Sync>` - If there is an error during the request or if the response cannot be parsed.
/// ## Example
/// ```
/// let api_url = get_api_url("mafreebox.freebox.fr", 443, true).await;
/// assert!(api_url.is_ok());
/// ```
async fn get_api_url(
    host: &str,
    port: u16,
    use_ssl: bool,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let client = http_client_factory().unwrap();

    let protocol = if use_ssl { "https" } else { "http" };

    let resp = (match client
        .get(format!("{protocol}://{host}:{port}/api_version"))
        .send()
        .await
    {
        Err(e) => return Err(Box::new(e)),
        Ok(r) => r,
    })
    .json::<ApiVersion>()
    .await;

    let resp = match resp {
        Err(e) => return Err(Box::new(e)),
        Ok(r) => r,
    };

    let proto = match resp.https_available {
        true => "https",
        false => "http",
    };

    let api_port = match resp.https_available {
        true => resp.https_port,
        false => port.into(),
    };

    if resp.https_available {}

    let url = format!(
        "{}://{}:{}{}",
        proto, resp.api_domain, api_port, resp.api_base_url
    );

    Ok(url)
}

/// Get the static API URL for the Freebox
/// This function constructs the static API URL for the Freebox using the default host.
/// ## Returns
/// * `Result<String, Box<dyn std::error::Error + Send + Sync>> - The static API URL as a string.
fn get_static_api_url() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("https://{DEFAULT_FBX_HOST}/api/").to_string();
    Ok(url)
}

/// Get the network mode of the Freebox
/// This function retrieves the network mode of the Freebox by making an authenticated request
/// to the LAN configuration endpoint.
/// ## Arguments
/// * `factory` - An instance of `AuthenticatedHttpClientFactory` to create an authenticated
///   HTTP client.
/// ## Returns
/// * `Result<String, Box<dyn std::error::Error + Send + Sync>> - The network mode as a string if the request is successful.
/// ## Errors
/// * `Box<dyn core::common::transport::FreeboxResponseError + Send + Sync>` - If there is an error during the request or if the response indicates failure.
/// ## Example
/// ```
/// let factory = AuthenticatedHttpClientFactory::new("https://mafreebox.freebox.fr", session_token_provider);
/// let network_mode = get_network_mode(&factory).await;
/// assert!(network_mode.is_ok());
/// ```
async fn get_network_mode<'a>(
    factory: &'a AuthenticatedHttpClientFactory<'a>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let client = factory.create_managed_client().await?.get().unwrap();

    let res = client
        .get(format!("{}v4/lan/config", factory.api_url)) // this endpoint requires authenticated request
        .send()
        .await?
        .json::<FreeboxResponse<LanConfig>>()
        .await
        .map_err(|e| Box::new(e))?;

    if !res.success.unwrap_or(false) {
        return Err(Box::new(FreeboxResponseError::new(
            res.msg.unwrap_or_default(),
        )));
    }

    Ok(res.result.unwrap().mode.unwrap_or_default())
}

/// Get the API URL based on the network mode of the Freebox
/// This function determines the API URL based on the network mode (bridge or router) of the
/// Freebox. It retrieves the mode and then constructs the appropriate URL based on the API definition.
/// ## Arguments
/// * `factory` - An instance of `AuthenticatedHttpClientFactory` to create an authenticated
///   HTTP client.
/// ## Returns
/// * `Result<String, Box<dyn std::error::Error + Send + Sync>> - The API URL as a string if the request is successful.
/// ## Errors
/// * `Box<dyn core::common::transport::FreeboxResponseError + Send + Sync>` - If there is an error during the request or if the response indicates failure.
/// ## Example
/// ```
/// let factory = AuthenticatedHttpClientFactory::new("https://mafreebox.freebox.fr", session_token_provider);
/// let api_url = get_url(&factory).await;
/// assert!(api_url.is_ok());
/// ```
pub async fn get_url<'a>(
    factory: &'a AuthenticatedHttpClientFactory<'a>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    info!("discovering freebox api url");

    let mode = get_network_mode(&factory).await?;
    let mode = mode.to_lowercase();

    let url = match mode.as_str() {
        "bridge" => {
            info!("network mode: {mode}, resolved api url {DEFAULT_FBX_HOST}");
            get_static_api_url().unwrap()
        }
        "router" => {
            let url = get_api_url(DEFAULT_FBX_HOST, 443, true).await.unwrap();
            info!("network mode: {mode}, resolved api url {url}");
            url
        }
        &_ => "".to_string(),
    };

    Ok(url)
}

#[cfg(test)]
mod tests {

    use serde_json::json;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer,
    };

    use crate::core::discovery;

    #[tokio::test]
    async fn get_api_url_test() {
        let mock_server = MockServer::start().await;

        let response = wiremock::ResponseTemplate::new(200).set_body_json(json!({
            "box_model_name":"Freebox v8 (r1)",
            "api_base_url":"/api/",
            "https_port":61406,
            "device_name":"Freebox Server",
            "https_available":true,
            "box_model":"fbxgw8-r1",
            "api_domain":"127.0.0.1",
            "uid":"00000000000000000000000000000000",
            "api_version":"12.2",
            "device_type":"FreeboxServer8,1"
        }));

        Mock::given(method("GET"))
            .and(path("/api_version"))
            .respond_with(response)
            .mount(&mock_server)
            .await;

        let api_url =
            discovery::get_api_url("127.0.0.1", mock_server.address().port(), false).await;

        match api_url {
            Ok(z) => {
                println!("{z}");
                assert_eq!("https://127.0.0.1:61406/api/", z.as_str())
            }
            Err(e) => {
                println!("Error! {e:#?}");
            }
        }
    }
}
