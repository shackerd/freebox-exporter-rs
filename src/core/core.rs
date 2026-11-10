use log::info;

use crate::{
    core::{authenticator::Authenticator, capabilities::CapabilitiesAgent, discovery},
    mappers::Mapper,
};

use super::{
    authenticator::{self, application_token_provider::FileSystemProvider},
    configuration::Configuration,
    prometheus,
};

/// ### Auto register and serve the application
/// This function will check if the application is already registered, if not it will register it
/// and then serve the metrics on the specified port
/// ### Arguments
/// * `conf` - The configuration object
/// * `interval` - The interval in seconds to check for user validation in registration process
/// * `port` - The port to serve the metrics on
/// ### Returns
/// * `Result<(), Box<dyn std::error::Error + Send + Sync>>` - The result of the operation
/// ### Errors
/// * `Box<dyn std::error::Error + Send + Sync>` - If there is an error during the operation
/// ### Example
/// ```
/// let conf = Configuration::new();
/// let interval = 5;
/// let port = 8080;
/// let result = auto_register_and_serve(&conf, interval, port).await;
/// assert_eq!(result, Ok(()));
/// ```
/// ### Notes
/// * This function will check if the application is already registered
/// * If the application is not registered, it will register it
/// * If the application is registered, it will log in
/// * It will then serve the metrics on the specified port
/// * It will return an error if there is an error during the operation
/// * It will return Ok(()) if the operation is successful
/// * It will return an error if the application is not registered
/// * It will return an error if the application is registered but there is an error during login
/// * It will return an error if there is an error during the operation
pub async fn auto_register_and_serve(
    conf: &Configuration,
    interval: u64,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let agnostic_auth = create_network_agnostic_authenticator(conf).await?;

    let res = agnostic_auth.is_registered().await;

    if let Err(e) = res {
        return Err(e);
    }

    if !res.unwrap_or(false) {
        info!("application is not registered, registering now");
        agnostic_auth.register(interval).await?;
    }

    info!("application is registered");

    let api_url = get_api_url(&agnostic_auth).await?;

    let authenticator = authenticator::Authenticator::new(
        api_url.clone(),
        Box::new(FileSystemProvider::new(
            conf.core.data_directory.as_ref().unwrap().to_owned(),
        )),
    )
    .with_timeouts(conf.api.timeouts());

    let factory = authenticator.login().await?;
    let cap_agent = CapabilitiesAgent::new(&factory);
    let capabilities = cap_agent.load().await?;

    let mapper = Mapper::new(
        &factory,
        conf.metrics.clone(),
        capabilities,
        conf.api.clone(),
        conf.policies.clone(),
    );
    let mut server = prometheus::Server::new(port, conf.api.refresh.unwrap_or(5), mapper);

    server.run().await
}

/// ### Get the API URL
/// This function will get the API URL from the Freebox API
pub async fn get_api_url(
    authenticator: &Authenticator,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let factory = match authenticator.login().await {
        Err(e) => return Err(e),
        Ok(r) => r,
    };

    let api_url = discovery::get_url(&factory).await?;

    info!("using api url: {api_url}");

    Ok(api_url)
}

/// ### Register the application
/// This function will register the application with the Freebox API
/// ## Arguments
/// * `conf` - The configuration object
/// * `interval` - The interval in seconds to check for user validation in registration process
/// ## Returns
/// * `Result<(), Box<dyn std::error::Error + Send + Sync>>` - The result of the operation
/// ## Errors
/// * `Box<dyn std::error::Error + Send + Sync>` - If there is an error during the operation
/// ## Example
/// ```
/// let conf = Configuration::new();
/// let interval = 5;
/// let result = register(&conf, interval).await;
/// assert_eq!(result, Ok(()));
/// ```
pub async fn register(
    conf: Configuration,
    interval: u64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let agnostic_auth = create_network_agnostic_authenticator(&conf).await?;

    register_once(&agnostic_auth, interval).await
}

/// Registers the application unless a token is already stored.
/// The token is shared by every API url, so a single registration is enough.
async fn register_once(
    authenticator: &Authenticator,
    interval: u64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if authenticator.is_registered().await? {
        info!("application is already registered, skipping registration");
        return Ok(());
    }

    info!("application is not registered, registering now");
    authenticator.register(interval).await?;
    info!("application is registered");

    Ok(())
}

