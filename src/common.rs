use reqwest::{Client, RequestBuilder};
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
#[allow(unused)] // deserialized object
pub struct Permissions {
    #[serde(default, with= "bool")]
    settings: bool,
    #[serde(default, with= "bool")]
    contacts: bool,
    #[serde(default, with= "bool")]
    calls: bool,
    #[serde(default, with= "bool")]
    explorer: bool,
    #[serde(default, with= "bool")]
    downloader: bool,
    #[serde(default, with= "bool")]
    parental: bool,
    #[serde(default, with= "bool")]
    pvr: bool
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
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

pub struct FreeboxClient {
    api_url: String,
    session_token: String
}

impl FreeboxClient {

    pub fn new(api_url: String, session_token: String) -> Self {
        Self {
            api_url,
            session_token
        }
    }

    pub async fn test(&self) -> Result<FreeboxResponse<Connection>,()> {

        let body =
            self.append_token(http_client_factory().unwrap().get(format!("{}v4/connection", self.api_url)))
            .send().await.unwrap()
            .text().await.unwrap();

        let res = serde_json::from_str::<FreeboxResponse<Connection>>(&body);

        match res {
            Ok(c) => {
                return Ok(c);
            },
            Err(e) => {
                panic!("{}", e);
            }
        }
    }

    fn append_token(&self, request_builder: RequestBuilder) -> RequestBuilder
    {
        request_builder.header("X-Fbx-App-Auth", self.session_token.as_str())
    }
}

#[derive(Deserialize, Debug)]
pub struct Connection {
    #[serde(alias="type")]
    pub _type: String,
    pub rate_down: u64,
    pub bytes_up: u64,
    pub rate_up: u64,
    pub bandwidth_up: u64,
    pub ipv4: String,
    pub ipv6: String,
    pub bandwidth_down: u64,
    pub state: String,
    pub bytes_down: u64,
    pub media: String
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
            .unwrap();
    Ok(client)
}