use core::str;
use std::{path::Path, sync::Arc, thread, time::Duration};
use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use hmac::{Hmac, Mac};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use tokio::{fs::File, io::{AsyncReadExt, AsyncWriteExt}, sync::Mutex};
use crate::core::common::{http_client_factory, AuthenticatedHttpClientFactory, FreeboxResponse, FreeboxResponseError, Permissions};

type HmacSha1 = Hmac<Sha1>;

#[derive(Serialize, Debug)]
pub struct PromptPayload {
    app_id: String,
    app_name: String,
    app_version: String,
    device_name: String
}

impl PromptPayload {
    fn new(app_id: String, app_name: String, app_version: String, device_name: String) -> Self {
        PromptPayload {
            app_id,
            app_name,
            app_version,
            device_name
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct PromptResult {
    app_token: String,
    track_id: i32
}

#[derive(Clone)]
pub struct SessionTokenProvider {
    issued_on: Arc<Mutex<DateTime<Utc>>>,
    value: Arc<Mutex<String>>,
    app_token: String,
    api_url: String
}

impl SessionTokenProvider {

    pub fn new(app_token: String, api_url: String) -> Self {

        Self {
            issued_on: Arc::new(Mutex::new(Utc.with_ymd_and_hms(01, 01, 01, 00, 00, 01).unwrap())),
            value: Arc::new(Mutex::new(String::new())),
            app_token,
            api_url
        }
    }

    pub async fn get(&self) -> Result<String, Box<dyn std::error::Error>> {

        let duration = Utc::now() - *self.issued_on.lock().await;

        if duration > TimeDelta::minutes(30) {

            let mut issued_on_wl = self.issued_on.lock().await;

            let mut token_wl = self.value.lock().await;

            let login_res = self.login().await;

            if login_res.is_err() {
                return Err(login_res.err().unwrap());
            }

            let result = login_res.unwrap();
            *issued_on_wl = Utc::now();

            (*token_wl).clear();
            (*token_wl).push_str(result.as_str());
            return Ok(result);
        }

        Ok((*self.value.lock().await).clone())
    }

    async fn login(&self) -> Result<String, Box<dyn std::error::Error>>{

        debug!("login in");

        let token = self.app_token.clone();
        let challenge_result = self.get_challenge().await;
        let challenge = challenge_result.unwrap().result;

        let password = self.compute_password(token.to_owned(), challenge).unwrap();

        let session_token_result = self.get_session_token(password).await;

        match session_token_result {
            Err(e) => {
                return Err(e);
            },
            _ => { }
        }

        let result = session_token_result.unwrap().result;
        let permissions = result.permissions.unwrap();
        debug!("app permissions: {permissions:#?}");

        match result.session_token {
            Some(t) => Ok(t),
            None => Err(Box::new(AuthenticatorError::new("cannot get session token".to_string())))
        }
    }

    async fn get_challenge(&self) -> Result<FreeboxResponse<ChallengeResult>, Box<dyn std::error::Error>> {

        debug!("fetching challenge");

        let client = http_client_factory().unwrap();

        let response =
            client.get(format!("{}v4/login/", self.api_url))
                .send().await;

        if response.is_err() {
            return Err(Box::new(FreeboxResponseError::new("cannot get response".to_string())));
        }

        let body_result = response.unwrap().text().await;

        if body_result.is_err() {
            return Err(Box::new(FreeboxResponseError::new("cannot read body".to_string())));
        }

        let body = body_result.unwrap();

        let res =
            serde_json::from_str::<FreeboxResponse<ChallengeResult>>(body.as_str());

        if res.is_err() {
            return Err(Box::new(FreeboxResponseError::new("cannot read response".to_string())));
        }

        let challenge = res.unwrap();

        Ok(challenge.clone())
    }

    async fn get_session_token(&self, password: String) -> Result<FreeboxResponse<SessionResult>, Box<dyn std::error::Error>> {

        debug!("negociating session token");

        let client = http_client_factory().unwrap();

        let payload = SessionPayload {
            app_id : String::from("fr.freebox.prometheus.exporter"),
            password
        };

        let resp =
            client.post(format!("{}v4/login/session", self.api_url))
                .json(&payload)
                .send().await?;

        let body = resp.text().await?;

        let res =
            serde_json::from_str::<FreeboxResponse<SessionResult>>(&body)?;

        if !res.success {
            error!("{}", res.msg);
            return Err(Box::new(AuthenticatorError::new("Failed to get session token".to_string())));
        }

        Ok(res)
    }


    fn compute_password(&self, app_token: String, result: ChallengeResult) -> Result<String, ()>{

        debug!("computing session password");

        let mut mac = HmacSha1::new_from_slice(app_token.as_bytes()).unwrap();
        mac.update(result.challenge.as_bytes());
        let code = mac.finalize().into_bytes();
        let res = code.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join("");

        Ok(res)
    }
}

pub struct Authenticator {
    api_url: String,
    token_file: String
}


impl Authenticator {
    pub fn new(api_url: String, data_dir: String) -> Self {
        Self {
            api_url,
            token_file: Authenticator::get_token_file_path(data_dir.to_owned())
        }
    }

    fn get_token_file_path(data_dir: String) -> String {

        let sep = if cfg!(windows) { '\\' } else { '/' };
        format!("{}{}{}", data_dir, sep, "token.dat")
    }

    async fn store_app_token(&self, token: String) -> Result<(), Box<dyn std::error::Error>>
    {
        debug!("storing APP_TOKEN");

        let path = Path::new(self.token_file.as_str());

        if path.exists() {
            std::fs::remove_file(path)?;
        }

        let mut file = File::create(path).await?;

        file.write_all(token.as_bytes()).await?;

        file.shutdown().await?;

        Ok(())
    }

    async fn load_app_token(&self) -> Result<String, Box<dyn std::error::Error>> {

        debug!("loading APP_TOKEN");

        let path = Path::new(self.token_file.as_str());

        if !path.exists() {
            error!("token file does not exist {}, did you registered the application? See register command", self.token_file);
            panic!("token file does not exist")
        }

        let mut file = File::open(self.token_file.as_str()).await?;
        let mut buffer = vec![];

        file.read_to_end(&mut buffer).await?;

        let token = String::from_utf8(buffer)?;

        Ok(token)
    }

    pub async fn register(&self, pool_interval: u64) -> Result<(), Box<dyn std::error::Error>> {

        let prompt_result = self.prompt().await?;

        self.store_app_token(prompt_result.result.app_token).await?;

        let monitor_result = self.monitor_prompt(prompt_result.result.track_id, pool_interval).await;

        match monitor_result {
            Err(e) => {
                error!("{e:#?}");
                return Err(Box::new(AuthenticatorError::new("Failed to register application".to_string())));
            },
            _ => { }
        }

        Ok(())
    }

    pub async fn login(&self) -> Result<AuthenticatedHttpClientFactory, Box<dyn std::error::Error>>{

        debug!("login in");

        let app_token_result = self.load_app_token().await;

        if app_token_result.is_err() {
            return Err(app_token_result.err().unwrap());
        }

        let app_token = app_token_result.unwrap();

        let provider = SessionTokenProvider::new(app_token, self.api_url.clone());

        match provider.login().await {
            Ok(_) => Ok(AuthenticatedHttpClientFactory::new(self.api_url.clone(), provider)) ,
            Err(e) => Err(e)
        }
    }

    async fn prompt(&self) -> Result<FreeboxResponse<PromptResult>, Box<dyn std::error::Error>> {

        debug!("prompting for registration");

        let client = http_client_factory().unwrap();

        let payload =
            PromptPayload::new(
                String::from("fr.freebox.prometheus.exporter"),
                String::from("Prometheus Exporter"),
                String::from("1.0.0.0"),
                String::from("todo")
            );

        let resp =
            client.post(format!("{}v4/login/authorize", self.api_url))
            .json(&payload)
            .send().await?
            .text().await?;

        let res =
            serde_json::from_str::<FreeboxResponse<PromptResult>>(&resp)?;

        Ok(res)
    }

    async fn monitor_prompt(&self, track_id: i32, pool_interval: u64) -> Result<(), Box<dyn std::error::Error>> {

        debug!("monitoring registration prompt");

        let mut result = false;

        info!("Requested authorization, please go to the Freebox and check LCD screen instructions");

        for _ in 0..100 {

            thread::sleep(Duration::from_secs(pool_interval));

            let resp =
                self.get_authorization_status(track_id).await;

            let res = match resp {
                Ok(r) => r,
                Err(e) => return Err(e),
            };

            match res.result.status.as_str() {
                "granted" => {
                    result = true;
                    break;
                },
                "pending" => {
                    continue;
                },
                "timeout" | "unknown" | "denied" => {
                    let err =
                        Box::new(
                            AuthenticatorError::new(
                                std::format!(
                                    "Authorization has failed, reason: {}",
                                    res.result.status
                                )
                            )
                        );
                    return Err(err);
                }
                _ => {
                    let err =
                        Box::new(
                            AuthenticatorError::new(
                                "Incorrect response from server, escaping".to_string()
                            )
                        );
                    return Err(err);
                }
            }
        }

        if !result {
            let err =
                Box::new(
                    AuthenticatorError::new(
                        "Authorization aborted, reason: too much attempts".to_string()
                    )
                );
            return Err(err);
        }

        Ok(())

    }

    async fn get_authorization_status(&self, track_id: i32) -> Result<FreeboxResponse<AuthorizationResult>, Box<dyn std::error::Error>> {

        debug!("checking authorization status");

        let client = http_client_factory().unwrap();

        let resp =
            client.get(format!("{}v4/login/authorize/{}", self.api_url, track_id))
            .send().await?;

        let body = resp.text().await?;

        let res =
            serde_json::from_str::<FreeboxResponse<AuthorizationResult>>(&body)?;

        Ok(res)
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct AuthorizationResult {
    status: String,
}

#[derive(Debug)]
pub struct AuthenticatorError {
    reason: String
}

impl AuthenticatorError {
    fn new(reason: String) -> Self {
        Self {
            reason
        }
    }
}

impl std::fmt::Display for AuthenticatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for AuthenticatorError { }

#[derive(Deserialize, Clone, Debug)]
pub struct ChallengeResult {
    challenge: String,
}

#[derive(Serialize, Debug)]
pub struct SessionPayload {
    app_id: String,
    password: String
}

#[derive(Deserialize, Clone, Debug)]
pub struct SessionResult {
    session_token: Option<String>,
    permissions: Option<Permissions>
}

#[cfg(test)]
mod tests {

    use crate::{authenticator, discovery};
    use std::path::Path;
    use tokio::{fs::{self, File}, io::AsyncWriteExt};

    #[tokio::test]
    async fn register_test() {

        let api_url = discovery::get_api_url("localhost:3001").await.unwrap();

        let authenticator =
            authenticator::Authenticator::new(api_url.to_owned(), "./".to_string());

        match authenticator.register(1).await {
            Ok(_) => { },
            Err(e) => {
                println!("Have you launched mockoon?");
                println!("{e}:#?");
                panic!();
            }
        };
    }

    async fn create_sample_token() -> Result<&'static Path, Box<dyn std::error::Error>> {

        let data_dir_path = Path::new("./test/");
        let token_path = Path::new("./test/token.dat");

        if !data_dir_path.exists() {
            fs::create_dir(data_dir_path).await.expect("cannot create test directory");
        }

        if token_path.exists() {
            fs::remove_file(token_path).await.expect("cannot remove sample token file");
        }

        let mut file = File::create(token_path).await.expect("cannot create sample token file");
        let content = "foo.bar";

        file.write_all(content.as_bytes()).await.expect("cannot write to sample token file");
        file.shutdown().await.unwrap();

        Ok(token_path)
    }

    #[tokio::test]
    async fn login_test() {

        let api_url = discovery::get_api_url("localhost:3001").await.unwrap();

        let path = create_sample_token().await.unwrap();

        let authenticator =
            authenticator::Authenticator::new(api_url.to_owned(), "./test".to_string());

        match authenticator.login().await {
            Ok(_) => { },
            Err(e) => {
                println!("Have you launched mockoon?");
                println!("{e}:#?");
                panic!();
            }
        }

        fs::remove_file(path).await.expect("cannot cleanup sample token file");
    }
}