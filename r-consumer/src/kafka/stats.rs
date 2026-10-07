use rdkafka::{ClientContext, Statistics, consumer::ConsumerContext};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
pub(crate) struct StatsContext {
    pub(crate) rps: Arc<RpsMeter>,
}

#[derive(Default)]
pub(crate) struct RpsMeter {
    count: AtomicU64,
    window: Mutex<Window>,
}

#[derive(Default, Debug, PartialEq)]
pub(crate) struct Window {
    min: f64,
    max: f64,
    msgs: u64,
    secs: f64,
    n: u32,
}

impl Window {
    fn avg(&self) -> f64 {
        if self.secs > 0.0 {
            self.msgs as f64 / self.secs
        } else {
            0.0
        }
    }
}

impl RpsMeter {
    pub(crate) fn inc(&self) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn sample(&self, elapsed: Duration) {
        let secs = elapsed.as_secs_f64();
        if secs <= 0.0 {
            return;
        }
        let msgs = self.count.swap(0, Ordering::Relaxed);
        let rate = msgs as f64 / secs;

        let mut w = self.window.lock().unwrap();
        if w.n == 0 {
            w.min = rate;
            w.max = rate;
        } else {
            w.min = w.min.min(rate);
            w.max = w.max.max(rate);
        }
        w.msgs += msgs;
        w.secs += secs;
        w.n += 1;
    }

    pub(crate) fn take(&self) -> Window {
        std::mem::take(&mut *self.window.lock().unwrap())
    }
}

impl ClientContext for StatsContext {
    fn stats(&self, stats: Statistics) {
        for (topic, t) in &stats.topics {
            for (pid, p) in &t.partitions {
                tracing::debug!(
                    target: "kafka::lag",
                    topic, partition = pid,
                    committed = p.committed_offset,
                    stored = p.stored_offset,
                    hi = p.hi_offset,
                    lag = p.consumer_lag,
                    "partition state"
                );
            }
        }

        let max_lag: i64 = stats
            .topics
            .values()
            .flat_map(|t| t.partitions.values())
            .map(|p| p.consumer_lag)
            .filter(|l| *l >= 0)
            .max()
            .unwrap_or(0);

        let rps = self.rps.take();

        tracing::info!(
            target: "kafka::stats",
            client = %stats.name,
            client_type = %stats.client_type,
            rxmsgs = stats.rxmsgs,
            rxmsg_bytes = stats.rxmsg_bytes,
            rx_bytes = stats.rx_bytes,
            tx = stats.tx,
            rx = stats.rx,
            max_consumer_lag = max_lag,
            rps_min = rps.min,
            rps_max = rps.max,
            rps_avg = rps.avg(),
            "rdkafka stats"
        );
    }
}

impl ConsumerContext for StatsContext {}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(m: &RpsMeter, msgs: u64, elapsed: Duration) {
        for _ in 0..msgs {
            m.inc();
        }
        m.sample(elapsed);
    }

    #[test]
    fn window_min_max_avg() {
        let m = RpsMeter::default();
        feed(&m, 10, Duration::from_secs(1));
        feed(&m, 0, Duration::from_secs(1));
        feed(&m, 60, Duration::from_secs(2)); 

        let w = m.take();
        assert_eq!((w.min, w.max, w.n), (0.0, 30.0, 3));
        assert_eq!(w.avg(), 70.0 / 4.0);

        assert_eq!(m.take(), Window::default(), "take() resets the window");
        assert_eq!(Window::default().avg(), 0.0);
    }

    #[test]
    fn count_between_samples_is_not_lost() {
        let m = RpsMeter::default();
        feed(&m, 5, Duration::from_secs(1));
        let _ = m.take();
        feed(&m, 7, Duration::from_secs(1));
        let w = m.take();
        assert_eq!((w.min, w.max, w.msgs), (7.0, 7.0, 7));
    }
}
