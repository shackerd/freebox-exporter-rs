use std::fmt::Display;

use log::debug;
use reqwest::{header::{HeaderMap, HeaderValue}, Client };
use serde::{Deserialize, Serialize};

use super::authenticator::SessionTokenProvider;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FreeboxResponse<T : Clone> {
    #[serde(default, with= "String")]
    pub msg: String,
    pub success: bool,
    #[serde(default, with= "String")]
    pub uid: String,
    #[serde(default, with= "String")]
    pub error_code: String,
    pub result: T
}

#[derive(Deserialize, Clone, Debug)]
#[allow(unused)]
pub struct Permissions {
    #[serde(default, with= "bool")]
    pub connection: bool,
    #[serde(default, with= "bool")]
    pub settings: bool,
    #[serde(default, with= "bool")]
    pub contacts: bool,
    #[serde(default, with= "bool")]
    pub calls: bool,
    #[serde(default, with= "bool")]
    pub explorer: bool,
    #[serde(default, with= "bool")]
    pub downloader: bool,
    #[serde(default, with= "bool")]
    pub parental: bool,
    #[serde(default, with= "bool")]
    pub pvr: bool
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            connection: Default::default(),
            settings: Default::default(),
            contacts: Default::default(),
            calls: Default::default(),
            explorer: Default::default(),
            downloader: Default::default(),
            parental: Default::default(),
            pvr: Default::default()
        }
    }
}

#[derive(Clone)]
pub struct AuthenticatedHttpClientFactory {
    pub api_url: String,
    token_provider: SessionTokenProvider
}



impl AuthenticatedHttpClientFactory {

    pub fn new(api_url: String, token_provider: SessionTokenProvider) -> Self {
        Self {
            api_url,
            token_provider
        }
    }

    pub async fn create_client(&self) -> Result<Client, Box<dyn std::error::Error>> {

        let mut headers = HeaderMap::new();

        let token_result = self.token_provider.get().await;

        if token_result.is_err() {
            return Err(token_result.err().unwrap());
        }

        let session_token = token_result.unwrap();

        debug!("creating a client with session_token: {}", session_token);

        headers.append("X-Fbx-App-Auth", HeaderValue::from_str(session_token.as_str()).unwrap());

        let client =
            reqwest::ClientBuilder::new()
                .danger_accept_invalid_certs(true)
                .default_headers(headers)
                .build()
                .expect("cannot create HTTP Client");
        Ok(client)
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
    let client =
        reqwest::ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .build()
            .expect("cannot create HTTP Client");
    Ok(client)
}

#[derive(Debug)]
pub struct FreeboxResponseError {
    pub reason: String
}

impl FreeboxResponseError {
    pub fn new(reason: String) -> Self {
        Self {
            reason
        }
    }
}

impl Display for FreeboxResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for FreeboxResponseError { }
