#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

use std::time::{Duration, Instant};

const TIMER_DURATION: u64 = 1;

pub struct Timer {
    start_time: Instant,
    duration:   Duration,
}

impl Timer {

    pub fn new() -> Self {
        let start_time = Instant::now();
        let duration = Duration::new(TIMER_DURATION, 0);

        let timer = Timer {
            start_time: start_time,
            duration:   duration,
        };

        return timer;
    }


    pub fn start(&mut self) {
        self.start_time = Instant::now();
    }


    pub fn timeout(&self) -> bool {
        return Instant::now() > self.start_time + self.duration;
    }
}
