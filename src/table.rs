use std::{ops::RangeBounds, thread, time::Duration};

use glam::{DMat2, DVec2};
use ordered_float::OrderedFloat;
use redis::TypedCommands;

use crate::table::{
    ball::Ball,
    players::{FriendlyPlayers, OpposingPlayers, PlayerKind, Players},
};

pub mod ball;
pub mod players;

const BALL_DIAMETER: f64 = 2.5;
const BALL_RADIUS: f64 = BALL_DIAMETER / 2.0;

pub const TABLE_WIDTH: f64 = 48.5;
pub const TABLE_LENGTH: f64 = 81.0;

pub const GOAL: std::ops::RangeInclusive<f64> =
    (TABLE_WIDTH / 2.0 - 4.0)..=(TABLE_WIDTH / 2.0 + 4.0);

const PLAYER_FOOT_LENGTH: f64 = 1.25;
const PLAYER_FOOT_WIDTH: f64 = 1.4;

pub struct Table {
    pub gk: FriendlyPlayers,
    pub defence: FriendlyPlayers,
    pub midfield: FriendlyPlayers,
    pub strikers: FriendlyPlayers,
    pub opp_gk: OpposingPlayers,
    pub opp_defence: OpposingPlayers,
    pub opp_midfield: OpposingPlayers,
    pub opp_strikers: OpposingPlayers,
    pub ball: Ball,
    last_update: u128,
}

impl Table {
    pub fn new() -> Self {
        let gk = FriendlyPlayers::new(PlayerKind::GoalKeeper, 7.25);
        let defence = FriendlyPlayers::new(PlayerKind::Defender, 7.25 + 9.5);
        let opp_strikers = OpposingPlayers::new(PlayerKind::Striker, 7.25 + 2.0 * 9.5);
        let midfield = FriendlyPlayers::new(PlayerKind::MidFielder, 7.25 + 3.0 * 9.5);
        let opp_midfield = OpposingPlayers::new(PlayerKind::MidFielder, 7.25 + 4.0 * 9.5);
        let strikers = FriendlyPlayers::new(PlayerKind::Striker, 7.25 + 5.0 * 9.5);
        let opp_defence = OpposingPlayers::new(PlayerKind::Defender, 7.25 + 6.0 * 9.5);
        let opp_gk = OpposingPlayers::new(PlayerKind::GoalKeeper, 7.25 + 7.0 * 9.5);
        let ball = Ball::new();

        Self {
            gk,
            defence,
            midfield,
            strikers,
            opp_gk,
            opp_defence,
            opp_midfield,
            opp_strikers,
            ball,
        }
    }

    pub fn update(&mut self, redis_con: &mut redis::Connection) {
        let previous_last_update = loop {
            let update_timestamp = redis_con.get("last_update").unwrap().unwrap().parse().unwrap();
            if update_timestamp != self.last_update {
                let previous_last_update = self.last_update;
                self.last_update = update_timestamp;
                break previous_last_update;
            }
            thread::sleep(Duration::from_millis(5));
        };

        self.gk.update(
            redis_con
                .get("gk_position")
                .unwrap()
                .unwrap()
                .parse()
                .unwrap(),
            redis_con.get("gk_angle").unwrap().unwrap().parse().unwrap(),
        );
        self.defence.update(
            redis_con
                .get("defence_position")
                .unwrap()
                .unwrap()
                .parse()
                .unwrap(),
            redis_con
                .get("defence_angle")
                .unwrap()
                .unwrap()
                .parse()
                .unwrap(),
        );
        self.midfield.update(
            redis_con
                .get("midfield_position")
                .unwrap()
                .unwrap()
                .parse()
                .unwrap(),
            redis_con
                .get("midfield_angle")
                .unwrap()
                .unwrap()
                .parse()
                .unwrap(),
        );
        self.strikers.update(
            redis_con
                .get("striker_position")
                .unwrap()
                .unwrap()
                .parse()
                .unwrap(),
            redis_con
                .get("striker_angle")
                .unwrap()
                .unwrap()
                .parse()
                .unwrap(),
        );
        self.opp_gk.update(
            redis_con
                .get("opp_gk_position")
                .unwrap()
                .unwrap()
                .parse()
                .unwrap(),
            redis_con
                .get("opp_gk_angle")
                .unwrap()
                .unwrap()
                .parse()
                .unwrap(),
        );
        self.opp_defence.update(
            redis_con
                .get("opp_defence_position")
                .unwrap()
                .unwrap()
                .parse()
                .unwrap(),
            redis_con
                .get("opp_defence_angle")
                .unwrap()
                .unwrap()
                .parse()
                .unwrap(),
        );
        self.opp_midfield.update(
            redis_con
                .get("opp_midfield_position")
                .unwrap()
                .unwrap()
                .parse()
                .unwrap(),
            redis_con
                .get("opp_midfield_angle")
                .unwrap()
                .unwrap()
                .parse()
                .unwrap(),
        );
        self.opp_strikers.update(
            redis_con
                .get("opp_striker_position")
                .unwrap()
                .unwrap()
                .parse()
                .unwrap(),
            redis_con
                .get("opp_striker_angle")
                .unwrap()
                .unwrap()
                .parse()
                .unwrap(),
        );

        self.ball.update(
            redis_con.get("ball_x").unwrap().unwrap().parse().unwrap(),
            redis_con.get("ball_y").unwrap().unwrap().parse().unwrap(),
            self.last_update - previous_last_update,
        );
    }

