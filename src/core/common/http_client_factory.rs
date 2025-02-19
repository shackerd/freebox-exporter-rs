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
}

impl<'a> AuthenticatedHttpClientFactory<'a> {
    /// Create a new factory with the API URL and the session token provider.
    pub fn new(api_url: String, token_provider: SessionTokenProvider<'a>) -> Self {
        Self {
            api_url,
            token_provider,
        }
    }

    /// Create a new HTTP client with the session token.
    ///
    /// Remark: Session token is automatically fetched.
    pub async fn create_client(&self) -> Result<Client, Box<dyn std::error::Error + Sync + Send>> {
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
    let client = reqwest::ClientBuilder::new()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("cannot create HTTP Client");
    Ok(client)
}
