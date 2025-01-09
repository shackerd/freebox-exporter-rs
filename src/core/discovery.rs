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
    device_type: String,
}
pub const DEFAULT_FBX_HOST: &str = "mafreebox.freebox.fr";

pub async fn get_api_url(host: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = http_client_factory().unwrap();

    let resp = (match client
        .get(format!("https://{host}/api_version"))
        .send()
        .await
    {
        Err(e) => return Err(Box::new(e)),
        Ok(r) => r,
    })
    .json::<ApiVersion>()
    .await?;

    let url = format!(
        "https://{}:{}{}",
        resp.api_domain, resp.https_port, resp.api_base_url
    );

    Ok(url)
}

pub fn get_static_api_url() -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("https://{DEFAULT_FBX_HOST}/api/").to_string();
    Ok(url)
}

#[cfg(test)]
mod tests {

    use crate::discovery;

    #[tokio::test]
    async fn get_api_url_test() {
        let api_url = discovery::get_api_url("locahost:3001").await;

        match api_url {
            Ok(z) => {
                println!("{z}");
                assert_eq!("https://localhost:3001/api/", z.as_str())
            }
            Err(e) => {
                println!("Error! {e:#?}");
            }
        }
    }
}
