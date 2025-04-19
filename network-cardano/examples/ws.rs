use clap::Parser;
use network_cardano::{ClientBuilder, Magic, VersionN2N};
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

#[derive(Debug, Parser)]
struct Arguments {
    #[arg(default_value = "ws://localhost.:3000")]
    url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Arguments { url } = Arguments::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("network_cardano=trace"))
        .with(tracing_subscriber::fmt::layer().pretty())
        .init();

    let mut builder = ClientBuilder::new();
    let mut chainsync = builder.with_n2n_chainsync()?;

    let _client = builder
        .ws_connect(url, VersionN2N::V14, Magic::CARDANO_MAINNET)
        .await?;

    let tip = chainsync.get_tip().await?;

    println!("{tip:?}");

    Ok(())
}
