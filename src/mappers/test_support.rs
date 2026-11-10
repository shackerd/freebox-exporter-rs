use std::sync::OnceLock;

use prometheus_exporter::{prometheus::Registry, Exporter, UpdateLock};
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

/// Update lock of an exporter shared by the tests that do not check it
pub fn update_lock() -> &'static UpdateLock {
    static LOCK: OnceLock<UpdateLock> = OnceLock::new();
    LOCK.get_or_init(new_update_lock)
}

/// Update lock of a new exporter, listening on a free local port with its own registry
pub fn new_update_lock() -> UpdateLock {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("cannot bind a local port");
    Exporter::builder_listener(listener)
        .with_registry(&Registry::new())
        .start()
        .expect("cannot start the exporter")
        .update_lock()
}

/// How long `collect_while_locked` holds the update lock
pub const LOCK_HOLD: std::time::Duration = std::time::Duration::from_millis(500);

/// Runs `collect` while another thread holds `lock` for `LOCK_HOLD`, as the exporter does
/// while it returns the metrics, and scrapes `prefix` from that thread just before releasing it
pub async fn collect_while_locked<F: std::future::Future>(
    lock: &UpdateLock,
    collect: F,
    prefix: &str,
) -> (F::Output, Vec<String>) {
    let (locked, is_locked) = std::sync::mpsc::channel();
    let holder = {
        let lock = lock.clone();
        let prefix = prefix.to_string();
        std::thread::spawn(move || {
            let _guard = lock.lock();
            locked.send(()).unwrap();
            std::thread::sleep(LOCK_HOLD);
            scrape(&prefix)
        })
    };
    is_locked.recv().unwrap();

    // blocks this thread once the collection waits for the lock: the holder runs on its own
    let output = collect.await;
    (output, holder.join().unwrap())
}

/// Serves `body` on GET `endpoint` until the returned guard is dropped
pub async fn serve_scoped(server: &MockServer, endpoint: &str, body: Value) -> wiremock::MockGuard {
    Mock::given(method("GET"))
        .and(path(endpoint))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount_as_scoped(server)
        .await
}
