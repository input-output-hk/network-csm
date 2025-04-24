use crate::client::{
    ConnectionError,
    common::{Client, ClientBuilder},
};
use network_csm_cardano_protocols::handshake_n2n;
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
    pub async fn tcp_connect(
        self,
        address: SocketAddr,
        version: handshake_n2n::Version,
        magic: handshake_n2n::Magic,
    ) -> Result<Client, ConnectionError> {
        let stream = TcpStream::connect(address).await?;
        self.tcp(stream, version, magic).await
    }

    pub async fn tcp(
        self,
        stream: TcpStream,
        version: handshake_n2n::Version,
        magic: handshake_n2n::Magic,
    ) -> Result<Client, ConnectionError> {
        let (r, w) = stream.into_split();
        Self::build_n2n(self, r, w, version, magic).await
    }
}
