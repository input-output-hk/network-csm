use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::Mutex as AsyncMutex;

pub fn mempipe() -> (MemPipeHandle, MemPipeHandle) {
    let (w1, r1) = unipipe_new();
    let (w2, r2) = unipipe_new();
    (
        MemPipeHandle {
            read: r1,
            write: w2,
        },
        MemPipeHandle {
            read: r2,
            write: w1,
        },
    )
}

#[derive(Clone)]
pub struct MemPipeHandle {
    read: MemPipeReader,
    write: MemPipeWriter,
}

impl AsyncRead for MemPipeHandle {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        unsafe { self.map_unchecked_mut(|s| &mut s.read).poll_read(cx, buf) }
    }
}

impl AsyncWrite for MemPipeHandle {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        unsafe { self.map_unchecked_mut(|s| &mut s.write).poll_write(cx, buf) }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// Unidirectional pipe where you can read what has been written to it
#[derive(Clone)]
pub struct MemPipeReader {
    buffer: Arc<AsyncMutex<MemPipeData>>,
}

#[derive(Clone)]
pub struct MemPipeWriter {
    buffer: Arc<AsyncMutex<MemPipeData>>,
}

struct MemPipeData {
    data: VecDeque<u8>,
}

impl MemPipeData {
    fn new() -> Self {
        Self {
            data: VecDeque::new(),
        }
    }
}

pub fn unipipe_new() -> (MemPipeWriter, MemPipeReader) {
    let buffer = Arc::new(AsyncMutex::new(MemPipeData::new()));
    let writer = MemPipeWriter {
        buffer: buffer.clone(),
    };
    let reader = MemPipeReader {
        buffer: buffer.clone(),
    };
    (writer, reader)
}

impl AsyncRead for MemPipeReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let mut buffer = match self.buffer.try_lock() {
            Ok(lock) => lock,
            Err(_) => {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        };

        if buffer.data.is_empty() {
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }

        while let Some(byte) = buffer.data.pop_front() {
            if buf.remaining() == 0 {
                break;
            }
            buf.put_slice(&[byte]);
        }

        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for MemPipeWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let mut buffer = match self.buffer.try_lock() {
            Ok(lock) => lock,
            Err(_) => {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        };
        buffer.data.extend(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn test_fake_memory_handle() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (mut write_handle, mut read_handle) = unipipe_new();

    tokio::spawn(async move {
        write_handle.write_all(b"hello world").await.unwrap();
    });

    let mut buf = vec![0; 11];
    read_handle.read_exact(&mut buf).await.unwrap();

    assert_eq!(&buf, b"hello world");
}
