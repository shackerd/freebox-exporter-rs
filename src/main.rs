use std::net::SocketAddr;

use clap::{command, Parser, Subcommand};
use configuration::{get_configuration, Configuration};
use prometheus_exporter::prometheus::{register_counter, register_gauge, register_histogram, register_int_counter_vec};

mod common;
mod authenticator;
mod discovery;
mod prometheus;
mod configuration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let cli = Cli::parse();

    let conf_path: &str =
        match &cli.configuration_file {
            Some(c) => { &c },
            None => { "config.toml" }
        };

    let conf = get_configuration(conf_path.to_string()).await?;

    match &cli.command {
        Command::Register { pooling_interval } => {
            let interval =
                match &pooling_interval {
                    Some(i) => { *i },
                    None => { 6 }
                };

            register(conf, interval).await?;
        } ,
        Command::Serve { port } => {
            let serve_port =
                match &port {
                    Some(p) => { *p },
                    None => { conf.core.port }
                };

            serve(conf, serve_port).await?;
        },
        Command::Revoke => {
            todo!()
        }
    }

    Ok(())
}

async fn register(conf: Configuration, interval: u64) -> Result<(), Box<dyn std::error::Error>> {
    let api_url = discovery::get_api_url(&conf.api.host.to_owned(), true).await?;

    let authenticator =
        authenticator::Authenticator::new(api_url.to_owned(), conf.core.data_dir);

    match authenticator.register(interval).await {
        Ok(_) => {
            println!("Successfully registered application");
        },
        Err(e) => panic!("{e:#?}")
    }

    Ok(())
}

async fn serve(conf: Configuration, port: u16) -> Result<(), Box<dyn std::error::Error>> {

    let api_url = discovery::get_api_url(conf.api.host.as_str(), true).await?;
    let authenticator =
        authenticator::Authenticator::new(api_url.to_owned(), conf.core.data_dir);

    let client = authenticator.login().await?;

    let server = prometheus::Server::new(port, client);

    server.run().await?;

    Ok(())
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(short, long)]
    configuration_file: Option<String>
}

#[derive(Subcommand)]
enum Command {

    Register { pooling_interval: Option<u64>},
    Serve { port: Option<u16>},
    Revoke
}