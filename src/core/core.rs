use log::{error, info};

use crate::{core::discovery, mappers::Mapper};

use super::{
    authenticator::{self, application_token_provider::FileSystemProvider},
    configuration::Configuration,
    prometheus,
};

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
            panic!("Unrecognized freebox mode")
        }
    };
    info!("using api url: {api_url}");

    Ok(api_url)
}

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
        error!("unable to get api url");
    }

    Ok(())
}
