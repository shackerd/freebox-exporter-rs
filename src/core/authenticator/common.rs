use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Debug)]
pub struct AuthorizationResult {
    pub status: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ChallengeResult {
    pub challenge: String,
}

#[derive(Serialize, Debug)]
pub struct SessionPayload {
    pub app_id: String,
    pub password: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SessionResult {
    pub session_token: Option<String>,
    //permissions: Option<Permissions>
}
