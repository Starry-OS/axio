use core::{
    mem::MaybeUninit,
    task::{Context, Waker},
};

use bitflags::bitflags;
use spin::Mutex;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct IoEvents: u16 {
        const IN     = 0x0001;
        const PRI    = 0x0002;
        const OUT    = 0x0004;
        const ERR    = 0x0008;
        const HUP    = 0x0010;
        const NVAL   = 0x0020;

        const RDNORM = 0x0040;
        const RDBAND = 0x0080;
        const WRNORM = 0x0100;
        const WRBAND = 0x0200;

        const MSG    = 0x0400;
        const REMOVE = 0x1000;
        const RDHUP  = 0x2000;

        /// Events that are always polled even without specifying them.
        const ALWAYS_POLL = Self::ERR.bits() | Self::HUP.bits();
    }
}

/// Trait for types that can be polled for I/O events.
pub trait Pollable {
    /// Polls the type for I/O events.
    fn poll(&self) -> IoEvents;

    /// Registers wakers for I/O events.
    fn register(&self, context: &mut Context<'_>, events: IoEvents);
}

const POLL_SET_CAPACITY: usize = 64;

struct Inner {
    entries: [MaybeUninit<Waker>; POLL_SET_CAPACITY],
    cursor: usize,
}

impl Inner {
    const fn new() -> Self {
        Self {
            entries: unsafe { MaybeUninit::uninit().assume_init() },
            cursor: 0,
        }
    }

    fn len(&self) -> usize {
        self.cursor.min(POLL_SET_CAPACITY)
    }

    fn is_empty(&self) -> bool {
        self.cursor == 0
    }

    fn register(&mut self, waker: &Waker) {
        let slot = self.cursor % POLL_SET_CAPACITY;
        if self.cursor >= POLL_SET_CAPACITY {
            let old = unsafe { self.entries[slot].assume_init_read() };
            if !old.will_wake(waker) {
                old.wake();
            }
        }
        self.entries[slot].write(waker.clone());
        self.cursor = (self.cursor + 1) % (POLL_SET_CAPACITY * 2);
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let len = self.cursor.min(POLL_SET_CAPACITY);
        for i in 0..len {
            unsafe { self.entries[i].assume_init_read() }.wake();
        }
    }
}

/// A data structure for waking up tasks that are waiting for I/O events.
pub struct PollSet(Mutex<Inner>);

impl Default for PollSet {
    fn default() -> Self {
        Self::new()
    }
}

impl PollSet {
    pub const fn new() -> Self {
        Self(Mutex::new(Inner::new()))
    }

    pub fn register(&self, waker: &Waker) {
        self.0.lock().register(waker);
    }

    pub fn wake(&self) -> usize {
        let mut guard = self.0.lock();
        if guard.is_empty() {
            return 0;
        }
        let inner = core::mem::replace(&mut *guard, Inner::new());
        drop(guard);
        inner.len()
    }
}

impl Drop for PollSet {
    fn drop(&mut self) {
        // Ensure all entries are dropped
        self.wake();
    }
}

#[cfg(feature = "alloc")]
impl alloc::task::Wake for PollSet {
    fn wake(self: alloc::sync::Arc<Self>) {
        self.as_ref().wake();
    }

    fn wake_by_ref(self: &alloc::sync::Arc<Self>) {
        self.as_ref().wake();
    }
}
