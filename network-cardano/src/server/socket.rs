use network_csm_cardano_protocols::handshake_n2c;
use network_csm_cardano_protocols::handshake_n2n;
use tokio::net::TcpStream;

#[cfg(not(target_os = "windows"))]
use tokio::net::UnixStream;

use super::{Server, ServerBuilder, ServerError};

impl ServerBuilder {
    /// Use a connected tcp stream as a Server
    pub async fn tcp<F>(self, stream: TcpStream, f: F) -> Result<Server, ServerError>
    where
        F: FnOnce(handshake_n2n::VersionProposal) -> handshake_n2n::ProposeVersionsRet,
    {
        let (r, w) = stream.into_split();
        self.accept_handshake_n2n(r, w, f).await
    }

    /// Use a connected unix stream as a Server
    #[cfg(not(target_os = "windows"))]
    pub async fn unix<F>(self, stream: UnixStream, f: F) -> Result<Server, ServerError>
    where
        F: FnOnce(handshake_n2c::VersionProposal) -> handshake_n2c::ProposeVersionsRet,
    {
        let (r, w) = stream.into_split();
        self.accept_handshake_n2c(r, w, f).await
    }
}
