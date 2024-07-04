use std::net::SocketAddr;

use clap::{command, Parser, Subcommand};
use prometheus_exporter::prometheus::{register_counter, register_gauge, register_histogram, register_int_counter_vec};

mod common;
mod authenticator;
mod discovery;
mod prometheus;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let cli = Cli::parse();

    let configuration: &str =
        match &cli.configuration_file {
            Some(c) => {
                ""
            },
            None => { "" }
        };

    match &cli.command {
        Command::Register { pooling_interval } => {
            let interval =
                match &pooling_interval {
                    Some(i) => { *i },
                    None => { 6 }
                };

            register(interval).await?;
        } ,
        Command::Serve { port } => {
            let serve_port =
                match &port {
                    Some(p) => { *p },
                    None => { 9186 }
                };

            serve(serve_port).await?;
        },
        Command::Revoke => {
            todo!()
        }
    }

    Ok(())
}

async fn register(interval: u64) -> Result<(), Box<dyn std::error::Error>> {
    let api_url = discovery::get_api_url("mafreebox.freebox.fr", false).await?;

    let authenticator =
        authenticator::Authenticator::new(api_url.to_owned());

    authenticator.register(interval).await?;

    Ok(())
}

async fn serve(port: u16) -> Result<(), Box<dyn std::error::Error>> {

    // let api_url = discovery::get_api_url("mafreebox.freebox.fr", false).await?;
    // let authenticator =
    //     authenticator::Authenticator::new(api_url.to_owned());
    // // server startup procedure
    // let client = authenticator.login().await?;

    let addr_raw = format!("0.0.0.0:{}", port);
    let addr: SocketAddr = addr_raw.parse().expect("Cannot parse addr");
    let exporter = prometheus_exporter::start(addr).expect("Cannot start exporter");
    let duration = std::time::Duration::from_millis(5000);
    let metric1 = register_gauge!("test", "this is a test").expect("cannot create test gauge");

    let mut i = 0;
    loop {
        let guard = exporter.wait_duration(duration);
        metric1.set(i as f64);
        i = i + 1;
    }

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