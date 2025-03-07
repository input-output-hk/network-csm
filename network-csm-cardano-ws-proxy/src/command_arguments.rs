use clap::Parser;
use std::net::SocketAddr;
use tracing_subscriber::filter::LevelFilter;

#[derive(Debug, Parser)]
pub struct CommandArguments {
    /// listen to the given address and port number to expose the
    /// websocket proxy URL
    ///
    /// Setting the IP address to 0.0.0.0 will listen globally.
    #[arg(long = "listen", default_value = "0.0.0.0:3000")]
    pub ws_listen_port: SocketAddr,

    /// set the env filter
    #[arg(long = "log-level", default_value = "info")]
    pub log_level: LevelFilter,

    /// Cardano network bootstrap nodes
    #[arg(
        long = "bootstrap-node",
        default_value = "backbone.mainnet.cardanofoundation.org."
    )]
    pub bootstrap_node: String,

    /// Cardano network bootstrap port number
    #[arg(long, default_value_t = 3001)]
    pub bootstrap_port: u16,
}

impl CommandArguments {
    #[inline]
    pub fn collect() -> Self {
        Self::parse()
    }
}
