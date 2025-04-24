use crate::client::{
    ConnectionError,
    common::{Client, ClientBuilder},
};
use network_csm_cardano_protocols::{handshake_n2c, handshake_n2n};
use std::path::Path;
use tokio::net::UnixStream;

impl ClientBuilder {
    /// connect to the UNIX Pipe
    ///
    /// # Supported protocols
    ///
    /// * [`handshake_n2c`]
    /// * [`chainsync_n2c`]
    /// * [`local_tx_submission`]
    ///
    pub async fn unix_connect(
        self,
        path: impl AsRef<Path>,
        version: handshake_n2c::Version,
        magic: handshake_n2n::Magic,
    ) -> Result<Client, ConnectionError> {
        let stream = UnixStream::connect(path).await?;
        self.unix(stream, version, magic).await
    }

    pub async fn unix(
        self,
        stream: UnixStream,
        version: handshake_n2c::Version,
        magic: handshake_n2n::Magic,
    ) -> Result<Client, ConnectionError> {
        let (r, w) = stream.into_split();
        Self::build_n2c(self, r, w, version, magic).await
    }
}
