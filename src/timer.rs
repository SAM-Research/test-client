use std::time::Duration;

pub struct Timer {
    tick_duration: Duration,
    end_tick: u32,
    counter: u32,
}

impl Timer {
    pub fn new(tick_duration: Duration, end_tick: u32) -> Self {
        Self {
            tick_duration,
            end_tick,
            counter: 0,
        }
    }

    pub async fn next(&mut self) -> bool {
        self.counter += 1;
        tokio::time::sleep(self.tick_duration).await;
        self.counter != self.end_tick
    }

    pub fn do_action(&self, rate: u32) -> bool {
        self.counter % rate == 0
    }

    pub fn current_tick(&self) -> u32 {
        self.counter
    }
}
