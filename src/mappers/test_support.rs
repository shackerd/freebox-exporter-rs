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

    let authenticator = Authenticator::new(format!("{}/api/", server.uri()), Box::new(store));

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

/// Series whose name starts with `prefix`, as a scrape of the exporter would return them
pub fn scrape(prefix: &str) -> Vec<String> {
    use prometheus_exporter::prometheus::{Encoder, TextEncoder};

    let mut buffer = vec![];
    TextEncoder::new()
        .encode(&prometheus_exporter::prometheus::gather(), &mut buffer)
        .unwrap();
    String::from_utf8(buffer)
        .unwrap()
        .lines()
        .filter(|line| line.starts_with(&format!("{prefix}_")))
        .map(str::to_string)
        .collect()
}

/// Delay of the slow endpoint served by `serve_slowly`: `scrape_during` scrapes within it
pub const SLOW_RESPONSE: std::time::Duration = std::time::Duration::from_secs(1);

/// Serves `body` on GET `endpoint` after `SLOW_RESPONSE`
pub async fn serve_slowly(server: &MockServer, endpoint: &str, body: Value) {
    Mock::given(method("GET"))
        .and(path(endpoint))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(body)
                .set_delay(SLOW_RESPONSE),
        )
        .mount(server)
        .await;
}

/// Runs `collect` and scrapes `prefix` once the slow `endpoint` has been requested, so while
/// the collection waits for it
pub async fn scrape_during<F: std::future::Future>(
    server: &MockServer,
    endpoint: &str,
    collect: F,
    prefix: &str,
) -> (F::Output, Vec<String>) {
    let requests = || async {
        let received = server.received_requests().await.unwrap();
        received.iter().filter(|r| r.url.path() == endpoint).count()
    };
    let before = requests().await;

    tokio::join!(collect, async {
        tokio::time::timeout(SLOW_RESPONSE, async {
            while requests().await == before {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("{endpoint} was not requested during the collection"));
        scrape(prefix)
    })
}

/// Serves `body` on GET `endpoint` until the returned guard is dropped
pub async fn serve_scoped(server: &MockServer, endpoint: &str, body: Value) -> wiremock::MockGuard {
    Mock::given(method("GET"))
        .and(path(endpoint))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount_as_scoped(server)
        .await
}
