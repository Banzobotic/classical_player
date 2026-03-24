use glam::DVec2;
use redis::TypedCommands;

use crate::table::{
    BALL_RADIUS, GOAL, Line, PLAYER_FOOT_WIDTH, TABLE_LENGTH, TABLE_WIDTH, ball::Ball,
};
use ordered_float::OrderedFloat;

#[derive(serde_derive::Serialize)]
struct SlideCommand {
    #[serde(rename = "type")]
    kind: &'static str,
    rod: u32,
    position: f64,
}

impl SlideCommand {
    pub fn new(player_kind: PlayerKind, position: f64) -> Self {
        let rod = match player_kind {
            PlayerKind::GoalKeeper => 0,
            PlayerKind::Defender => 1,
            PlayerKind::MidFielder => 2,
            PlayerKind::Striker => 3,
        };

        Self {
            kind: "slide", rod, position
        }
    }

    pub fn as_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

#[derive(serde_derive::Serialize)]
struct KickCommand {
    #[serde(rename = "type")]
    kind: &'static str,
    rod: u32,
    angle: f64,
    speed: f64,
}

impl KickCommand {
    pub fn new(player_kind: PlayerKind, angle: f64, speed: f64) -> Self {
        let rod = match player_kind {
            PlayerKind::GoalKeeper => 0,
            PlayerKind::Defender => 1,
            PlayerKind::MidFielder => 2,
            PlayerKind::Striker => 3,
        };

        Self {
            kind: "kick", rod, angle, speed,
        }
    }

    pub fn as_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PlayerKind {
    GoalKeeper,
    Defender,
    MidFielder,
    Striker,
}

pub trait Players {
    fn is_friendly(&self) -> bool;
    fn line_position(&self) -> f64;
    fn kind(&self) -> PlayerKind;
}

pub struct FriendlyPlayers {
    kind: PlayerKind,
    line_position: f64,
    position: f64,
    target_position: f64,
    angle: f64,
    target_angle: f64,
}

impl FriendlyPlayers {
    pub const fn new(kind: PlayerKind, line_position: f64) -> Self {
        Self {
            kind,
            line_position,
            position: 0.0,
            target_position: 0.0,
            angle: 0.0,
            target_angle: 0.0,
        }
    }

    pub fn update(&mut self, position: f64, angle: f64) {
        self.position = position;
        self.angle = angle;
    }

    pub const fn target_angle(&self) -> f64 {
        self.target_angle
    }

    fn player_positions_zero(&self) -> impl Iterator<Item = f64> {
        match self.kind {
            PlayerKind::GoalKeeper => [16.0].iter(),
            PlayerKind::Defender | PlayerKind::Striker => [1.2, 16.2, 31.2].iter(),
            PlayerKind::MidFielder => [1.2, 11.4, 21.6, 31.8].iter(),
        }.copied()
    }

    const fn movement_range(&self) -> f64 {
        match self.kind {
            PlayerKind::GoalKeeper => 16.0,
            PlayerKind::Defender | PlayerKind::Striker => 16.0,
            PlayerKind::MidFielder => 15.0,
        }
    }

    fn player_positions(&self) -> impl Iterator<Item = f64> {
        self.player_positions_zero()
            .map(|pos| pos + self.movement_range() * self.position)
    }

    pub fn move_to(&mut self, position: f64, redis_con: &mut redis::Connection) {
        // TODO: send command to table
        assert!((0.0..=1.0).contains(&position));
        redis_con.lpush("task_queue", SlideCommand::new(self.kind, position).as_json());
        self.target_position = position;
    }

    pub fn move_to_align(&mut self, position: f64, redis_con: &mut redis::Connection) -> f64 {
        let move_distance = self
            .player_positions()
            .zip(self.player_positions_zero())
            .filter(|&(_, zero)| (zero..=(zero + self.movement_range())).contains(&position))
            .map(|(player_position, _)| position - player_position)
            .min_by_key(|player_position| OrderedFloat(player_position.abs()));

        if let Some(move_distance) = move_distance {
            self.move_to(self.position + move_distance / self.movement_range(), redis_con);
            move_distance
        } else {
            if position < TABLE_WIDTH / 2.0 {
                let move_distance = self.position * self.movement_range();
                self.move_to(0.0, redis_con);
                move_distance
            } else {
                let move_distance = (1.0 - self.position) * self.movement_range();
                self.move_to(1.0, redis_con);
                move_distance
            }
        }
    }

