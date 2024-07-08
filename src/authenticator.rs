use core::str;
use std::{path::Path, thread::{self}, time::Duration};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use tokio::{fs::File, io::{AsyncReadExt, AsyncWriteExt}};
use crate::common::{http_client_factory, FreeboxClient, FreeboxResponse, Permissions};

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

#[derive(Deserialize, Debug)]
pub struct PromptResult {
    app_token: String,
    track_id: i32
}

pub struct Authenticator {
    api_url: String,
    token_file: String
}


impl Authenticator {
    pub fn new(api_url: String) -> Self {
        Self {
            api_url,
            token_file: "./token.dat".to_string()
        }
    }

    async fn store_app_token(&self, token: String) -> Result<(), Box<dyn std::error::Error>>
    {
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

        let path = Path::new(self.token_file.as_str());

        if !path.exists() {
            panic!("token file does not exist, did you registered the application? See -r option")
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

        let authorized = self.monitor_prompt(prompt_result.result.track_id, pool_interval).await?;

        if !authorized {
            panic!("unauthorized");
        }

        Ok(())
    }

    pub async fn login(&self) -> Result<FreeboxClient, Box<dyn std::error::Error>>{

        let token = self.load_app_token().await.expect("Cannot load app_token!");
        let challenge = self.get_challenge().await?.result;

        let password = self.compute_password(token, challenge).unwrap();

        let session_token = self.get_session_token(password).await?.result;
        let permissions = session_token.permissions;
        println!("{permissions:#?}");

        return Ok(FreeboxClient::new(self.api_url.to_owned(), session_token.session_token.unwrap().to_owned()));
    }

    async fn prompt(&self) -> Result<FreeboxResponse<PromptResult>, Box<dyn std::error::Error>> {

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

    async fn monitor_prompt(&self, track_id: i32, pool_interval: u64) -> Result<bool, Box<dyn std::error::Error>> {

        let mut result = false;

        println!("Requested authorization, please go to the Freebox and check LCD screen instructions");

        for _ in 0..100 {

            // pooling interval
            thread::sleep(Duration::from_secs(pool_interval));

            let resp =
                self.get_authorization_status(track_id).await;

            let res = match resp {
                Ok(r) => r,
                Err(e) => {
                    println!("{e:#?}");
                    panic!();
                }
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
                            AuthorizationError::new(
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
                            AuthorizationError::new(
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
                    AuthorizationError::new(
                        "Authorization aborted, reason: too much attempts".to_string()
                    )
                );
            return Err(err);
        }

        Ok(result)

    }

    async fn get_authorization_status(&self, track_id: i32) -> Result<FreeboxResponse<AuthorizationResult>, Box<dyn std::error::Error>> {

        let client = http_client_factory().unwrap();

        let resp =
            client.get(format!("{}v4/login/authorize/{}", self.api_url, track_id))
            .send().await?;

        let body = resp.text().await?;

        let res =
            serde_json::from_str::<FreeboxResponse<AuthorizationResult>>(&body)?;

        Ok(res)
    }

    async fn get_challenge(&self) -> Result<FreeboxResponse<ChallengeResult>, Box<dyn std::error::Error>> {

        let client = http_client_factory().unwrap();

        let body =
            client.get(format!("{}v4/login/", self.api_url))
                .send().await?
                .text().await?;

        let res =
            serde_json::from_str::<FreeboxResponse<ChallengeResult>>(&body)?;

        Ok(res)
    }

    async fn get_session_token(&self, password: String) -> Result<FreeboxResponse<SessionResult>, Box<dyn std::error::Error>> {

        let client = http_client_factory().unwrap();

        let payload = SessionPayload {
            app_id : String::from("fr.freebox.prometheus.exporter"),
            // app_version : "1.0.0.0".to_string(),
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
            panic!("{}", res.msg)
        }

        Ok(res)
    }


    fn compute_password(&self, app_token: String, result: ChallengeResult) -> Result<String, ()>{

        let mut mac = HmacSha1::new_from_slice(app_token.as_bytes()).unwrap();
        mac.update(result.challenge.as_bytes());
        let code = mac.finalize().into_bytes();
        let res = code.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join("");

        Ok(res)
    }
}

#[derive(Deserialize, Debug)]
pub struct AuthorizationResult {
    status: String,
    // challenge: String
}

#[derive(Debug)]
pub struct AuthorizationError {
    reason: String
}

impl AuthorizationError {
    fn new(reason: String) -> Self {
        Self {
            reason
        }
    }
}

impl std::fmt::Display for AuthorizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for AuthorizationError { }

#[derive(Deserialize, Debug)]
pub struct ChallengeResult {
    logged_in: bool,
    challenge: String,
    // password_salt: String
}

#[derive(Serialize, Debug)]
pub struct SessionPayload {
    app_id: String,
    // app_version: String,
    password: String
}

#[derive(Deserialize, Debug)]
pub struct SessionResult {
    // #[serde(default, with="String")]
    session_token: Option<String>,
    // challenge: String,
    // #[serde(default, with= "Permissions")]
    permissions: Option<Permissions>
}

#[cfg(test)]
mod tests {

    use crate::{authenticator, discovery};

    #[tokio::test]
    async fn register_test() {

        let api_url = discovery::get_api_url("localhost:3001", true).await.unwrap();

        let authenticator =
            authenticator::Authenticator::new(api_url.to_owned());

        match authenticator.register(1).await {
            Ok(_) => { },
            Err(e) => {
                println!("Have you launched mockoon?");
                println!("{e}:#?");
                panic!();
            }
        };
    }

    #[tokio::test]
    async fn login_test() {

        let api_url = discovery::get_api_url("localhost:3001", true).await.unwrap();

        let authenticator =
            authenticator::Authenticator::new(api_url.to_owned());

        match authenticator.login().await {
            Ok(_) => { },
            Err(e) => {
                println!("Have you launched mockoon?");
                println!("{e}:#?");
                panic!();
            }
        }
    }
}