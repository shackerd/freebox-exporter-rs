use std::{env, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};
use log::debug;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};

use crate::core::authenticator::SessionTokenProvider;

const FBX_APP_AUTH_HEADER: &str = "X-Fbx-App-Auth";

#[derive(Clone)]
pub struct AuthenticatedHttpClientFactory<'a> {
    pub api_url: String,
    token_provider: SessionTokenProvider<'a>,
    pub expiration: TimeDelta,
}
static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

impl<'a> AuthenticatedHttpClientFactory<'a> {
    /// Create a new factory with the API URL and the session token provider.
    pub fn new(api_url: String, token_provider: SessionTokenProvider<'a>) -> Self {
        Self {
            api_url,
            token_provider,
            expiration: TimeDelta::minutes(30),
        }
    }

    /// Creates a new managed HTTP client with the necessary headers and configurations.
    ///
    /// # Returns
    ///
    /// A `Result` containing a `ManagedHttpClient` on success, or a boxed error on failure.
    ///
    /// # Errors
    ///
    /// This function will return an error if the session token cannot be retrieved from the token provider.
    ///
    /// # Example
    ///
    /// ```rust
    /// let factory = AuthenticatedHttpClientFactory::new(api_url, token_provider);
    /// let client = factory.create_managed_client().await?;
    /// ```
    pub async fn create_managed_client(
        &self,
    ) -> Result<ManagedHttpClient, Box<dyn std::error::Error + Sync + Send>> {
        debug!("creating managed http client");
        let mut headers = HeaderMap::new();

        let session_token = match self.token_provider.get().await {
            Err(e) => return Err(e),
            Ok(t) => t,
        };

        headers.append(
            FBX_APP_AUTH_HEADER,
            HeaderValue::from_str(session_token.as_str()).unwrap(),
        );

        let client = reqwest::ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .default_headers(headers)
            .tcp_keepalive(Duration::from_secs(self.expiration.num_seconds() as u64))
            .user_agent(APP_USER_AGENT)
            .build()
            .expect("cannot create HTTP Client");

        Ok(ManagedHttpClient::new(client, self.expiration))
    }
}

/*
auth_required 	Invalid session token, or not session token sent
invalid_token 	The app token you are trying to use is invalid or has been revoked
pending_token 	The app token you are trying to use has not been validated by user yet
insufficient_rights 	Your app permissions does not allow accessing this API
denied_from_external_ip 	You are trying to get an app_token from a remote IP
invalid_request 	Your request is invalid
ratelimited 	Too many auth error have been made from your IP
new_apps_denied 	New application token request has been disabled
apps_denied 	API access from apps has been disabled
internal_error 	Internal error
 */

pub fn http_client_factory() -> Result<Client, ()> {
    debug!("creating HTTP client");

    let client = reqwest::ClientBuilder::new()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("cannot create HTTP Client");
    Ok(client)
}
#[derive(Clone)]
pub struct ManagedHttpClient {
    client: Client,
    expiry: DateTime<Utc>, // 30 minutes
}

impl ManagedHttpClient {
    pub fn new(client: Client, timeout: TimeDelta) -> Self {
        let expiry = Utc::now().checked_add_signed(timeout).unwrap();
        Self { client, expiry }
    }

    pub fn get(&self) -> Result<Client, Box<dyn std::error::Error + Sync + Send>> {
        if Utc::now() > self.expiry {
            return Err(Box::new(ManagedHttpClientError::new(
                "HTTP Client expired".to_string(),
            )));
        }
        Ok(self.client.clone())
    }
}

pub struct ManagedHttpClientError {
    error: String,
}

impl ManagedHttpClientError {
    pub fn new(error: String) -> Self {
        Self { error }
    }
}

impl std::fmt::Display for ManagedHttpClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ManagedHttpClientError: {}", self.error)
    }
}

impl std::fmt::Debug for ManagedHttpClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ManagedHttpClientError: {}", self.error)
    }
}

impl std::error::Error for ManagedHttpClientError {}
