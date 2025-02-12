#[cfg(test)]
mod tests {

    use crate::core::authenticator::{
        self, application_token_provider::MockApplicationTokenProvider,
    };
    use serde_json::json;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer,
    };

    #[tokio::test]
    async fn register_test() {
        let mock_server = MockServer::start().await;
        let mut store_mock = MockApplicationTokenProvider::new();
        store_mock.expect_store().times(1).returning(|_| Ok(()));

        let response = wiremock::ResponseTemplate::new(200).set_body_json(json!(
            {"result": {"app_token": "foo.bar", "track_id": 1 }, "success": true}
        ));

        Mock::given(method("POST"))
            .and(path("/api/v4/login/authorize"))
            .respond_with(response)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/v4/login/authorize/1"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "result": { "status": "granted" }, "success": true,
            })))
            .mount(&mock_server)
            .await;

        let api_url = format!("{}/api/", mock_server.uri());

        println!("api_url: {api_url}");

        let authenticator =
            authenticator::Authenticator::new(api_url.to_owned(), Box::new(store_mock));

        match authenticator.register(1).await {
            Ok(_) => {}
            Err(e) => {
                println!("{e}:#?");
                panic!();
            }
        };
    }

    #[tokio::test]
    async fn login_test() {
        let mock_server = MockServer::start().await;
        let mut store_mock = MockApplicationTokenProvider::new();
        store_mock
            .expect_get()
            .times(1)
            .returning(|| Ok("foo.bar".to_string()));

        let api_url = format!("{}/api/", mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/api/v4/login/"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "result": { "challenge": "1234" }, "success": true,
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/v4/login/session"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "result": { "session_token": "4321" }, "success": true,
            })))
            .mount(&mock_server)
            .await;

        {
            let authenticator =
                authenticator::Authenticator::new(api_url.to_owned(), Box::new(store_mock));
            let res = authenticator.login().await;

            match res {
                Ok(_) => {}
                Err(e) => {
                    println!("{e}:#?");
                    panic!();
                }
            }
        }
    }
}
