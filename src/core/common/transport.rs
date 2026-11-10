use std::fmt::Display;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FreeboxResponse<T: Clone> {
    pub msg: Option<String>,
    pub success: Option<bool>,
    pub uid: Option<String>,
    pub error_code: Option<String>,
    pub result: Option<T>,
}

#[derive(Debug)]
pub struct FreeboxResponseError {
    pub reason: String,
}

impl FreeboxResponseError {
    pub fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl Display for FreeboxResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for FreeboxResponseError {}

/// Error raised when the Freebox API answers with `success: false`,
/// keeping the Freebox `error_code` (e.g. `insufficient_rights`, `invalid_api_version`)
#[derive(Debug)]
pub struct FreeboxApiError {
    pub endpoint: String,
    pub error_code: Option<String>,
    pub msg: Option<String>,
}

impl Display for FreeboxApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} failed: {} ({})",
            self.endpoint,
            self.error_code.as_deref().unwrap_or("unknown_error"),
            self.msg.as_deref().unwrap_or_default()
        )
    }
}

impl std::error::Error for FreeboxApiError {}

/// GETs a Freebox API endpoint and returns its `result`;
/// `None` when the API omits it, which it does for empty collections
pub async fn get_api_result<T: DeserializeOwned + Clone>(
    client: &reqwest::Client,
    api_url: &str,
    endpoint: &str,
) -> Result<Option<T>, Box<dyn std::error::Error + Send + Sync>> {
    let res = client
        .get(format!("{api_url}{endpoint}"))
        .send()
        .await?
        .json::<FreeboxResponse<T>>()
        .await?;

    if !res.success.unwrap_or(false) {
        return Err(Box::new(FreeboxApiError {
            endpoint: endpoint.to_string(),
            error_code: res.error_code,
            msg: res.msg,
        }));
    }

    Ok(res.result)
}
