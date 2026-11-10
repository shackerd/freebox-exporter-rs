use serde_json::{json, Value};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

use crate::core::authenticator::{
    application_token_provider::MockApplicationTokenProvider, Authenticator,
};

/// Starts a fake Freebox API accepting any app token, and an authenticator pointing to it
pub async fn mock_api() -> (MockServer, Authenticator) {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/login/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": { "challenge": "1234" }, "success": true,
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/api/v4/login/session"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": { "session_token": "4321" }, "success": true,
        })))
        .mount(&server)
        .await;

    let mut store = MockApplicationTokenProvider::new();
    store.expect_get().returning(|| Ok("foo.bar".to_string()));

    let authenticator =
        Authenticator::new(format!("{}/api/", server.uri()), Box::new(store));

    (server, authenticator)
}

/// Serves `body` on GET `endpoint`
pub async fn serve(server: &MockServer, endpoint: &str, body: Value) {
    Mock::given(method("GET"))
        .and(path(endpoint))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}
