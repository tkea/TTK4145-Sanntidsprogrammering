#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

use std::time::{Duration, Instant};

pub struct Timer {
    start_time: Instant,
    duration:   Duration,
}

impl Timer {

    pub fn new(timer_duration: u64) -> Self {
        let start_time = Instant::now();
        let duration = Duration::new(timer_duration, 0);

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
