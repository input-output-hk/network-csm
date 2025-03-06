use clap::Parser;
use network_cardano::ClientBuilder;

#[derive(Debug, Parser)]
struct Arguments {
    #[arg(default_value = "ws://localhost.:3000")]
    url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Arguments { url } = Arguments::parse();

    let mut builder = ClientBuilder::new();
    let mut handshake = builder.with_n2n_handshake()?;
    let mut chainsync = builder.with_n2n_chainsync()?;

    let _client = builder.ws_connect(url).await?;

    handshake.handshake().await?;
    let tip = chainsync.get_tip().await?;

    println!("{tip:?}");

    Ok(())
}
