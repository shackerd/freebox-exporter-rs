#[derive(Debug)]
pub struct AuthenticationError {
    reason: String,
}

impl AuthenticationError {
    pub fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl std::fmt::Display for AuthenticationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for AuthenticationError {}
