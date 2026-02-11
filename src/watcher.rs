use notify::{RecommendedWatcher, RecursiveMode, Event, EventKind, Result as NotifyResult, Config};
use notify::Watcher;
use tokio::sync::mpsc;
use std::path::Path;
use std::time::Duration;

/// Watch the given path and return a `tokio::mpsc::Receiver<()>`
/// that signals when a rebuild should happen
pub fn watch(path: &Path) -> NotifyResult<(RecommendedWatcher, mpsc::Receiver<()>)> {
    let (tx, rx) = mpsc::channel(1);
    let (debounce_tx, mut debounce_rx) = mpsc::channel::<()>(1);

    let mut watcher: RecommendedWatcher = RecommendedWatcher::new(
        move |res: NotifyResult<Event>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        if !event.paths.iter().any(|p| p.components().any(|c| c.as_os_str() == "target")) {
                            let _ = debounce_tx.try_send(());
                        }
                    }
                    _ => {}
                }
            }
        },
        Config::default(),
    )?;

    watcher.watch(path, RecursiveMode::Recursive)?;

    tokio::spawn(async move {
        let mut last_trigger = tokio::time::Instant::now() - Duration::from_secs(1);
        while let Some(_) = debounce_rx.recv().await {
            let now = tokio::time::Instant::now();
            if now.duration_since(last_trigger) >= Duration::from_millis(500) {
                last_trigger = now;
                let _ = tx.send(()).await;
            }
        }
    });

    Ok((watcher, rx)) // return watcher so it lives
}