    pub fn move_to_block(&mut self, ball: Ball, redis_con: &mut redis::Connection) {
        let movement_line = Line::new(ball.position, ball.position + ball.velocity);
        let players_line = Line::new(
            DVec2::new(self.line_position, 0.0),
            DVec2::new(self.line_position, TABLE_WIDTH),
        );
        let intersection_point = movement_line.intersection(players_line);

        if let Some(position) = intersection_point {
            self.move_to_align(position.y, redis_con);
        }
    }

    pub fn move_to_kick(&mut self, ball: Ball, to: DVec2, redis_con: &mut redis::Connection) -> f64 {
        let kick_line = to - ball.position;
        let straight_line = DVec2::new(TABLE_LENGTH, ball.position.y);
        if kick_line.angle_to(straight_line).abs() < 5.0 {
            return self.move_to_align(ball.position.y, redis_con);
        }
        let center_to_kick_point = (-kick_line).normalize() * BALL_RADIUS;
        let kick_point = ball.position + center_to_kick_point;

        let offset = if kick_point.y > TABLE_WIDTH / 2.0 {
            PLAYER_FOOT_WIDTH / 2.0
        } else {
            -PLAYER_FOOT_WIDTH / 2.0
        };
        self.move_to_align(kick_point.y + offset, redis_con)
    }

    pub fn move_to_kick_goal(&mut self, ball: Ball, redis_con: &mut redis::Connection) -> f64 {
        let ball_position = ball.position.y;
        if GOAL.contains(&ball_position) {
            self.move_to_align(ball_position, redis_con)
        } else {
            self.move_to_kick(ball, DVec2::new(TABLE_LENGTH, TABLE_WIDTH / 2.0), redis_con)
        }
    }

    fn foot_position(&self) -> f64 {
        self.line_position + self.angle.sin() * 4.5
    }

    pub fn set_angle(&mut self, angle: f64, speed: f64, redis_con: &mut redis::Connection) -> f64 {
        self.target_angle = angle;
        redis_con.lpush("task_queue", KickCommand::new(self.kind, angle, speed).as_json()).unwrap();
        (self.target_angle - self.angle).abs()
    }

    pub fn set_angle_avoiding_ball(&mut self, angle: f64, ball: Ball, redis_con: &mut redis::Connection) -> f64 {
        const MARGIN: f64 = 4.0;
        let closest_distance_to_ball = self
            .player_positions()
            .map(|player_position| ball.position.y - player_position)
            .min_by_key(|player_position| OrderedFloat(player_position.abs()))
            .unwrap();
        if self.foot_position() > ball.position.x && closest_distance_to_ball < MARGIN {
            let move_distance = if closest_distance_to_ball < 0.0 {
                -MARGIN - closest_distance_to_ball
            } else {
                MARGIN - closest_distance_to_ball
            };
            self.move_to(self.position - move_distance * self.movement_range(), redis_con);
            return (angle - self.angle).abs();
        }

        self.set_angle(angle, 1.0, redis_con)
    }
}

impl Players for FriendlyPlayers {
    fn is_friendly(&self) -> bool {
        true
    }

    fn line_position(&self) -> f64 {
        self.line_position
    }

    fn kind(&self) -> PlayerKind {
        self.kind
    }
}

pub struct OpposingPlayers {
    kind: PlayerKind,
    line_position: f64,
    position: f64,
    angle: f64,
}

impl OpposingPlayers {
    pub const fn new(kind: PlayerKind, line_position: f64) -> Self {
        Self {
            kind,
            line_position,
            position: 0.0,
            angle: 0.0,
        }
    }

    pub fn update(&mut self, position: f64, angle: f64) {
        self.position = position;
        self.angle = angle;
    }

    pub fn set_position(&mut self, position: f64) {
        self.position = position;
    }
}

impl Players for OpposingPlayers {
    fn is_friendly(&self) -> bool {
        true
    }

    fn line_position(&self) -> f64 {
        self.line_position
    }

    fn kind(&self) -> PlayerKind {
        self.kind
    }

}
