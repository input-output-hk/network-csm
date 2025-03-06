use crate::client::common::{Client, ClientBuilder};
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
    pub async fn unix_connect(self, path: impl AsRef<Path>) -> Result<Client, tokio::io::Error> {
        let stream = UnixStream::connect(path).await.unwrap();
        let (r, w) = stream.into_split();

        Ok(Self::build(self, r, w))
    }
}
