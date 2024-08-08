use clap::{command, Parser, Subcommand};
use flexi_logger::FileSpec;
use log::{error, info};
use mappers::Mapper;
use core::{authenticator, configuration::{get_configuration, Configuration}, discovery, prometheus::{self}};
use std::str::FromStr;
mod core;
mod mappers;

const DEFAULT_CONF_FILE : &str = "config.toml";
const DEFAULT_LOG_LEVEL : &str = "Info";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let cli = Cli::parse();

    let conf_path: &str = &cli.configuration_file.unwrap_or_else(|| DEFAULT_CONF_FILE.to_string());

    let conf = get_configuration(conf_path.to_string()).await?;

    conf.assert_data_dir_permissions()?;
    conf.assert_metrics_prefix_is_not_empty().expect("metrics prefix cannot be empty");

    let specs =
        FileSpec::default()
            .directory(conf.core.data_directory.clone().expect("Please configure data_directory in config.toml"));

    let logger = flexi_logger::Logger::try_with_env_or_str(
        cli.verbosity.unwrap_or_else(
            || log::LevelFilter::from_str(
                &conf.log.level.clone().unwrap_or_else(|| DEFAULT_LOG_LEVEL.to_string())
            ).unwrap()).as_str())?
        .log_to_file(specs)
        .write_mode(flexi_logger::WriteMode::BufferAndFlush)
        .duplicate_to_stdout(flexi_logger::Duplicate::Info)
        .cleanup_in_background_thread(true)
        .rotate(
            flexi_logger::Criterion::Age(flexi_logger::Age::Day),
            flexi_logger::Naming::TimestampsDirect,
            flexi_logger::Cleanup::KeepCompressedFiles(conf.log.retention.unwrap_or_else(|| 31)))
        .start()?;

    match &cli.command {
        Command::Register { pooling_interval } => {
            let interval = pooling_interval.unwrap_or_else(|| 6);

            match register(conf, interval).await {
                Err(e) => error!("{e:#?}"),
                _ => { }
            }
        } ,
        Command::Serve { port } => {
            let serve_port = port.unwrap_or_else(|| conf.core.port.unwrap());

            match serve(conf, serve_port).await {
                Err(e) => error!("{e:#?}"),
                _ => { }
            }
        },
        Command::Revoke => {
            todo!()
        }
    }

    // force flush before exit
    logger.flush();

    Ok(())
}

async fn register(conf: Configuration, interval: u64) -> Result<(), Box<dyn std::error::Error>> {

    let api_url =
        match conf.api.mode.expect("Please specify freebox mode").as_str() {
            "router" => { discovery::get_api_url(discovery::DEFAULT_FBX_HOST).await? },
            "bridge" => { discovery::get_static_api_url().unwrap() }
            _ => { panic!("Unrecognized freebox mode") }
        };

    info!("using api url: {api_url}");

    let authenticator =
        authenticator::Authenticator::new(api_url.to_owned(), conf.core.data_directory.unwrap());

    match authenticator.register(interval).await {
        Ok(_) => info!("Successfully registered application"),
        Err(e) => return Err(e)
    }

    Ok(())
}

async fn serve(conf: Configuration, port: u16) -> Result<(), Box<dyn std::error::Error>> {

    let api_url =
        match conf.api.mode.expect("Please specify freebox mode").as_str() {
            "router" => discovery::get_api_url(discovery::DEFAULT_FBX_HOST).await?,
            "bridge" => discovery::get_static_api_url().unwrap(),
            _ => panic!("Unrecognized freebox mode")
        };

    info!("using api url: {api_url}");

    let authenticator =
        authenticator::Authenticator::new(api_url.to_owned(), conf.core.data_directory.unwrap());

    let login_result = authenticator.login().await;

    match login_result {
        Err(e) => return Err(e),
        _ => {}
    }

    let factory = login_result.unwrap();
    let mapper = Mapper::new(factory, conf.metrics);
    let server = prometheus::Server::new(port, conf.api.refresh.unwrap_or_else(|| 5), mapper);

    server.run().await?;

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
    verbosity: Option<log::LevelFilter>
}

#[derive(Subcommand)]
enum Command {

    Register { pooling_interval: Option<u64>},
    Serve { port: Option<u16>},
    Revoke
}