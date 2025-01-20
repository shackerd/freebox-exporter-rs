use clap::{command, Parser, Subcommand};
use core::{
    authenticator,
    configuration::{get_configuration, Configuration},
    discovery,
    prometheus::{self},
};
use flexi_logger::FileSpec;
use log::{error, info};
use mappers::Mapper;
use std::str::FromStr;
mod core;
mod mappers;

const DEFAULT_CONF_FILE: &str = "config.toml";
const DEFAULT_LOG_LEVEL: &str = "Info";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let conf_path: &str = &cli
        .configuration_file
        .unwrap_or(DEFAULT_CONF_FILE.to_string());

    let conf = get_configuration(conf_path.to_string()).await?;

    conf.assert_data_dir_permissions()?;
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
    )?
    .log_to_file(specs)
    .write_mode(flexi_logger::WriteMode::BufferAndFlush)
    .duplicate_to_stdout(flexi_logger::Duplicate::Info)
    .set_palette("b1;3;2;4;6".to_string())
    .cleanup_in_background_thread(true)
    .rotate(
        flexi_logger::Criterion::Age(flexi_logger::Age::Day),
        flexi_logger::Naming::TimestampsDirect,
        flexi_logger::Cleanup::KeepCompressedFiles(conf.log.retention.unwrap_or_else(|| 31)),
    )
    .start()?;

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
    }

    // force flush before exit
    logger.flush();

    Ok(())
}

async fn register(conf: Configuration, interval: u64) -> Result<(), Box<dyn std::error::Error>> {
    let api_url = match conf.api.mode.expect("Please specify freebox mode").as_str() {
        "router" => match discovery::get_api_url(discovery::DEFAULT_FBX_HOST).await {
            Err(e) => return Err(e),
            Ok(r) => r,
        },
        "bridge" => discovery::get_static_api_url().unwrap(),
        _ => {
            panic!("Unrecognized freebox mode")
        }
    };

    info!("using api url: {api_url}");

    let authenticator =
        authenticator::Authenticator::new(api_url.to_owned(), conf.core.data_directory.unwrap());

    authenticator.register(interval).await
}

async fn serve(conf: Configuration, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let api_url = match conf
        .to_owned()
        .api
        .mode
        .expect("Please specify freebox mode")
        .as_str()
    {
        "router" => match discovery::get_api_url(discovery::DEFAULT_FBX_HOST).await {
            Err(e) => return Err(e),
            Ok(r) => r,
        },
        "bridge" => discovery::get_static_api_url().unwrap(),
        _ => panic!("Unrecognized freebox mode"),
    };

    info!("using api url: {api_url}");

    let authenticator = authenticator::Authenticator::new(
        api_url.to_owned(),
        conf.to_owned().core.data_directory.unwrap(),
    );

    let factory = match authenticator.login().await {
        Err(e) => return Err(e),
        Ok(r) => r,
    };

    let mapper = Mapper::new(factory, conf.to_owned().metrics, conf.to_owned().api);
    let mut server = prometheus::Server::new(port, conf.api.refresh.unwrap_or_else(|| 5), mapper);

    server.run().await
}

async fn get_api_url(conf: &Configuration) -> Result<String, Box<dyn std::error::Error>> {
    let api_url = match conf
        .to_owned()
        .api
        .mode
        .expect("Please specify freebox mode")
        .as_str()
    {
        "router" => match discovery::get_api_url(discovery::DEFAULT_FBX_HOST).await {
            Err(e) => return Err(e),
            Ok(r) => r,
        },
        "bridge" => discovery::get_static_api_url().unwrap(),
        _ => panic!("Unrecognized freebox mode"),
    };

    Ok(api_url)
}

async fn session_diagnostic(
    conf: Configuration,
    show_token: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(api_url) = get_api_url(&conf).await {
        info!("using api url: {api_url}");

        let authenticator = authenticator::Authenticator::new(
            api_url.to_owned(),
            conf.to_owned().core.data_directory.unwrap(),
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
    Register { pooling_interval: Option<u64> },
    Serve { port: Option<u16> },
    SessionDiagnostic { show_token: Option<bool> },
    Revoke,
}
