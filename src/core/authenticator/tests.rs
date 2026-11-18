#[cfg(test)]
#[allow(clippy::module_inception)]
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

#[cfg(test)]
mod timeouts {
    use std::time::Duration;

    use serde_json::json;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    use crate::{
        core::{
            authenticator::{
                application_token_provider::MockApplicationTokenProvider, Authenticator,
            },
            common::http_client_factory::Timeouts,
        },
        mappers::test_support::mock_api,
    };

    #[tokio::test]
    async fn the_sessions_use_the_timeouts_of_the_authenticator() {
        let timeouts = Timeouts {
            connect: Duration::from_secs(3),
            request: Duration::from_secs(7),
        };
        let (_server, authenticator) = mock_api().await;
        let authenticator = authenticator.with_timeouts(timeouts);

        let factory = authenticator.login().await.unwrap();

        assert_eq!(timeouts, factory.timeouts);
    }

    #[tokio::test]
    async fn the_login_gives_up_after_the_request_timeout() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v4/login/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "result": { "challenge": "1234" }, "success": true }))
                    .set_delay(Duration::from_secs(1)),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v4/login/session"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    json!({ "result": { "session_token": "4321" }, "success": true }),
                ),
            )
            .mount(&server)
            .await;

        let authenticator = |timeouts: Timeouts| {
            let mut store = MockApplicationTokenProvider::new();
            store.expect_get().returning(|| Ok("fake".to_string()));
            Authenticator::new(format!("{}/api/", server.uri()), Box::new(store))
                .with_timeouts(timeouts)
        };

        let short = Timeouts {
            request: Duration::from_millis(100),
            ..Timeouts::default()
        };
        assert!(authenticator(short).login().await.is_err());
        assert!(authenticator(Timeouts::default()).login().await.is_ok());
    }
}

#[cfg(test)]
mod address {
    use std::net::{IpAddr, Ipv4Addr};

    use serde_json::json;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    use crate::core::{
        authenticator::{application_token_provider::MockApplicationTokenProvider, Authenticator},
        discovery::DEFAULT_FBX_HOST,
    };

    #[tokio::test]
    async fn the_host_of_the_api_is_reached_at_the_configured_address() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v4/login/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "result": { "challenge": "1234" }, "success": true })),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v4/login/session"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    json!({ "result": { "session_token": "4321" }, "success": true }),
                ),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v4/system"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": true })))
            .expect(1)
            .mount(&server)
            .await;

        // the URL names the box, the mock server only listens on the loopback address
        let api_url = format!("http://{DEFAULT_FBX_HOST}:{}/api/", server.address().port());
        let mut store = MockApplicationTokenProvider::new();
        store.expect_get().returning(|| Ok("fake".to_string()));
        let authenticator = Authenticator::new(api_url.clone(), Box::new(store))
            .with_address(Some(IpAddr::V4(Ipv4Addr::LOCALHOST)));

        // the login, then a collector request through the managed client
        let factory = authenticator.login().await.unwrap();
        let client = factory
            .create_managed_client()
            .await
            .unwrap()
            .get()
            .unwrap();
        let res = client
            .get(format!("{api_url}v4/system"))
            .send()
            .await
            .unwrap();

        assert!(res.status().is_success());
        server.verify().await;
    }
}
