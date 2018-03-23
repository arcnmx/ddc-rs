use std::thread::sleep;
use std::time::{Instant, Duration};

/// A type that can help with implementing the DDC specificationed delays.
#[derive(Clone, Debug)]
pub struct Delay {
    time: Option<Instant>,
    delay: Duration,
}

impl Delay {
    /// Creates a new delay starting now.
    pub fn new(delay: Duration) -> Self {
        Delay {
            time: Some(Instant::now()),
            delay: delay,
        }
    }

    /// The time remaining in this delay.
    pub fn remaining(&self) -> Duration {
        self.time.as_ref().and_then(|time| self.delay.checked_sub(time.elapsed())).unwrap_or(Duration::default())
    }

    /// Waits out the remaining time in this delay.
    pub fn sleep(&mut self) {
        if let Some(delay) = self.time.take().and_then(|time| self.delay.checked_sub(time.elapsed())) {
            sleep(delay);
        }
    }
}

impl Default for Delay {
    fn default() -> Self {
        Delay {
            time: None,
            delay: Default::default(),
        }
    }
}
