use crate::client::{
    ConnectionError,
    common::{Client, ClientBuilder},
};
use futures::Sink;
use reqwest_websocket::RequestBuilderExt as _;
use std::{pin::Pin, task::ready};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};

struct WebSocket {
    inner: reqwest_websocket::WebSocket,
}

unsafe impl Send for WebSocket {}

#[derive(Debug, Error)]
pub enum WsConnectError {
    #[error("Failed to connect to the given websocket path")]
    WsError(#[from] reqwest_websocket::Error),
}

impl ClientBuilder {
    /// connect to the given websocket
    ///
    pub async fn ws_connect(self, path: String) -> Result<Client, ConnectionError> {
        let response = reqwest::Client::default()
            .get(path)
            .upgrade() // Prepares the WebSocket upgrade.
            .send()
            .await?;

        // Turns the response into a WebSocket stream.
        let websocket = WebSocket {
            inner: response.into_websocket().await?,
        };

        // let stream = UnixStream::connect(path).await.unwrap();
        let (r, w) = tokio::io::split(websocket);

        Self::build(self, r, w).await
    }
}

impl AsyncRead for WebSocket {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        use futures::Stream as _;

        let res = unsafe { self.map_unchecked_mut(|ws| &mut ws.inner).poll_next(cx) };
        let Some(msg) = ready!(res) else {
            cx.waker().wake_by_ref();
            return std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "Disconnected",
            )));
        };

        let msg = match msg {
            Err(error) => {
                cx.waker().wake_by_ref();
                return std::task::Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    error,
                )));
            }
            Ok(msg) => msg,
        };

        match msg {
            reqwest_websocket::Message::Binary(items) => {
                buf.put_slice(&items);

                std::task::Poll::Ready(Ok(()))
            }

            reqwest_websocket::Message::Text(..) => todo!(),
            reqwest_websocket::Message::Ping(..) => todo!(),
            reqwest_websocket::Message::Pong(..) => todo!(),
            reqwest_websocket::Message::Close { .. } => todo!(),
        }
    }
}

impl AsyncWrite for WebSocket {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        use futures::Sink as _;
        let this = Pin::into_inner(self);

        let inner = std::pin::pin!(&mut this.inner);
        let res = inner.poll_ready(cx);
        if let Err(error) = ready!(res) {
            cx.waker().wake_by_ref();
            return std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                error,
            )));
        }

        let msg = reqwest_websocket::Message::Binary(buf.to_vec());
        let inner = std::pin::pin!(&mut this.inner);
        if let Err(error) = inner.start_send(msg) {
            cx.waker().wake_by_ref();
            return std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                error,
            )));
        }

        std::pin::pin!(this).poll_flush(cx).map_ok(|()| buf.len())
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let inner = unsafe { self.map_unchecked_mut(|ws| &mut ws.inner) };

        inner
            .poll_flush(cx)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let inner = unsafe { self.map_unchecked_mut(|ws| &mut ws.inner) };

        inner
            .poll_close(cx)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    }
}
