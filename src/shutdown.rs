use tokio::sync::watch;

pub struct Shutdown {
    rx: watch::Receiver<bool>,
}

impl Shutdown {
    pub fn install() -> Self {
        let (tx, rx) = watch::channel(false);
        tokio::spawn(async move {
            stop_signal().await;
            let _ = tx.send(true);
        });
        Self { rx }
    }

    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.rx.clone()
    }
}

#[cfg(unix)]
async fn stop_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    match signal(SignalKind::terminate()) {
        Ok(mut term) => {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => tracing::info!("SIGTERM received, shutting down"),
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "SIGTERM handler not installed, only Ctrl+C stops the service");
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}

#[cfg(not(unix))]
async fn stop_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
