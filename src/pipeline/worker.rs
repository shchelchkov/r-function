use std::sync::Arc;

use r_consumer::kafka::consumer::Work;
use r_consumer::process::process::Processor;
use tokio::sync::mpsc;

pub(super) async fn run_worker(
    mut rx: mpsc::Receiver<Work>,
    process: Arc<Processor>,
    chunk_max: usize,
) {
    let mut buf: Vec<Work> = Vec::with_capacity(chunk_max);
    loop {
        let n = rx.recv_many(&mut buf, chunk_max).await;
        if n == 0 {
            break;
        }
        let mut msgs = Vec::with_capacity(n);
        let mut acks = Vec::with_capacity(n);
        for Work { msg, ack } in buf.drain(..) {
            msgs.push(msg);
            acks.push(ack);
        }
        let results = process.handle_batch(msgs).await;
        for (ack, res) in acks.into_iter().zip(results) {
            let _ = ack.send(res);
        }
    }
}
