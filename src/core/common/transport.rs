use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FreeboxResponse<T: Clone> {
    pub msg: Option<String>,
    pub success: Option<bool>,
    pub uid: Option<String>,
    pub error_code: Option<String>,
    pub result: Option<T>,
}

#[derive(Debug)]
pub struct FreeboxResponseError {
    pub reason: String,
}

impl FreeboxResponseError {
    pub fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl Display for FreeboxResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for FreeboxResponseError {}
