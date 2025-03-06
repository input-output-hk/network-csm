use std::path::PathBuf;

use clap::Parser;
use network_cardano::ClientBuilder;

#[derive(Debug, Parser)]
struct Arguments {
    path: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Arguments { path } = Arguments::parse();

    let mut builder = ClientBuilder::new();
    let mut handshake = builder.with_n2c_handshake()?;
    let mut chainsync = builder.with_n2c_chainsync()?;

    let _client = builder.unix_connect(path).await?;

    handshake.handshake().await?;
    let tip = chainsync.get_tip().await?;

    println!("{tip:?}");

    Ok(())
}
