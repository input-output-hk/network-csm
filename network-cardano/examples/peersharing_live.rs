use anyhow::Result;
use clap::Parser;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Instant;
use tokio::time::{Duration, sleep};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

use network_cardano::{ClientBuilder, VersionN2N, peersharing::PeerSharingClient};
use network_csm_cardano_protocols::handshake_n2n::{
    DiffusionMode, Magic as HandshakeMagic, PeerSharing,
};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(value_delimiter = ' ')]
    seeds: Vec<String>,

    #[arg(long, default_value_t = 8)]
    reply_timeout_secs: u64,

    #[arg(long, default_value_t = 5)]
    count: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    let seeds = if args.seeds.is_empty() {
        load_bootstraps_from_env()
    } else {
        args.seeds
    };

    if seeds.is_empty() {
        eprintln!("No seeds provided. Pass them on the CLI OR set BOOTSTRAP1/2/3 env vars.");
        std::process::exit(2);
    }

    info!("🌐 PeerSharing (client-only, enhanced)");
    info!("Seeds: {:?}", seeds);

    let mut unique: HashSet<SocketAddr> = HashSet::new();

    for s in seeds {
        for addr in resolve(&s).await {
            let mut builder = ClientBuilder::new();
            let mut ps = builder.with_peersharing()?;

            // Simulate handshake info manually
            let diffusion_mode = DiffusionMode::InitiatorAndResponder;
            let peer_sharing = PeerSharing::Enabled;

            // ⏱️ Measure RTT
            let start = Instant::now();
            let connect_result = builder
                .tcp_connect(addr, VersionN2N::V14, HandshakeMagic(1))
                .await;
            let rtt = start.elapsed().as_millis();

            match connect_result {
                Ok(_) => {
                    println!(
                        "\n🌍 {addr}\n  RTT: {rtt} ms\n  Mode: {diffusion_mode:?}\n  PeerSharing: {peer_sharing:?}"
                    );

                    match request_once_with_timeout(
                        &mut ps,
                        args.count,
                        Duration::from_secs(args.reply_timeout_secs),
                    )
                    .await
                    {
                        Ok(peers) if peers.is_empty() => println!("  → no peers returned\n"),
                        Ok(peers) => {
                            println!("  → {} peers returned:", peers.len());
                            for p in peers {
                                if unique.insert(p) {
                                    println!("     - {p}");
                                }
                            }
                        }
                        Err(e) => warn!("{addr} → {e}"),
                    }
                }
                Err(e) => warn!("{addr} connection failed: {e:?}"),
            }

            sleep(Duration::from_millis(200)).await;
        }
    }

    eprintln!("\n== Summary ==\nunique peers: {}", unique.len());
    Ok(())
}

async fn request_once_with_timeout(
    ps: &mut PeerSharingClient,
    count: u16,
    timeout: Duration,
) -> Result<Vec<SocketAddr>> {
    use tokio::time::timeout as to;
    Ok(to(timeout, ps.request_once(count))
        .await
        .map_err(|_| anyhow::anyhow!("PeerSharing timed out (no reply)"))??)
}

async fn resolve(s: &str) -> Vec<SocketAddr> {
    if let Ok(sa) = s.parse::<SocketAddr>() {
        return vec![sa];
    }
    if let Some((host, port)) = s.rsplit_once(':') {
        if let Ok(port) = port.parse::<u16>() {
            if let Ok(iter) = tokio::net::lookup_host((host, port)).await {
                return iter.collect();
            }
        }
    }
    warn!("DNS resolution failed for {s}");
    vec![]
}

fn load_bootstraps_from_env() -> Vec<String> {
    let mut v = Vec::new();
    if let Ok(s) = std::env::var("BOOTSTRAP1") {
        if !s.is_empty() {
            v.push(s);
        }
    }
    if let Ok(s) = std::env::var("BOOTSTRAP2") {
        if !s.is_empty() {
            v.push(s);
        }
    }
    if let Ok(s) = std::env::var("BOOTSTRAP3") {
        if !s.is_empty() {
            v.push(s);
        }
    }
    v
}
