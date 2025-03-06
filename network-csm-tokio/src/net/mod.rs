#[cfg(not(target_arch = "wasm32"))]
mod tokio;
// #[cfg(target_arch = "wasm32")]
// mod wasm32;

#[cfg(not(target_arch = "wasm32"))]
pub use self::tokio::TcpStream;
