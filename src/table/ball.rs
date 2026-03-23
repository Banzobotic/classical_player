use glam::DVec2;

#[derive(Clone, Copy)]
pub struct Ball {
    pub position: DVec2,
    pub velocity: DVec2,
}

impl Ball {
    pub const fn new() -> Self {
        Self { position: DVec2::NAN, velocity: DVec2::ZERO }
    }

    pub fn on_table(&self) -> bool {
        !self.position.is_nan()
    }
}