    fn all_players<'a>(&'a self) -> [&'a dyn Players; 8] {
        [
            &self.gk,
            &self.defence,
            &self.midfield,
            &self.strikers,
            &self.opp_gk,
            &self.opp_defence,
            &self.opp_midfield,
            &self.opp_strikers,
        ]
    }

    pub fn friendly_players<'a>(&'a mut self) -> [&'a mut FriendlyPlayers; 4] {
        [
            &mut self.gk,
            &mut self.defence,
            &mut self.midfield,
            &mut self.strikers,
        ]
    }

    pub fn players_in_range<'a>(
        &'a self,
        range: impl RangeBounds<f64>,
    ) -> impl Iterator<Item = &'a dyn Players> {
        self.all_players()
            .into_iter()
            .filter(move |players| range.contains(&players.line_position()))
    }

    pub fn friendly_players_in_range<'a>(
        &'a mut self,
        range: impl RangeBounds<f64>,
    ) -> impl Iterator<Item = &'a mut FriendlyPlayers> {
        self.friendly_players()
            .into_iter()
            .filter(move |players| range.contains(&players.line_position()))
    }

    pub fn players_closest_to_ball<'a>(&'a self) -> &'a dyn Players {
        self.all_players()
            .into_iter()
            .min_by_key(|players| {
                OrderedFloat((players.line_position() - self.ball.position.x).abs())
            })
            .unwrap()
    }

    pub fn friendly_players_closest_to_ball<'a>(&'a mut self) -> &'a mut FriendlyPlayers {
        let ball = self.ball;
        self.friendly_players()
            .into_iter()
            .min_by_key(|players| OrderedFloat((players.line_position() - ball.position.x).abs()))
            .unwrap()
    }
}

struct Line {
    from: DVec2,
    to: DVec2,
}

impl Line {
    fn new(from: DVec2, to: DVec2) -> Self {
        Line { from, to }
    }

    fn intersection(self, other: Line) -> Option<DVec2> {
        let x1x2 = DVec2::new(self.from.x, self.to.x);
        let x3x4 = DVec2::new(other.from.x, other.to.x);
        let y1y2 = DVec2::new(self.from.y, self.to.y);
        let y3y4 = DVec2::new(other.from.y, other.to.y);
        let unit = DVec2::ONE;

        let divisor = DMat2::from_cols_array(&[
            DMat2::from_cols(x1x2, unit).determinant(),
            DMat2::from_cols(x3x4, unit).determinant(),
            DMat2::from_cols(y1y2, unit).determinant(),
            DMat2::from_cols(y3y4, unit).determinant(),
        ])
        .determinant();

        if divisor == 0.0 {
            return None;
        }

        let intersection_x = DMat2::from_cols_array(&[
            DMat2::from_cols(x1x2, y1y2).determinant(),
            DMat2::from_cols(x3x4, y3y4).determinant(),
            DMat2::from_cols(x1x2, unit).determinant(),
            DMat2::from_cols(x3x4, unit).determinant(),
        ])
        .determinant()
            / divisor;

        let intersection_y = DMat2::from_cols_array(&[
            DMat2::from_cols(x1x2, y1y2).determinant(),
            DMat2::from_cols(x3x4, y3y4).determinant(),
            DMat2::from_cols(y1y2, unit).determinant(),
            DMat2::from_cols(y3y4, unit).determinant(),
        ])
        .determinant()
            / divisor;

        if !(0.0..=TABLE_WIDTH).contains(&intersection_y) {
            return None;
        }

        Some(DVec2::new(intersection_x, intersection_y))
    }
}
