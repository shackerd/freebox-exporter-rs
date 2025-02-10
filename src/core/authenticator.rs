use crate::core::common::{
    http_client_factory::http_client_factory,
    transport::{FreeboxResponse, FreeboxResponseError},
};
use application_token_provider::ApplicationTokenProvider;
use authentication_error::AuthenticationError;
use common::AuthorizationResult;
use log::{debug, error, info, warn};
use std::{thread, time::Duration};

pub mod application_token_provider;
pub mod authentication_error;
pub mod common;
pub mod prompt;
pub mod session_token_provider;
pub use prompt::{PromptPayload, PromptResult};
pub use session_token_provider::SessionTokenProvider;

use super::common::http_client_factory::AuthenticatedHttpClientFactory;

pub struct Authenticator {
    api_url: String,
    token_store: Box<dyn ApplicationTokenProvider>,
}

impl Authenticator {
    pub fn new(api_url: String, store: Box<dyn ApplicationTokenProvider>) -> Self {
        Self {
            api_url,
            token_store: store,
        }
    }

    pub async fn is_registered(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let token = self.token_store.get().await;

        Ok(token.is_ok())
    }

    pub async fn register(
        &self,
        pool_interval: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let prompt_result = match self.prompt().await {
            Ok(r) => r,
            Err(e) => return Err(e),
        };

        match self.token_store.store(prompt_result.to_owned().app_token).await {
            Err(_) => warn!("storing applicaton token failed, you can still save it by yourself (token.dat): {}", prompt_result.app_token),
            _ => {}
        }

        let monitor_result = self
            .monitor_prompt(prompt_result.track_id, pool_interval)
            .await;

        match monitor_result {
            Err(e) => {
                error!("{e:#?}");
                return Err(Box::new(AuthenticationError::new(
                    "Failed to register application".to_string(),
                )));
            }
            _ => {}
        }

        info!("Successfully registered application");
        Ok(())
    }

    pub async fn login(
        &self,
    ) -> Result<AuthenticatedHttpClientFactory, Box<dyn std::error::Error + Send + Sync>> {
        debug!("login in");

        let provider = SessionTokenProvider::new(&self.token_store, self.api_url.clone());

        match provider.login().await {
            Ok(_) => Ok(AuthenticatedHttpClientFactory::new(
                self.api_url.clone(),
                provider,
            )),
            Err(e) => Err(e),
        }
    }

    async fn prompt(&self) -> Result<PromptResult, Box<dyn std::error::Error + Send + Sync>> {
        debug!("prompting for registration");

        let client = http_client_factory().unwrap();
        let hostname = hostname::get().unwrap();

        let payload = PromptPayload::new(
            String::from("fr.freebox.prometheus.exporter"),
            String::from("Prometheus Exporter"),
            String::from("1.0.0.0"),
            String::from(hostname.to_str().unwrap()),
        );

        let resp = match (match client
            .post(format!("{}v4/login/authorize", self.api_url))
            .json(&payload)
            .send()
            .await
        {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        })
        .text()
        .await
        {
            Err(e) => return Err(Box::new(e)),
            Ok(t) => t,
        };

        let res = match serde_json::from_str::<FreeboxResponse<PromptResult>>(&resp) {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                "response was not success".to_string(),
            )));
        }

        if res.result.is_none() {
            return Err(Box::new(FreeboxResponseError::new(
                "v4/login/authorize response was empty".to_string(),
            )));
        }

        Ok(res.result.unwrap())
    }

    async fn monitor_prompt(
        &self,
        track_id: i32,
        pool_interval: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("monitoring registration prompt");

        let mut result = false;

        info!(
            "Requested authorization, please go to the Freebox and check LCD screen instructions"
        );

        for _ in 0..100 {
            thread::sleep(Duration::from_secs(pool_interval));

            let res = match self.get_authorization_status(track_id).await {
                Ok(r) => r,
                Err(e) => return Err(e),
            };

            match res.status.as_str() {
                "granted" => {
                    result = true;
                    break;
                }
                "pending" => {
                    continue;
                }
                "timeout" | "unknown" | "denied" => {
                    let err = Box::new(AuthenticationError::new(std::format!(
                        "Authorization has failed, reason: {}",
                        res.status
                    )));
                    return Err(err);
                }
                _ => {
                    let err = Box::new(AuthenticationError::new(
                        "Incorrect response from server, escaping".to_string(),
                    ));
                    return Err(err);
                }
            }
        }

        if !result {
            let err = Box::new(AuthenticationError::new(
                "Authorization aborted, reason: too much attempts".to_string(),
            ));
            return Err(err);
        }

        Ok(())
    }

    async fn get_authorization_status(
        &self,
        track_id: i32,
    ) -> Result<AuthorizationResult, Box<dyn std::error::Error + Send + Sync>> {
        debug!("checking authorization status");

        let client = http_client_factory().unwrap();

        let resp = match client
            .get(format!("{}v4/login/authorize/{}", self.api_url, track_id))
            .send()
            .await
        {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        let body = match resp.text().await {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        let res = serde_json::from_str::<FreeboxResponse<AuthorizationResult>>(&body);
        let res = res.unwrap();

        if !res.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                "response was not success".to_string(),
            )));
        }

        if res.result.is_none() {
            return Err(Box::new(FreeboxResponseError::new(format!(
                "v4/login/authorize/{} response was empty",
                track_id
            ))));
        }

        Ok(res.result.unwrap())
    }

    pub async fn diagnostic(
        &self,
        show_token: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let provider = SessionTokenProvider::new(&self.token_store, self.api_url.clone());
        let token_result = provider.login().await;

        if token_result.is_ok() && show_token {
            println!("SESSION_TOKEN: {}", token_result.unwrap());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use crate::core::authenticator::{
        self, application_token_provider::MockApplicationTokenProvider,
    };
    use serde_json::json;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer,
    };

    #[tokio::test]
    async fn register_test() {
        let mock_server = MockServer::start().await;
        let mut store_mock = MockApplicationTokenProvider::new();
        store_mock.expect_store().times(1).returning(|_| Ok(()));

        let response = wiremock::ResponseTemplate::new(200).set_body_json(json!(
            {"result": {"app_token": "foo.bar", "track_id": 1 }, "success": true}
        ));

        Mock::given(method("POST"))
            .and(path("/api/v4/login/authorize"))
            .respond_with(response)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/v4/login/authorize/1"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "result": { "status": "granted" }, "success": true,
            })))
            .mount(&mock_server)
            .await;

        let api_url = format!("{}/api/", mock_server.uri());

        println!("api_url: {api_url}");

        let authenticator =
            authenticator::Authenticator::new(api_url.to_owned(), Box::new(store_mock));

        match authenticator.register(1).await {
            Ok(_) => {}
            Err(e) => {
                println!("{e}:#?");
                panic!();
            }
        };
    }

    #[tokio::test]
    async fn login_test() {
        let mock_server = MockServer::start().await;
        let mut store_mock = MockApplicationTokenProvider::new();
        store_mock
            .expect_get()
            .times(1)
            .returning(|| Ok("foo.bar".to_string()));

        let api_url = format!("{}/api/", mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/api/v4/login/"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "result": { "challenge": "1234" }, "success": true,
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/v4/login/session"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "result": { "session_token": "4321" }, "success": true,
            })))
            .mount(&mock_server)
            .await;

        {
            let authenticator =
                authenticator::Authenticator::new(api_url.to_owned(), Box::new(store_mock));
            let res = authenticator.login().await;

            match res {
                Ok(_) => {}
                Err(e) => {
                    println!("{e}:#?");
                    panic!();
                }
            }
        }
    }
}
