use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const DROP_LOG_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Default)]
pub(crate) struct OffsetPrefix {
    next: Option<i64>,
    completed_above: BTreeSet<i64>,
    dropped: u64,
    last_drop_log: Option<Instant>,
}

impl OffsetPrefix {
    pub(crate) fn observe(&mut self, offset: i64) {
        match self.next {
            None => self.next = Some(offset),
            Some(n) if offset < n => self.next = Some(offset),
            _ => {}
        }
    }

    pub(crate) fn complete(&mut self, offset: i64) -> Option<i64> {
        let next = self.next.get_or_insert(offset);
        if offset < *next {
            self.dropped += 1;
            let now = Instant::now();
            if self
                .last_drop_log
                .is_none_or(|t| now.duration_since(t) >= DROP_LOG_INTERVAL)
            {
                self.last_drop_log = Some(now);
                tracing::info!(
                    target: "metrics::offset",
                    dropped = self.dropped,
                    offset,
                    next = *next,
                    pending = self.completed_above.len(),
                    "below-watermark completions dropped (duplicate delivery / rebalance)"
                );
            }
            return None;
        }
        self.completed_above.insert(offset);
        let mut advanced = false;
        while self.completed_above.remove(next) {
            *next += 1;
            advanced = true;
        }
        if advanced {
            tracing::debug!(
                target: "kafka::offset",
                offset,
                next = *next,
                pending = self.completed_above.len(),
                "complete: advanced"
            );
            Some(*next)
        } else {
            tracing::debug!(
                target: "kafka::offset",
                offset,
                next = *next,
                pending = self.completed_above.len(),
                "complete: held (waiting for earlier offset)"
            );
            None
        }
    }
}

pub(crate) type Tracker = Arc<Mutex<HashMap<(String, i32), OffsetPrefix>>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_order_completion_advances_each_step() {
        let mut p = OffsetPrefix::default();
        for o in 0..3 {
            p.observe(o);
        }
        assert_eq!(p.complete(0), Some(1));
        assert_eq!(p.complete(1), Some(2));
        assert_eq!(p.complete(2), Some(3));
    }

    #[test]
    fn out_of_order_holds_until_prefix_filled() {
        let mut p = OffsetPrefix::default();
        for o in 0..3 {
            p.observe(o);
        }
        assert_eq!(p.complete(2), None, "gap at 0,1 > held");
        assert_eq!(p.complete(1), None, "gap at 0 > held");
        assert_eq!(p.complete(0), Some(3), "0..=2 contiguous > advance to 3");
    }

    #[test]
    fn watermark_starts_at_lowest_observed() {
        let mut p = OffsetPrefix::default();
        p.observe(100);
        p.observe(98);
        p.observe(99);
        assert_eq!(p.complete(98), Some(99));
        assert_eq!(p.complete(99), Some(100));
        assert_eq!(p.complete(100), Some(101));
    }

    #[test]
    fn offset_below_watermark_is_dropped() {
        let mut p = OffsetPrefix::default();
        p.observe(5);
        assert_eq!(p.complete(5), Some(6));
        assert_eq!(p.complete(4), None);
    }

    #[test]
    fn complete_without_observe_seeds_watermark() {
        let mut p = OffsetPrefix::default();
        assert_eq!(p.complete(7), Some(8));
    }
}
