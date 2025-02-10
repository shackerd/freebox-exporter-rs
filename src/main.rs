use core::{
    cli::{Cli, Command},
    configuration::get_configuration,
    core::{auto_register_and_serve, register, serve, session_diagnostic},
    logger::IgnoreReqwest,
};

use clap::Parser;
use flexi_logger::FileSpec;
use log::{error, info};
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

    let specs = FileSpec::default().directory(conf.core.data_directory.clone().unwrap());

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
    .format_for_files(flexi_logger::detailed_format)
    .format_for_stdout(flexi_logger::detailed_format)
    .format_for_stderr(flexi_logger::detailed_format)
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
