use tokio::sync::watch;

pub struct Shutdown {
    rx: watch::Receiver<bool>,
}

impl Shutdown {
    pub fn install() -> Self {
        let (tx, rx) = watch::channel(false);
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = tx.send(true);
        });
        Self { rx }
    }

    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.rx.clone()
    }
}