/// ### Serve the application
/// This function will serve the application on the specified port
/// ## Arguments
/// * `conf` - The configuration object
/// * `port` - The port to serve the application on
/// ## Returns
/// * `Result<(), Box<dyn std::error::Error + Send + Sync>>` - The result of the operation
/// ## Errors
/// * `Box<dyn std::error::Error + Send + Sync>` - If there is an error during the operation
/// ## Example
/// ```
/// let conf = Configuration::new();
/// let port = 8080;
/// let result = serve(&conf, port).await;
/// assert_eq!(result, Ok(()));
/// ```
/// ## Notes
/// * This function will serve the application on the specified port
/// * It will return an error if there is an error during the operation
/// * It will return Ok(()) if the operation is successful
/// * It will return an error if the application is not registered
pub async fn serve(
    conf: Configuration,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let agnostic_auth = create_network_agnostic_authenticator(&conf).await?;

    let res = agnostic_auth.is_registered().await;

    if let Err(e) = res {
        return Err(e);
    }

    if !res.unwrap_or(false) {
        info!("application is not registered, exiting now");
        return Err(Box::new(std::io::Error::other(
            "Application is not registered, please register it first",
        )));
    }

    let api_url = get_api_url(&agnostic_auth).await?;

    let authenticator = authenticator::Authenticator::new(
        api_url.to_owned(),
        Box::new(FileSystemProvider::new(
            conf.core.data_directory.as_ref().unwrap().to_owned(),
        )),
    )
    .with_timeouts(conf.api.timeouts());

    let factory = match authenticator.login().await {
        Err(e) => return Err(e),
        Ok(r) => r,
    };

    let cap_agent = CapabilitiesAgent::new(&factory);

    let capabilities = cap_agent.load().await;
    if let Err(e) = capabilities {
        return Err(e);
    }

    let capabilities = capabilities.unwrap();

    let mapper = Mapper::new(
        &factory,
        conf.to_owned().metrics,
        capabilities,
        conf.to_owned().api,
        conf.to_owned().policies,
    );
    let mut server = prometheus::Server::new(port, conf.api.refresh.unwrap_or(5), mapper);

    server.run().await
}

async fn create_network_agnostic_authenticator(
    conf: &Configuration,
) -> Result<authenticator::Authenticator, Box<dyn std::error::Error + Send + Sync>> {
    let api_url = format!("https://{}/api/", discovery::DEFAULT_FBX_HOST).to_string();

    Ok(authenticator::Authenticator::new(
        api_url,
        Box::new(FileSystemProvider::new(
            conf.core.data_directory.as_ref().unwrap().to_owned(),
        )),
    )
    .with_timeouts(conf.api.timeouts()))
}

/// ### Session diagnostic
/// This function will run the session diagnostic
/// ## Arguments
/// * `conf` - The configuration object
/// * `show_token` - Whether to show the token or not
/// ## Returns
/// * `Result<(), Box<dyn std::error::Error + Send + Sync>>` - The result of the operation
/// ## Errors
/// * `Box<dyn std::error::Error + Send + Sync>` - If there is an error during the operation
/// ## Example
/// ```
/// let conf = Configuration::new();
/// let show_token = true;
/// let result = session_diagnostic(conf, show_token).await;
/// assert_eq!(result, Ok(()));
/// ```
/// ## Notes
/// * This function will run the session diagnostic
/// * It will return an error if there is an error during the operation
/// * It will return Ok(()) if the operation is successful
/// * It will return an error if the application is not registered
/// * It will show the token if show_token is true
pub async fn session_diagnostic(
    conf: Configuration,
    show_token: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let agnostic_auth = create_network_agnostic_authenticator(&conf).await?;

    let res = agnostic_auth.is_registered().await;

    if let Err(e) = res {
        return Err(e);
    }

    if !res.unwrap_or(false) {
        info!("application is not registered, exiting now");
        return Err(Box::new(std::io::Error::other(
            "Application is not registered, please register it first",
        )));
    }

    if let Ok(api_url) = get_api_url(&agnostic_auth).await {
        let authenticator = authenticator::Authenticator::new(
            api_url.to_owned(),
            Box::new(FileSystemProvider::new(
                conf.core.data_directory.as_ref().unwrap().to_owned(),
            )),
        )
        .with_timeouts(conf.api.timeouts());

        authenticator.diagnostic(show_token).await?;
    } else {
        return Err(Box::new(std::io::Error::other("Unable to get api url")));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    use super::register_once;
    use crate::core::authenticator::{
        application_token_provider::MockApplicationTokenProvider, Authenticator,
    };

    async fn mock_authorization(server: &MockServer, expected_prompts: u64) {
        Mock::given(method("POST"))
            .and(path("/api/v4/login/authorize"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(
                {"result": {"app_token": "fake", "track_id": 1 }, "success": true}
            )))
            .expect(expected_prompts)
            .mount(server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/v4/login/authorize/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(
                {"result": { "status": "granted" }, "success": true}
            )))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn register_once_prompts_a_single_time_when_no_token_is_stored() {
        let server = MockServer::start().await;
        mock_authorization(&server, 1).await;

        let mut store = MockApplicationTokenProvider::new();
        store.expect_get().returning(|| Err("no token".into()));
        store.expect_store().times(1).returning(|_| Ok(()));

        let authenticator = Authenticator::new(format!("{}/api/", server.uri()), Box::new(store));

        register_once(&authenticator, 1).await.unwrap();
        server.verify().await;
    }

    #[tokio::test]
    async fn register_once_skips_registration_when_a_token_is_stored() {
        let server = MockServer::start().await;
        mock_authorization(&server, 0).await;

        let mut store = MockApplicationTokenProvider::new();
        store.expect_get().returning(|| Ok("fake".to_string()));
        store.expect_store().never();

        let authenticator = Authenticator::new(format!("{}/api/", server.uri()), Box::new(store));

        register_once(&authenticator, 1).await.unwrap();
        server.verify().await;
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}
