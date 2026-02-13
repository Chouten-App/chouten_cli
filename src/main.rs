use std::env;
use std::sync::Arc;

use local_ip_address::local_ip;

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::Mutex;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use futures_util::stream::SplitSink;
use tokio_tungstenite::WebSocketStream;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;

mod watcher;
mod builder;
mod protocol;

use protocol::ModuleUpdate;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 || args[1] != "serve" {
        eprintln!("Usage: chouten-cli serve /path/to/module");
        std::process::exit(1);
    }

    let module_path = std::path::PathBuf::from(&args[2]);
    println!("🔧 Serving module at {}", module_path.display());

    let listener = TcpListener::bind("0.0.0.0:9001").await?;
    let ip = local_ip().expect("Failed to get local IP");
    println!("🚀 Dev server listening on ws://{ip}:9001/dev");

    let connected_socket: Arc<Mutex<Option<SplitSink<WebSocketStream<TcpStream>, Message>>>> =
        Arc::new(Mutex::new(None));

    let socket_clone = connected_socket.clone();

    let watcher_task = tokio::spawn(async move {
        let (_watcher, mut changes) = watcher::watch(&module_path).expect("Failed to watch module");

        while let Some(_) = changes.recv().await {
            println!("📁 Change detected → building…");

            match builder::build_module(&module_path).await {
                Ok(wasm_path) => {
                    println!("🧱 Build success");
                    let wasm_bytes = tokio::fs::read(&wasm_path).await.unwrap();

                    // Prepare metadata
                    let meta = ModuleUpdate {
                        r#type: "module_update",
                        module_id: &module_path
                            .file_name()
                            .unwrap()
                            .to_string_lossy()
                            .to_string(),
                        api_version: 3,
                    };

                    if let Some(ws) = &mut *socket_clone.lock().await {
                        let _ = ws.send(serde_json::to_string(&meta).unwrap().into()).await;
                        let _ = ws.send(tokio_tungstenite::tungstenite::Message::Binary(wasm_bytes)).await;
                        println!("🚀 Module pushed to app");
                    } else {
                        println!("⚠️ No app connected; skipping push");
                    }
                }
                Err(err) => {
                    println!("❌ Build failed: {}", err);
                }
            }
        }
    });

    while let Ok((stream, _)) = listener.accept().await {
        let ws_stream = accept_async(stream).await?;
        println!("✅ App connected");

        // Split into read and write halves
        let (mut write, mut read) = ws_stream.split();

        // Store the write half for sending
        *connected_socket.lock().await = Some(write);

        // Spawn a task to handle incoming messages
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(txt)) => {
                        println!("📩 Received text: {}", txt);
                    }
                    Ok(Message::Binary(bin)) => {
                        println!("📦 Received binary: {} bytes", bin.len());
                    }
                    Ok(Message::Close(_)) => {
                        println!("❌ Client disconnected");
                        break;
                    }
                    _ => {}
                }
            }
        });
    }
    watcher_task.await?;
    Ok(())
}

