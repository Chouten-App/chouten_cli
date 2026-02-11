use std::env;
use std::sync::Arc;

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::Mutex;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;

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

    let listener = TcpListener::bind("127.0.0.1:9001").await?;
    println!("🚀 Dev server listening on ws://127.0.0.1:9001/dev");

    let connected_socket: Arc<Mutex<Option<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>>>> =
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

        // Store the socket
        *connected_socket.lock().await = Some(ws_stream);

        // TODO: Add support for receiving messages, like Logs
        // let ws_read = connected_socket.clone();
        // tokio::spawn(async move { /* handle incoming frames */ });
    }

    watcher_task.await?;
    Ok(())
}

