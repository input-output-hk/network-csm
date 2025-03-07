use crate::client::{
    ConnectionError,
    common::{Client, ClientBuilder},
};
use std::net::SocketAddr;
use tokio::net::TcpStream;

impl ClientBuilder {
    /// connect to the remote IP address and port number with a TCP connection
    ///
    /// # Supported protocols
    ///
    /// * [`handshake_n2n`]
    /// * [`blockfetch`]
    /// * [`chainsync_n2n`]
    /// * [`keepalice`]
    /// * [`peersharing`]
    /// * [`tx_submission`]
    ///
    pub async fn tcp_connect(self, address: SocketAddr) -> Result<Client, ConnectionError> {
        let stream = TcpStream::connect(address).await?;

        let (r, w) = stream.into_split();

        Self::build(self, r, w).await
    }
}
