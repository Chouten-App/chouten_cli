use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use tokio::net::TcpStream;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{
    connect_async,
    tungstenite::Message,
    MaybeTlsStream,
    WebSocketStream,
};

type WsWrite = futures_util::stream::SplitSink<
    WebSocketStream<MaybeTlsStream<TcpStream>>,
    Message,
>;

pub struct DevSocket {
    write: WsWrite,
}

impl DevSocket {
    /// Connects to the app's dev WebSocket, retrying until available.
    pub async fn connect_loop(url: &str) -> Result<Self> {
        loop {
            match connect_async(url).await {
                Ok((ws_stream, _)) => {
                    println!("🔌 Connected to Chouten dev socket");
                    let (write, mut read) = ws_stream.split();

                    // Spawn read task to avoid backpressure & detect disconnects
                    tokio::spawn(async move {
                        while let Some(msg) = read.next().await {
                            match msg {
                                Ok(Message::Text(t)) => {
                                    println!("📩 App: {t}");
                                }
                                Ok(Message::Close(_)) => {
                                    println!("⚠️ Dev socket closed by app");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    });

                    return Ok(Self { write });
                }
                Err(e) => {
                    println!("⏳ Waiting for app dev socket... ({e})");
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    /// Send a JSON control message
    pub async fn send_json<T: Serialize>(&mut self, value: &T) -> Result<()> {
        let text = serde_json::to_string(value)?;
        self.write.send(Message::Text(text.into())).await?;
        Ok(())
    }

    /// Send compiled WASM bytes
    pub async fn send_wasm(&mut self, bytes: Vec<u8>) -> Result<()> {
        self.write.send(Message::Binary(bytes.into())).await?;
        Ok(())
    }

    /// Notify the app that build failed
    pub async fn send_build_error(&mut self, error: &str) -> Result<()> {
        let msg = serde_json::json!({
            "type": "build_error",
            "message": error
        });
        self.write.send(Message::Text(msg.to_string().into())).await?;
        Ok(())
    }
}

