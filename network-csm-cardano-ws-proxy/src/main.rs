mod command_arguments;

use self::command_arguments::CommandArguments;
use anyhow::{Context as _, Result, anyhow, bail};
use axum::{
    Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::Response,
    routing::any,
};
use futures::{
    SinkExt,
    stream::{SplitSink, SplitStream, StreamExt},
};
use network_csm::Demux;
use std::{
    io::ErrorKind,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    str::FromStr as _,
    sync::{Arc, atomic::AtomicBool},
};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpStream, tcp},
};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing_futures::Instrument;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

#[derive(Clone)]
struct ProxyState {
    pub bootstrap_node: Arc<String>,
    pub bootstrap_port: Arc<u16>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let command_arguments = CommandArguments::collect();

    let ps = ProxyState {
        bootstrap_node: Arc::new(command_arguments.bootstrap_node),
        bootstrap_port: Arc::new(command_arguments.bootstrap_port),
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(command_arguments.log_level.into())
                .parse_lossy("hickory_proto=warn,network_csm_cardano_ws_proxy=debug"),
        )
        .with(tracing_subscriber::fmt::layer().pretty())
        .init();

    let app = Router::new()
        .route("/", any(handler))
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()))
        .with_state(ps);

    let listener = tokio::net::TcpListener::bind(&command_arguments.ws_listen_port)
        .await
        .with_context(|| {
            anyhow!(
                "Failed to listen to the socket address: {}",
                command_arguments.ws_listen_port
            )
        })?;
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn handler(ws: WebSocketUpgrade, State(ps): State<ProxyState>) -> Response {
    ws.on_upgrade(|ws| handle_socket(ws, ps))
}

pub struct Reader {
    receiver: SplitStream<WebSocket>,
    demux: Demux,

    writer: tcp::OwnedWriteHalf,
    disconnected: Arc<AtomicBool>,
}

pub struct Writer {
    receiver: tcp::OwnedReadHalf,
    demux: Demux,

    writer: SplitSink<WebSocket, Message>,
    disconnected: Arc<AtomicBool>,
}

#[tracing::instrument(skip(socket, ps))]
async fn handle_socket(socket: WebSocket, ps: ProxyState) {
    let (ws_writer, ws_receiver) = socket.split();

    match connect_to(ps.bootstrap_node.as_ref(), *ps.bootstrap_port).await {
        Err(error) => {
            tracing::warn!(%error, "Failed to establish a connection to the remote cardano node");
        }
        Ok((addr, stream)) => {
            let span = tracing::info_span!("Connected", %addr);
            let _span = span.enter();

            tracing::debug!("Connected");

            let (tcp_receiver, tcp_writer) = stream.into_split();
            let disconnected = Arc::new(AtomicBool::new(false));

            let reader = Reader {
                receiver: ws_receiver,
                demux: Demux::new(),
                writer: tcp_writer,
                disconnected: Arc::clone(&disconnected),
            };

            let writer = Writer {
                receiver: tcp_receiver,
                demux: Demux::new(),
                writer: ws_writer,
                disconnected,
            };

            tokio::spawn(cardano_to_ws(writer).in_current_span());
            tokio::spawn(ws_to_cardano(reader).in_current_span());
        }
    }
}

#[tracing::instrument(skip(reader))]
async fn ws_to_cardano(mut reader: Reader) {
    tracing::debug!("Waiting to receive messages.");

    'outer: while let Some(msg) = reader.receiver.next().await {
        tracing::debug!(?msg, "message received");
        let msg = match msg {
            Ok(msg) => msg,
            Err(error) => {
                tracing::error!(%error, "WebSocket error");
                break;
            }
        };

        let Message::Binary(bytes) = msg else {
            tracing::error!(?msg, "unsupported message format");
            break;
        };
        let mut data: &[u8] = &bytes[..];

        while !data.is_empty() {
            let (size, result) = reader.demux.ingress(data);

            match result {
                network_csm::DemuxResult::Continue => (),
                network_csm::DemuxResult::HeaderReceived(header)
                | network_csm::DemuxResult::DataAppend(header, ..) => {
                    tracing::debug!(id = ?header.id(), time = ?header.time(), direction = ?header.direction(), length = %header.payload_length(), "data received");

                    if let Err(error) = reader.writer.write_all(&data[..size]).await {
                        tracing::error!(%error, "Failed to forward bytes");
                        break 'outer;
                    }
                }
            }

            data = &data[size..];
        }
    }

    tracing::warn!("WebSocket disconnected");

    reader
        .disconnected
        .store(true, std::sync::atomic::Ordering::Release);
}

#[tracing::instrument(skip(writer))]
async fn cardano_to_ws(mut writer: Writer) {
    'outer: while writer.receiver.readable().await.is_ok()
        && !writer
            .disconnected
            .load(std::sync::atomic::Ordering::Acquire)
    {
        let mut buff = [0; 16384];
        let mut data = match writer.receiver.try_read(&mut buff) {
            Ok(size) => &buff[..size],
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                continue;
            }
            Err(error) => {
                tracing::error!(%error, "Failed to receive data from TCP connection");
                break;
            }
        };

        while !data.is_empty() {
            let (size, result) = writer.demux.ingress(data);

            match result {
                network_csm::DemuxResult::Continue => (),
                network_csm::DemuxResult::HeaderReceived(header)
                | network_csm::DemuxResult::DataAppend(header, ..) => {
                    tracing::debug!(id = ?header.id(), time = ?header.time(), direction = ?header.direction(), length = %header.payload_length(), "data received");

                    let msg = Message::binary(data[..size].to_vec());
                    if let Err(error) = writer.writer.send(msg).await {
                        tracing::error!(%error, "Failed to forward bytes");
                        break 'outer;
                    }
                }
            }

            data = &data[size..];
        }
    }

    tracing::warn!("Cardano Node disconnected");

    writer
        .disconnected
        .store(true, std::sync::atomic::Ordering::Release);
}

async fn connect_to(destination: &str, port: u16) -> Result<(SocketAddr, TcpStream)> {
    let ip_addresses = resolve_name(destination)
        .await
        .with_context(|| anyhow!("Failed to resolve `{destination}'"))?;

    // try to connect from (resolved) ip addresses at the expected port
    for ip_addr in ip_addresses.clone() {
        let addr = SocketAddr::new(ip_addr, port);
        tracing::debug!(%ip_addr, "Trying to establish TCP connection");
        let stream = TcpStream::connect(&addr)
            .await
            .with_context(|| anyhow!("Failed to connect to `{addr}' ({destination})"))?;

        return Ok((addr, stream));
    }

    bail!("Failed to find a connection to {destination}:{port} ({ip_addresses:?})")
}

async fn resolve_name(destination: &str) -> Result<Vec<IpAddr>> {
    let ip = match Ipv4Addr::from_str(destination) {
        Ok(addr) => Some(IpAddr::V4(addr)),
        Err(_) => match Ipv6Addr::from_str(destination) {
            Ok(addr6) => Some(IpAddr::V6(addr6)),
            Err(_) => None,
        },
    };

    match ip {
        // possibly a host then
        None => {
            let resolver = hickory_resolver::TokioAsyncResolver::tokio(
                hickory_resolver::config::ResolverConfig::default(),
                hickory_resolver::config::ResolverOpts::default(),
            );
            let response = resolver
                .lookup_ip(destination)
                .await
                .with_context(|| anyhow!("name resolution error for {destination}"))?;
            let addresses = response.iter().collect::<Vec<_>>();
            Ok(addresses)
        }
        Some(ip) => Ok(vec![ip]),
    }
}
