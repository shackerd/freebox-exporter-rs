use crate::core::common::{
    http_client_factory, AuthenticatedHttpClientFactory, FreeboxResponse, FreeboxResponseError,
};
use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use core::str;
use hmac::{Hmac, Mac};
use log::{debug, error, info, warn};
use mockall::automock;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use std::{path::Path, sync::Arc, thread, time::Duration};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Mutex,
};

type HmacSha1 = Hmac<Sha1>;

#[derive(Serialize, Debug)]
pub struct PromptPayload {
    app_id: String,
    app_name: String,
    app_version: String,
    device_name: String,
}

impl PromptPayload {
    fn new(app_id: String, app_name: String, app_version: String, device_name: String) -> Self {
        PromptPayload {
            app_id,
            app_name,
            app_version,
            device_name,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct PromptResult {
    app_token: String,
    track_id: i32,
}

#[derive(Clone)]
pub struct SessionTokenProvider {
    issued_on: Arc<Mutex<DateTime<Utc>>>,
    value: Arc<Mutex<String>>,
    app_token: String,
    api_url: String,
}

impl SessionTokenProvider {
    pub fn new(app_token: String, api_url: String) -> Self {
        Self {
            issued_on: Arc::new(Mutex::new(
                Utc.with_ymd_and_hms(01, 01, 01, 00, 00, 01).unwrap(),
            )),
            value: Arc::new(Mutex::new(String::new())),
            app_token,
            api_url,
        }
    }

    pub async fn get(&self) -> Result<String, Box<dyn std::error::Error + Send>> {
        let duration = Utc::now() - *self.issued_on.lock().await;

        if duration > TimeDelta::minutes(30) {
            let mut issued_on_guard = self.issued_on.lock().await;

            let mut token_guard = self.value.lock().await;

            let result = match self.login().await {
                Err(e) => return Err(e),
                Ok(r) => r,
            };

            *issued_on_guard = Utc::now();

            (*token_guard).clear();
            (*token_guard).push_str(result.as_str());
            return Ok(result);
        }

        Ok((*self.value.lock().await).clone())
    }

    async fn login(&self) -> Result<String, Box<dyn std::error::Error + Send>> {
        debug!("login in");

        let token = self.app_token.clone();

        let challenge = match self.get_challenge().await {
            Err(e) => return Err(e),
            Ok(c) => c,
        };

        let password = match self.compute_password(token.to_owned(), challenge) {
            Err(e) => return Err(e),
            Ok(p) => p,
        };

        let session_result = match self.get_session_token(password).await {
            Err(e) => return Err(e),
            Ok(s) => s,
        };

        match session_result.session_token {
            Some(t) => Ok(t),
            None => Err(Box::new(AuthenticatorError::new(
                "cannot get session token".to_string(),
            ))),
        }
    }

    async fn get_challenge(&self) -> Result<ChallengeResult, Box<dyn std::error::Error + Send>> {
        debug!("fetching challenge");

        let client = http_client_factory().unwrap();

        let body = match (match client
            .get(format!("{}v4/login/", self.api_url))
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
            Ok(r) => r,
        };

        let challenge =
            match serde_json::from_str::<FreeboxResponse<ChallengeResult>>(body.as_str()) {
                Err(e) => return Err(Box::new(e)),
                Ok(r) => r,
            };

        if !challenge.success.unwrap_or(false) {
            return Err(Box::new(FreeboxResponseError::new(
                "response was not success".to_string(),
            )));
        }

        if challenge.result.is_none() {
            return Err(Box::new(FreeboxResponseError::new(
                "v4/login response was empty".to_string(),
            )));
        }

        Ok(challenge.result.unwrap())
    }

    async fn get_session_token(
        &self,
        password: String,
    ) -> Result<SessionResult, Box<dyn std::error::Error + Send>> {
        debug!("negociating session token");

        let client = http_client_factory().unwrap();

        let payload = SessionPayload {
            app_id: String::from("fr.freebox.prometheus.exporter"),
            password,
        };

        let resp = match client
            .post(format!("{}v4/login/session", self.api_url))
            .json(&payload)
            .send()
            .await
        {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        let body = match resp.text().await {
            Err(e) => return Err(Box::new(e)),
            Ok(b) => b,
        };

        let res = match serde_json::from_str::<FreeboxResponse<SessionResult>>(&body) {
            Err(e) => return Err(Box::new(e)),
            Ok(r) => r,
        };

        if !res.success.unwrap_or(false) {
            error!("{}", res.msg.unwrap_or_default());
            return Err(Box::new(AuthenticatorError::new(
                "Failed to get session token".to_string(),
            )));
        }

        if res.result.is_none() {
            return Err(Box::new(FreeboxResponseError::new(
                "v4/login/session response was empty".to_string(),
            )));
        }

        Ok(res.result.unwrap())
    }

    fn compute_password(
        &self,
        app_token: String,
        result: ChallengeResult,
    ) -> Result<String, Box<dyn std::error::Error + Send>> {
        debug!("computing session password");

        let mut mac = match HmacSha1::new_from_slice(app_token.as_bytes()) {
            Err(e) => return Err(Box::new(e)),
            Ok(h) => h,
        };

        mac.update(result.challenge.as_bytes());

        let code = mac.finalize().into_bytes();
        let res = code
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join("");

        Ok(res)
    }
}

pub struct Authenticator {
    api_url: String,
    token_store: Box<dyn TokenStorage>,
}

#[automock]
#[async_trait]
pub trait TokenStorage {
    async fn store(&self, token: String) -> Result<(), Box<dyn std::error::Error + Send>>;
    async fn get(&self) -> Result<String, Box<dyn std::error::Error + Send>>;
}

pub struct FileStorage {
    path: String,
}

impl FileStorage {
    pub fn new(data_dir: String) -> Self {
        let path = FileStorage::get_token_file_path(data_dir);
        Self { path }
    }

    pub fn get_token_file_path(data_dir: String) -> String {
        let sep = if cfg!(windows) { '\\' } else { '/' };
        format!("{}{}{}", data_dir, sep, "token.dat")
    }
}

#[async_trait]
impl TokenStorage for FileStorage {
    async fn store(&self, token: String) -> Result<(), Box<dyn std::error::Error + Send>> {
        let path = Path::new(&self.path);

        if path.exists() {
            match std::fs::remove_file(path) {
                Err(e) => return Err(Box::new(e)),
                _ => {}
            };
        }

        let mut file = match File::create(path).await {
            Err(e) => return Err(Box::new(e)),
            Ok(f) => f,
        };

        match file.write_all(token.as_bytes()).await {
            Err(e) => {
                match file.shutdown().await {
                    Err(e) => return Err(Box::new(e)),
                    _ => {}
                };
                return Err(Box::new(e));
            }
            _ => {}
        }

        match file.shutdown().await {
            Err(e) => return Err(Box::new(e)),
            _ => {}
        };

        Ok(())
    }

    async fn get(&self) -> Result<String, Box<dyn std::error::Error + Send>> {
        let path = Path::new(self.path.as_str());

        if !path.exists() {
            error!(
                "file does not exist {}, did you registered the application? See register command",
                self.path
            );
            panic!("file does not exist")
        }

        let mut file = match File::open(&self.path).await {
            Err(e) => return Err(Box::new(e)),
            Ok(f) => f,
        };

        let mut buffer = vec![];

        match file.read_to_end(&mut buffer).await {
            Err(e) => return Err(Box::new(e)),
            _ => {}
        };

        let token = match String::from_utf8(buffer) {
            Err(e) => return Err(Box::new(e)),
            Ok(s) => s,
        };

        Ok(token)
    }
}

impl Authenticator {
    pub fn new(api_url: String, store: Box<dyn TokenStorage>) -> Self {
        Self {
            api_url,
            token_store: store,
        }
    }

    pub async fn is_registered(&self) -> Result<bool, Box<dyn std::error::Error + Send>> {
        let token = self.token_store.get().await;

        if let Err(e) = token {
            return Err(e);
        }

        let token = token?;

        let path = Path::new(token.as_str());

        Ok(path.exists())
    }

    pub async fn register(
        &self,
        pool_interval: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send>> {
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
                return Err(Box::new(AuthenticatorError::new(
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
    ) -> Result<AuthenticatedHttpClientFactory, Box<dyn std::error::Error + Send>> {
        debug!("login in");

        let app_token = match self.token_store.get().await {
            Err(e) => return Err(e),
            Ok(t) => t,
        };

        let provider = SessionTokenProvider::new(app_token, self.api_url.clone());

        match provider.login().await {
            Ok(_) => Ok(AuthenticatedHttpClientFactory::new(
                self.api_url.clone(),
                provider,
            )),
            Err(e) => Err(e),
        }
    }

    async fn prompt(&self) -> Result<PromptResult, Box<dyn std::error::Error + Send>> {
        debug!("prompting for registration");

        let client = http_client_factory().unwrap();

        let payload = PromptPayload::new(
            String::from("fr.freebox.prometheus.exporter"),
            String::from("Prometheus Exporter"),
            String::from("1.0.0.0"),
            String::from("todo"),
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
    ) -> Result<(), Box<dyn std::error::Error>> {
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
                    let err = Box::new(AuthenticatorError::new(std::format!(
                        "Authorization has failed, reason: {}",
                        res.status
                    )));
                    return Err(err);
                }
                _ => {
                    let err = Box::new(AuthenticatorError::new(
                        "Incorrect response from server, escaping".to_string(),
                    ));
                    return Err(err);
                }
            }
        }

        if !result {
            let err = Box::new(AuthenticatorError::new(
                "Authorization aborted, reason: too much attempts".to_string(),
            ));
            return Err(err);
        }

        Ok(())
    }

    async fn get_authorization_status(
        &self,
        track_id: i32,
    ) -> Result<AuthorizationResult, Box<dyn std::error::Error>> {
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

        let res = serde_json::from_str::<FreeboxResponse<AuthorizationResult>>(&body)?;

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

    pub async fn diagnostic(&self, show_token: bool) -> Result<(), Box<dyn std::error::Error>> {
        let app_token = match self.token_store.get().await {
            Err(e) => return Err(e),
            Ok(t) => t,
        };

        let provider = SessionTokenProvider::new(app_token, self.api_url.clone());
        let token_result = provider.login().await;

        if token_result.is_ok() && show_token {
            println!("APP_TOKEN: {}", token_result.unwrap());
        }

        Ok(())
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct AuthorizationResult {
    status: String,
}

#[derive(Debug)]
pub struct AuthenticatorError {
    reason: String,
}

impl AuthenticatorError {
    fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl std::fmt::Display for AuthenticatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for AuthenticatorError {}

#[derive(Deserialize, Clone, Debug)]
pub struct ChallengeResult {
    challenge: String,
}

#[derive(Serialize, Debug)]
pub struct SessionPayload {
    app_id: String,
    password: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SessionResult {
    session_token: Option<String>,
    //permissions: Option<Permissions>
}

#[cfg(test)]
mod tests {

    use crate::{authenticator, core::authenticator::MockTokenStorage, discovery};
    use serde_json::json;
    use std::path::Path;
    use tokio::{
        fs::{self, File},
        io::AsyncWriteExt,
    };
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer,
    };

    #[tokio::test]
    async fn register_test() {
        let mock_server = MockServer::start().await;
        let mut store_mock = MockTokenStorage::new();
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

    async fn create_sample_token() -> Result<&'static Path, Box<dyn std::error::Error>> {
        let data_dir_path = Path::new("./test/");
        let token_path = Path::new("./test/token.dat");

        if !data_dir_path.exists() {
            fs::create_dir(data_dir_path)
                .await
                .expect("cannot create test directory");
        }

        if token_path.exists() {
            fs::remove_file(token_path)
                .await
                .expect("cannot remove sample token file");
        }

        let mut file = File::create(token_path)
            .await
            .expect("cannot create sample token file");
        let content = "foo.bar";

        file.write_all(content.as_bytes())
            .await
            .expect("cannot write to sample token file");
        file.shutdown().await.unwrap();

        Ok(token_path)
    }

    #[tokio::test]
    async fn login_test() {
        let api_url = discovery::get_api_url("localhost", 3001, true)
            .await
            .unwrap();

        let path = create_sample_token().await.unwrap();

        let authenticator = authenticator::Authenticator::new(
            api_url.to_owned(),
            Box::new(MockTokenStorage::new()),
        );

        match authenticator.login().await {
            Ok(_) => {}
            Err(e) => {
                println!("Have you launched mockoon?");
                println!("{e}:#?");
                panic!();
            }
        }

        fs::remove_file(path)
            .await
            .expect("cannot cleanup sample token file");
    }
}
