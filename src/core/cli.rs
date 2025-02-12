use clap::{arg, command, Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
    #[arg(short, long)]
    pub configuration_file: Option<String>,
    #[arg(short, long)]
    pub verbosity: Option<log::LevelFilter>,
}

#[derive(Subcommand)]
pub enum Command {
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
