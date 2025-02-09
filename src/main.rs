use clap::{command, Parser, Subcommand};
use core::{
    authenticator::{self, app_token_storage::FileStorage},
    configuration::{get_configuration, Configuration},
    discovery,
    prometheus::{self},
};
use flexi_logger::{
    filter::{self, LogLineFilter},
    FileSpec,
};
use log::{error, info};
use mappers::Mapper;
use std::str::FromStr;
mod core;
mod mappers;

const DEFAULT_CONF_FILE: &str = "config.toml";
const DEFAULT_LOG_LEVEL: &str = "Info";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();

    let conf_path: &str = &cli
        .configuration_file
        .unwrap_or(DEFAULT_CONF_FILE.to_string());

    let conf = get_configuration(conf_path.to_string()).await.unwrap();

    conf.assert_data_dir_permissions().unwrap();
    conf.assert_metrics_prefix_is_not_empty()
        .expect("metrics prefix cannot be empty");

    let specs = FileSpec::default().directory(
        conf.core
            .data_directory
            .clone()
            .expect("Please configure data_directory in config.toml"),
    );

    let logger = flexi_logger::Logger::try_with_env_or_str(
        cli.verbosity
            .unwrap_or(
                log::LevelFilter::from_str(
                    &conf
                        .log
                        .level
                        .clone()
                        .unwrap_or(DEFAULT_LOG_LEVEL.to_string()),
                )
                .unwrap(),
            )
            .as_str(),
    )
    .unwrap()
    .filter(Box::new(IgnoreReqwest))
    .log_to_file(specs)
    .write_mode(flexi_logger::WriteMode::BufferAndFlush)
    .duplicate_to_stdout(flexi_logger::Duplicate::Debug)
    .set_palette("b1;3;2;4;6".to_string())
    .cleanup_in_background_thread(true)
    .rotate(
        flexi_logger::Criterion::Age(flexi_logger::Age::Day),
        flexi_logger::Naming::TimestampsDirect,
        flexi_logger::Cleanup::KeepCompressedFiles(conf.log.retention.unwrap_or_else(|| 31)),
    )
    .start()
    .unwrap();

    info!(
        "freebox exporter: {version}",
        version = env!("CARGO_PKG_VERSION")
    );

    match &cli.command {
        Command::Register { pooling_interval } => {
            let interval = pooling_interval.unwrap_or_else(|| 6);

            if let Err(e) = register(conf, interval).await {
                error!("{e:#?}")
            }
        }
        Command::Serve { port } => {
            let serve_port = port.unwrap_or_else(|| conf.core.port.unwrap());

            if let Err(e) = serve(conf, serve_port).await {
                error!("{e:#?}")
            }
        }

        Command::Revoke => {
            todo!()
        }
        Command::SessionDiagnostic { show_token } => {
            if let Err(e) = session_diagnostic(conf, show_token.unwrap_or_else(|| false)).await {
                error!("{e:#?}");
            }
        }
        Command::Auto {
            pooling_interval,
            port,
        } => {
            let interval = pooling_interval.unwrap_or_else(|| 6);
            let serve_port = port.unwrap_or_else(|| conf.core.port.unwrap());

            if let Err(e) = auto_register_and_serve(&conf, interval, serve_port).await {
                error!("{e:#?}");
            }
        }
    }

    // force flush before exit
    logger.flush();

    Ok(())
}

async fn auto_register_and_serve(
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
        Box::new(FileStorage::new(
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

async fn get_api_url(
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

async fn register(
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
        Box::new(FileStorage::new(
            conf.core.data_directory.as_ref().unwrap().to_owned(),
        )),
    );

    authenticator.register(interval).await
}

async fn serve(
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
        Box::new(FileStorage::new(
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

async fn session_diagnostic(
    conf: Configuration,
    show_token: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Ok(api_url) = get_api_url(&conf).await {
        let authenticator = authenticator::Authenticator::new(
            api_url.to_owned(),
            Box::new(FileStorage::new(
                conf.core.data_directory.as_ref().unwrap().to_owned(),
            )),
        );

        authenticator.diagnostic(show_token).await?;
    } else {
        error!("unable to get api url");
    }

    Ok(())
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(short, long)]
    configuration_file: Option<String>,
    #[arg(short, long)]
    verbosity: Option<log::LevelFilter>,
}

#[derive(Subcommand)]
enum Command {
    /// starts the application and registers it if necessary
    Auto {
        /// the interval in seconds to check for user validation in registration process
        pooling_interval: Option<u64>,
        /// the port to serve the metrics on
        port: Option<u16>,
    },
    /// registers the application
    Register {
        /// the interval in seconds to check for user validation in registration process
        pooling_interval: Option<u64>,
    },
    /// starts the application
    Serve {
        /// the port to serve the metrics on
        port: Option<u16>,
    },
    /// runs a diagnostic on the session
    SessionDiagnostic {
        /// show the token
        show_token: Option<bool>,
    },
    Revoke,
}

struct IgnoreReqwest;

impl LogLineFilter for IgnoreReqwest {
    fn write(
        &self,
        now: &mut flexi_logger::DeferredNow,
        record: &log::Record,
        log_line_writer: &dyn filter::LogLineWriter,
    ) -> std::io::Result<()> {
        if record
            .module_path()
            .unwrap_or_default()
            .starts_with("reqwest")
        {
            return Ok(());
        }

        log_line_writer.write(now, record)
    }
}
