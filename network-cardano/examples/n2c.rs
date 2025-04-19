use std::path::PathBuf;

use clap::Parser;
use network_cardano::{ClientBuilder, Magic, VersionN2C};

#[derive(Debug, Parser)]
struct Arguments {
    path: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Arguments { path } = Arguments::parse();

    let mut builder = ClientBuilder::new();
    let mut chainsync = builder.with_n2c_chainsync()?;

    let _client = builder
        .unix_connect(path, VersionN2C::V20, Magic::CARDANO_MAINNET)
        .await?;

    let tip = chainsync.get_tip().await?;

    println!("{tip:?}");

    Ok(())
}
