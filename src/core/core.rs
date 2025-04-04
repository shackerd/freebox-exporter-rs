use log::info;

use crate::{core::discovery, diagnostics, mappers::Mapper};

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
    let res = get_api_url(conf).await;

    if let Err(e) = res {
        return Err(e);
    }

    let api_url = res.unwrap();

    let authenticator = authenticator::Authenticator::new(
        api_url.to_owned(),
        Box::new(FileSystemProvider::new(
            conf.core.data_directory.as_ref().unwrap().to_owned(),
        )),
    );

    let is_registered = authenticator.is_registered().await;

    if is_registered.unwrap_or_default() {
        info!("application is already registered, logging in");
    } else {
        let res = authenticator.register(interval).await;
        if let Err(e) = res {
            return Err(e);
        }
    }

    let factory = match authenticator.login().await {
        Err(e) => return Err(e),
        Ok(r) => r,
    };

    let mapper = Mapper::new(&factory, conf.to_owned().metrics, conf.to_owned().api);
    let mut server = prometheus::Server::new(port, conf.api.refresh.unwrap_or_else(|| 5), mapper);

    server.run().await
}

/// ### Get the API URL
/// This function will return the API URL based on the configuration
/// It will check if the mode is router or bridge and return the appropriate URL
/// If the mode is not recognized, it will return an error
/// ### Arguments
/// * `conf` - The configuration object
/// ### Returns
/// * `Result<String, Box<dyn std::error::Error + Send + Sync>>` - The API URL
/// ### Errors
/// * `Box<dyn std::error::Error + Send + Sync>` - If the mode is not recognized or if there is an error getting the API URL
/// ### Example
/// ```
/// let conf = Configuration::new();
/// let api_url = get_api_url(&conf).await;
/// assert_eq!(api_url, "https://mafreebox.freebox.fr");
/// ```
/// ### Notes
/// * This function is used to get the API URL based on the configuration
/// * It is used in the `register` and `serve` functions to get the API URL
/// * It is also used in the `auto_register_and_serve` function to get the API URL
/// * It is used in the `session_diagnostic` function to get the API URL
/// * It is used in the `dry_run` function to get the API URL
/// * It is used in the `get_api_url` function to get the API URL
pub async fn get_api_url(
    conf: &Configuration,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let api_url = match conf
        .api
        .mode
        .as_ref()
        .expect("Please specify freebox mode")
        .as_str()
    {
        "router" => match discovery::get_api_url(discovery::DEFAULT_FBX_HOST, 443, true).await {
            Err(e) => return Err(e),
            Ok(r) => r,
        },
        "bridge" => discovery::get_static_api_url().unwrap(),
        _ => {
            return Err(Box::new(
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Unrecognized freebox mode",
                ),
            ));
        }
    };
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
    let res = get_api_url(&conf).await;

    if let Err(e) = res {
        return Err(e);
    }

    let api_url = res?;

    let authenticator = authenticator::Authenticator::new(
        api_url.to_owned(),
        Box::new(FileSystemProvider::new(
            conf.core.data_directory.as_ref().unwrap().to_owned(),
        )),
    );

    authenticator.register(interval).await
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
    let res = get_api_url(&conf).await;

    if let Err(e) = res {
        return Err(e);
    }

    let api_url = res?;

    let authenticator = authenticator::Authenticator::new(
        api_url.to_owned(),
        Box::new(FileSystemProvider::new(
            conf.core.data_directory.as_ref().unwrap().to_owned(),
        )),
    );

    let factory = match authenticator.login().await {
        Err(e) => return Err(e),
        Ok(r) => r,
    };

    let mapper = Mapper::new(&factory, conf.to_owned().metrics, conf.to_owned().api);
    let mut server = prometheus::Server::new(port, conf.api.refresh.unwrap_or_else(|| 5), mapper);

    server.run().await
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
    if let Ok(api_url) = get_api_url(&conf).await {
        let authenticator = authenticator::Authenticator::new(
            api_url.to_owned(),
            Box::new(FileSystemProvider::new(
                conf.core.data_directory.as_ref().unwrap().to_owned(),
            )),
        );

        authenticator.diagnostic(show_token).await?;
    } else {
        return Err(Box::new(
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unable to get api url",
            ),
        ));
    }

    Ok(())
}

/// ### Dry run
/// This function will run the dry run
/// ## Arguments
/// * `conf` - The configuration object
/// * `output_path` - The path to the output file
/// ## Returns
/// * `Result<(), Box<dyn std::error::Error + Send + Sync>>` - The result of the operation
/// ## Errors
/// * `Box<dyn std::error::Error + Send + Sync>` - If there is an error during the operation
/// ## Example
/// ```
/// let conf = Configuration::new();
/// let output_path = "output.txt";
/// let result = dry_run(&conf, output_path).await;
/// assert_eq!(result, Ok(()));
/// ```
/// ## Notes
/// * This function will run the dry run
/// * It will return an error if there is an error during the operation
/// * It will return Ok(()) if the operation is successful
/// * It will return an error if the application is not registered
/// * It will write the output to the specified file
/// * It will return an error if there is an error during the operation
pub async fn dry_run(
    conf: &Configuration,
    output_path: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let res = get_api_url(conf).await;

    if let Err(e) = res {
        return Err(e);
    }

    let api_url = res?;

    let authenticator = authenticator::Authenticator::new(
        api_url.to_owned(),
        Box::new(FileSystemProvider::new(
            conf.core.data_directory.as_ref().unwrap().to_owned(),
        )),
    );

    let factory = match authenticator.login().await {
        Err(e) => return Err(e),
        Ok(r) => r,
    };

    let mut mapper = Mapper::new(&factory, conf.to_owned().metrics, conf.to_owned().api);
    let mut runner = diagnostics::DryRunner::new(mapper.as_dry_runnable(), output_path);
    
    if let Err(e) = runner.run().await {
        return Err(e);
    }

    info!("dry run completed successfully");

    Ok(())
}
