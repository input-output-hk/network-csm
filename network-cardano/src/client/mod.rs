pub mod common;

#[cfg(all(not(target_arch = "wasm32")))]
pub mod tcp;
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
pub mod unix;

pub mod websocket;

use thiserror::Error;

use crate::handshake;

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("The protocol cannot be both n2c and n2n")]
    ProtocolConflict,
    #[error("The protocol must be either n2c or n2n")]
    ProtocolNotSpecified,

    #[error("I/O Error")]
    IoError(#[from] std::io::Error),

    #[error("WebSocket Error")]
    WebSocketError(#[from] reqwest_websocket::Error),

    #[error("Failed to establish secure handshake with peer")]
    Handshake(#[from] handshake::Error),
}
