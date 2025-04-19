use std::net::SocketAddr;

use clap::Parser;
use network_cardano::{ClientBuilder, Magic, VersionN2N};

#[derive(Debug, Parser)]
struct Arguments {
    #[arg(default_value = "147.75.92.75:3001")]
    address: SocketAddr,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Arguments { address } = Arguments::parse();

    let mut builder = ClientBuilder::new();
    let mut chainsync = builder.with_n2n_chainsync()?;

    let _client = builder
        .tcp_connect(address, VersionN2N::V14, Magic::CARDANO_MAINNET)
        .await?;

    let tip = chainsync.get_tip().await?;

    println!("{tip:?}");

    Ok(())
}
