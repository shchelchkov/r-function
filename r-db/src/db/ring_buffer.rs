use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct RingBuffer<T> {
    data: Box<[MaybeUninit<T>]>,
    write_index: AtomicU64,
    capacity: u64,
}

impl<T> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        assert!(capacity.is_power_of_two());

        let data = (0..capacity)
            .map(|_| MaybeUninit::uninit())
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self {
            data,
            write_index: AtomicU64::new(0),
            capacity: capacity as u64,
        }
    }

    pub fn push(&mut self, value: T) {
        let index = self.write_index.fetch_add(1, Ordering::Relaxed) % self.capacity;

        let slot = &mut self.data[index as usize];

        if self.write_index.load(Ordering::Relaxed) > self.capacity {
            unsafe {
                slot.assume_init_drop();
            }
        }

        slot.write(value);
    }
}
