use hmac::{Hmac, Mac};
use log::{debug, error};
use sha1::Sha1;
type HmacSha1 = Hmac<Sha1>;

use crate::core::{
    authenticator::{
        authentication_error::AuthenticationError,
        common::{ChallengeResult, SessionPayload},
    },
    common::{
        http_client_factory::http_client_factory,
        transport::{FreeboxResponse, FreeboxResponseError},
    },
};

use super::{application_token_provider::ApplicationTokenProvider, common::SessionResult};

#[derive(Clone)]
pub struct SessionTokenProvider<'a> {
    app_token_provider: &'a dyn ApplicationTokenProvider,
    api_url: String,
}

impl<'a> SessionTokenProvider<'a> {
    pub fn new(app_token_storage: &'a dyn ApplicationTokenProvider, api_url: String) -> Self {
        Self {
            app_token_provider: app_token_storage,
            api_url,
        }
    }

    pub async fn get(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let result = match self.login().await {
            Err(e) => return Err(e),
            Ok(r) => r,
        };

        Ok(result)
    }

    pub async fn login(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        debug!("login in");

        let token = self.app_token_provider.get().await;

        let token = token.as_ref().to_owned();

        let challenge = match self.get_challenge().await {
            Err(e) => return Err(e),
            Ok(c) => c,
        };

        let password = match self.compute_password(token.unwrap().to_owned().to_string(), challenge)
        {
            Err(e) => return Err(e),
            Ok(p) => p,
        };

        let session_result = match self.get_session_token(password).await {
            Err(e) => return Err(e),
            Ok(s) => s,
        };

        match session_result.session_token {
            Some(t) => Ok(t),
            None => Err(Box::new(AuthenticationError::new(
                "cannot get session token".to_string(),
            ))),
        }
    }

    async fn get_challenge(
        &self,
    ) -> Result<ChallengeResult, Box<dyn std::error::Error + Send + Sync>> {
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
    ) -> Result<SessionResult, Box<dyn std::error::Error + Send + Sync>> {
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
            return Err(Box::new(AuthenticationError::new(
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
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
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
