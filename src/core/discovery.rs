use serde::{Deserialize, Serialize};

use crate::core::common::http_client_factory;

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
    device_type: String
}

/// Discovers API url with given FQDN *(Fully Qualified Domain Name)*
///
/// The `use_tls` param may be set to `true` if discoverable host is HTTPS only
///
/// Remark: All freebox hosts have **this option set as default** nowadays.
pub async fn get_api_url(fqdn: &str, use_tls: bool) -> Result<String, Box<dyn std::error::Error>> {

    let client = http_client_factory().unwrap();

    let scheme =
        match use_tls {
            true => { "https"},
            false => { "http"}
        };

    let resp =
        client.get(format!("{scheme}://{fqdn}/api_version"))
            .send().await?
            .json::<ApiVersion>().await?;

    let url =
        format!("https://{}:{}{}", resp.api_domain, resp.https_port, resp.api_base_url);

    Ok(url)
}

#[cfg(test)]
mod tests {

    use crate::discovery;

    #[tokio::test]
    async fn get_api_url_test() {
        let api_url = discovery::get_api_url("localhost:3001", true).await;

        match api_url {
            Ok(z) => {
                println!("{z}");
                assert_eq!("https://localhost:3001/api/", z.as_str())
            },
            Err(e) => {
                println!("Error! {e:#?}");
            }
        }
    }
}