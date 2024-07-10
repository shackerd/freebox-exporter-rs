use reqwest::{header::{HeaderMap, HeaderValue}, Client };
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct FreeboxResponse<T> {
    #[serde(default, with= "String")]
    pub msg: String,
    pub success: bool,
    #[serde(default, with= "String")]
    pub uid: String,
    #[serde(default, with= "String")]
    pub error_code: String,
    pub result: T
}

#[derive(Deserialize, Debug)]
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
    session_token: String
}



impl AuthenticatedHttpClientFactory {

    pub fn new(api_url: String, session_token: String) -> Self {
        Self {
            api_url,
            session_token
        }
    }

    pub fn create_client(&self) -> Result<Client, ()> {

        let mut headers = HeaderMap::new();

        headers.append("X-Fbx-App-Auth", HeaderValue::from_str(self.session_token.as_str()).unwrap());

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