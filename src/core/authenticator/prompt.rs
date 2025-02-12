use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
pub struct PromptPayload {
    app_id: String,
    app_name: String,
    app_version: String,
    device_name: String,
}

impl PromptPayload {
    pub fn new(app_id: String, app_name: String, app_version: String, device_name: String) -> Self {
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
    pub app_token: String,
    pub track_id: i32,
}
