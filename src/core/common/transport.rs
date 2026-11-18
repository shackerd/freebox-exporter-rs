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
pub async fn get_api_result<T: DeserializeOwned>(
    client: &reqwest::Client,
    api_url: &str,
    endpoint: &str,
) -> Result<Option<T>, Box<dyn std::error::Error + Send + Sync>> {
    let body = client
        .get(format!("{api_url}{endpoint}"))
        .send()
        .await?
        .text()
        .await?;

    api_result(endpoint, &body)
}

/// `result` of a response of the Freebox API to `endpoint`, `None` when the API omits it.
///
/// `success` is read first: when the box answers with an error, `result` has another shape, e.g.
/// the login challenge for `auth_required`, so the error is reported instead of a decoding error.
pub fn api_result<T: DeserializeOwned>(
    endpoint: &str,
    body: &str,
) -> Result<Option<T>, Box<dyn std::error::Error + Send + Sync>> {
    let unexpected = |e: serde_json::Error| -> Box<dyn std::error::Error + Send + Sync> {
        Box::new(FreeboxResponseError::new(format!(
            "{endpoint} response is unexpected: {e}"
        )))
    };
    let res =
        serde_json::from_str::<FreeboxResponse<serde_json::Value>>(body).map_err(unexpected)?;

    if !res.success.unwrap_or(false) {
        return Err(Box::new(FreeboxApiError {
            endpoint: endpoint.to_string(),
            error_code: res.error_code,
            msg: res.msg,
        }));
    }

    match res.result {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(result) => serde_json::from_value(result).map(Some).map_err(unexpected),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    use super::get_api_result;

    async fn answering(status: u16, body: serde_json::Value) -> (MockServer, String) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v4/items/"))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
            .mount(&server)
            .await;
        let api_url = format!("{}/api/", server.uri());
        (server, api_url)
    }

    async fn items(api_url: &str) -> Result<Option<Vec<u32>>, String> {
        get_api_result::<Vec<u32>>(&reqwest::Client::new(), api_url, "v4/items/")
            .await
            .map_err(|e| e.to_string())
    }

    #[tokio::test]
    async fn an_error_of_the_box_is_reported_whatever_its_result() {
        // the box answers auth_required with the login challenge as result, an object where
        // the endpoint returns a list
        let (_server, api_url) = answering(
            403,
            json!({ "success": false, "error_code": "auth_required", "msg": "login",
                    "result": { "password_salt": "fake", "challenge": "fake" } }),
        )
        .await;

        assert_eq!(
            Err("v4/items/ failed: auth_required (login)".to_string()),
            items(&api_url).await
        );
    }

    #[tokio::test]
    async fn the_result_is_returned_and_may_be_omitted() {
        let (_server, api_url) = answering(200, json!({ "success": true, "result": [1, 2] })).await;
        assert_eq!(Ok(Some(vec![1, 2])), items(&api_url).await);

        let (_server, api_url) = answering(200, json!({ "success": true })).await;
        assert_eq!(Ok(None), items(&api_url).await);
    }

    #[tokio::test]
    async fn an_unexpected_result_names_the_endpoint() {
        let (_server, api_url) = answering(
            200,
            json!({ "success": true, "result": { "not": "a list" } }),
        )
        .await;

        let error = items(&api_url).await.unwrap_err();
        assert!(
            error.starts_with("v4/items/ response is unexpected: "),
            "{error}"
        );
    }
}
