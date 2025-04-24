use crate::channel::{AsyncRawChannel, HandleChannels, Sending};
use network_csm::{ChannelsMap, Demux, DemuxResult, Direction, HEADER_SIZE, Id, Mux, OnDirection};
use std::sync::{Arc, atomic::AtomicU64};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::Notify,
};

pub struct Handle {
    pub channels: ChannelsMap<OnDirection<AsyncRawChannel>>,
    #[allow(unused)]
    #[cfg(not(target_arch = "wasm32"))]
    mux_task: tokio::task::JoinHandle<()>,
    #[allow(unused)]
    #[cfg(target_arch = "wasm32")]
    mux_task: (),
    #[allow(unused)]
    #[cfg(not(target_arch = "wasm32"))]
    demux_task: tokio::task::JoinHandle<Result<(), DemuxError>>,
    #[allow(unused)]
    #[cfg(target_arch = "wasm32")]
    demux_task: (),
    bytes_read: Arc<AtomicU64>,
    bytes_written: Arc<AtomicU64>,
}

async fn muxer_task<S: AsyncWrite + Unpin>(
    mut stream: S,
    mux_notifier: Arc<Notify>,
    mut mux: Mux,
    channels: ChannelsMap<OnDirection<AsyncRawChannel>>,
) {
    pub enum MuxResult {
        Full,
        NothingToSend,
        Written,
    }

    fn mux_chan(mux: &mut Mux, channel_id: Id, channel: &AsyncRawChannel) -> MuxResult {
        let writable = mux.writable();

        // no need to continue in the loop if we don't have enough
        // writable bytes for a header and some payload
        if writable.len() < HEADER_SIZE + PAYLOAD_MINIMUM {
            return MuxResult::Full;
        }

        let mut channel_buf = channel.to_send.lock().unwrap();
        let mut channel_sending_var: Option<Sending> = None;
        std::mem::swap(&mut *channel_buf, &mut channel_sending_var);

        if let Some(channel_sending) = &mut channel_sending_var {
            let max_payload_writable = writable.len() - HEADER_SIZE;
            let max_writable = max_payload_writable.min(channel_sending.left().len());
            let to_send = &channel_sending.left()[0..max_writable];

            let written = match mux.egress(channel_id, channel.direction, to_send) {
                Ok(()) => to_send.len(),
                Err(_) => {
                    // something went wrong
                    0
                }
            };
            channel_sending.advance(written);
            if !channel_sending.left().is_empty() {
                std::mem::swap(&mut *channel_buf, &mut channel_sending_var);
            } else {
                channel.sending_notify.notify_one()
            }
            MuxResult::Written
        } else {
            MuxResult::NothingToSend
        }
    }

    const PAYLOAD_MINIMUM: usize = 4;
    loop {
        // iterate over all channels, and try to stuff data from the channel into the muxer
        //
        // TODO: replace by a fair'er implementation:
        // currently it process all channels always in the same order, so
        // some channel might have "preferential" access.
        for (&channel_id, dir_channel) in channels.iterate() {
            let (c1, c2) = dir_channel.split();
            if let Some(c1) = c1 {
                match mux_chan(&mut mux, channel_id, c1) {
                    MuxResult::Full => break,
                    MuxResult::NothingToSend => (),
                    MuxResult::Written => (),
                }
            }
            if let Some(c2) = c2 {
                match mux_chan(&mut mux, channel_id, c2) {
                    MuxResult::Full => break,
                    MuxResult::NothingToSend => (),
                    MuxResult::Written => (),
                }
            }
        }

        let work = mux.work();
        if !work.is_empty() {
            match stream.write(work).await {
                Err(_e) => {
                    break;
                }
                Ok(bytes) => mux.consume(bytes),
            }
        } else {
            // wait for work
            mux_notifier.notified().await;
        }
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum DemuxError {
    #[error("I/O Error")]
    IoError(#[source] Arc<std::io::Error>),
    #[error("Invalid channel {0:?} {1:?}")]
    InvalidChannel(Id, Direction),
    #[error("Full channel {0:?} {1:?}")]
    FullChannel(Id, Direction),
}

async fn demuxer_task<R: AsyncRead + Unpin>(
    mut stream: R,
    demux_notify: Arc<Notify>,
    mut demux: Demux,
    channels: ChannelsMap<OnDirection<AsyncRawChannel>>,
) -> Result<(), DemuxError> {
    let mut buf = vec![0; 16384];
    let r = 'outer: loop {
        let bytes = match stream.read(&mut buf).await {
            Ok(b) => b,
            Err(e) => {
                break Err(DemuxError::IoError(Arc::new(e)));
            }
        };

        let mut data = &buf[0..bytes];
        while !data.is_empty() {
            let (sz, ret) = demux.ingress(data);
            match ret {
                DemuxResult::Continue => {
                    data = &data[sz..];
                }
                DemuxResult::HeaderReceived(header) => {
                    let directional_chans = channels.dispatch(header.id());
                    let dir = !header.direction();

                    let Some(directional_chans) = directional_chans else {
                        // TODO shutdown the connection
                        break 'outer Err(DemuxError::InvalidChannel(header.id(), dir));
                    };
                    if !directional_chans.has_direction(dir) {
                        break 'outer Err(DemuxError::InvalidChannel(header.id(), dir));
                    }
                    data = &data[sz..];
                }
                DemuxResult::DataAppend(header, _finished, to_append) => {
                    // it's guaranteed to be a valid channel here
                    let channel = channels.dispatch(header.id()).unwrap();
                    let channel = channel.get(!header.direction()).unwrap();

                    let mut buf = channel.raw_channel.buf_received();
                    let appended = buf.append(to_append);
                    if appended < to_append.len() {
                        // TODO apply back pressure
                        break 'outer Err(DemuxError::FullChannel(header.id(), header.direction()));
                    } else {
                        channel.r_notify.notify_waiters();
                        demux_notify.notify_waiters();
                        data = &data[sz..];
                    }
                }
            }
        }
    };
    for (_id, chan) in channels.iterate() {
        match chan {
            OnDirection::Initiator(chan) => chan.terminate(),
            OnDirection::Responder(chan) => chan.terminate(),
            OnDirection::InitiatorAndResponder(chan1, chan2) => {
                chan1.terminate();
                chan2.terminate()
            }
        }
    }
    r
}

impl Handle {
    /// Return the number of bytes read and written respectively
    pub fn stats(&self) -> (u64, u64) {
        (
            self.bytes_read.load(std::sync::atomic::Ordering::Relaxed),
            self.bytes_written
                .load(std::sync::atomic::Ordering::Relaxed),
        )
    }

    pub fn create<R, W>(read_stream: R, write_stream: W, channels: HandleChannels) -> Self
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let mux = Mux::new(16_384);
        let demux = Demux::new();

        let bytes_written = mux.bytes_written.clone();
        let bytes_read = demux.bytes_read.clone();

        let demux_notify = Arc::new(Notify::new());

        let mux_notify = channels.mux_notify.clone();
        let channels = channels.finalize();

        #[cfg(not(target_arch = "wasm32"))]
        let mux_task = {
            let channels = channels.clone();
            tokio::spawn(async { muxer_task(write_stream, mux_notify, mux, channels).await })
        };
        #[cfg(target_arch = "wasm32")]
        let mux_task = {
            let channels = channels.clone();
            wasm_bindgen_futures::spawn_local(async {
                muxer_task(write_stream, mux_notify, mux, channels).await
            })
        };

        #[cfg(not(target_arch = "wasm32"))]
        let demux_task = {
            let demux_notify = demux_notify.clone();
            let channels = channels.clone();
            tokio::spawn(async { demuxer_task(read_stream, demux_notify, demux, channels).await })
        };
        #[cfg(target_arch = "wasm32")]
        let demux_task = {
            let demux_notify = demux_notify.clone();
            let channels = channels.clone();
            wasm_bindgen_futures::spawn_local(async {
                demuxer_task(read_stream, demux_notify, demux, channels)
                    .await
                    .unwrap()
            })
        };

        Handle {
            mux_task,
            demux_task,
            bytes_read,
            bytes_written,
            channels,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod extra {
    use super::*;

    use crate::net::TcpStream;
    use hickory_resolver::{
        TokioAsyncResolver,
        config::{ResolverConfig, ResolverOpts},
    };
    use std::{
        net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
        str::FromStr,
    };
    use tracing::{debug, info};

    impl Handle {
        #[cfg(not(target_os = "windows"))]
        pub async fn connect_unix<P: AsRef<std::path::Path>>(
            path: P,
            channels: HandleChannels,
        ) -> Result<Self, std::io::Error> {
            let stream = tokio::net::UnixStream::connect(path).await?;
            let (read_stream, write_stream) = stream.into_split();
            let handle = Self::create(read_stream, write_stream, channels);
            Ok(handle)
        }

        pub async fn connect_tcp(
            dest: &[(&str, u16)],
            channels: HandleChannels,
        ) -> Result<Self, std::io::Error> {
            let (_sockaddr, stream) = connect_to(dest)
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", e)))?;

            let (read_stream, write_stream) = stream.into_split();

            let handle = Self::create(read_stream, write_stream, channels);

            Ok(handle)
        }
    }

    async fn connect_to(
        destinations: &[(&str, u16)],
    ) -> Result<(SocketAddr, TcpStream), Vec<std::io::Error>> {
        let mut errors = Vec::new();

        for (dest, port) in destinations {
            let ip_addresses = match resolve_name(dest).await {
                Ok(r) => r,
                Err(e) => {
                    errors.push(std::io::Error::new(std::io::ErrorKind::Other, e));
                    continue;
                }
            };

            // try to connect from (resolved) ip addresses at the expected port
            for ip_addr in ip_addresses {
                let addr = SocketAddr::new(ip_addr, *port);
                debug!("trying to connect to {}:{} ({})", dest, port, ip_addr);
                match TcpStream::connect(&addr).await {
                    Err(e) => errors.push(e),
                    Ok(stream) => {
                        info!("connected to {}:{} ({})", dest, port, ip_addr);
                        return Ok((addr, stream));
                    }
                }
            }
        }

        Err(errors)
    }

    async fn resolve_name(dest: &str) -> Result<Vec<IpAddr>, String> {
        let ip = match Ipv4Addr::from_str(dest) {
            Ok(addr) => Some(IpAddr::V4(addr)),
            Err(_) => match Ipv6Addr::from_str(dest) {
                Ok(addr6) => Some(IpAddr::V6(addr6)),
                Err(_) => None,
            },
        };

        match ip {
            // possibly a host then
            None => {
                let resolver =
                    TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());
                let response = resolver.lookup_ip(dest).await.map_err(|resolve_err| {
                    format!("name resolution error for {}: {}", dest, resolve_err)
                })?;
                let addresses = response.iter().collect::<Vec<_>>();
                Ok(addresses)
            }
            Some(ip) => Ok(vec![ip]),
        }
    }
}
