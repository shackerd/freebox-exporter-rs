use serde::{Deserialize, Serialize};

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

pub async fn get_api_url(
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

pub fn get_static_api_url() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("https://{DEFAULT_FBX_HOST}/api/").to_string();
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
