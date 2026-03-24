use std::time::Duration;

use glam::DVec2;

use crate::table::{TABLE_LENGTH, TABLE_WIDTH};

#[derive(Clone, Copy)]
pub struct Ball {
    pub position: DVec2,
    pub velocity: DVec2,
}

impl Ball {
    pub const fn new() -> Self {
        Self { position: DVec2::NAN, velocity: DVec2::ZERO }
    }

    pub fn update(&mut self, x: f64, y: f64, time_delta: u128) {
        let old_position = self.position;
        let new_position = DVec2::new(x * TABLE_LENGTH, y * TABLE_WIDTH);

        let change = new_position - old_position;
        let delta_s = Duration::from_nanos_u128(time_delta).as_secs_f64();
        self.position = new_position;
        self.velocity = change / delta_s;
    }

    pub fn on_table(&self) -> bool {
        !self.position.is_nan()
    }
}
