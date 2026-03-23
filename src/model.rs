use std::{
    thread,
    time::{Duration, Instant},
};

use ordered_float::OrderedFloat;
use rand::random_bool;

use crate::table::{
    GOAL, Table,
    players::{PlayerKind, Players},
};

#[derive(Clone, Copy)]
enum Action {
    Block { moved_in_range: Option<Instant> },
    Avoid,
    Pass,
    Centre,
    Shoot,
}

pub struct Model {
    table: Table,
    action: Action,
    redis_client: redis::Client,
}

impl Model {
    pub fn new(redis_client: redis::Client) -> Self {
        Self {
            table: Table::new(),
            action: Action::Block {
                moved_in_range: None,
            },
            redis_client,
        }
    }

    fn select_offensive_action(&self, closest_players: &dyn Players) -> Action {
        if closest_players.kind() == PlayerKind::Striker {
            Action::Shoot
        } else {
            if random_bool(0.5) {
                Action::Shoot
            } else {
                Action::Pass
            }
        }
    }

    fn update_action(&mut self) {
        if !self.table.ball.on_table() {
            self.action = Action::Block {
                moved_in_range: None,
            };
            return;
        }

        self.action = match self.action {
            Action::Block { moved_in_range } => {
                let closest_players = self.table.players_closest_to_ball();
                let distance_to_ball =
                    (closest_players.line_position() - self.table.ball.position.x).abs();
                if closest_players.is_friendly() && distance_to_ball < 3.0 {
                    if self.table.ball.velocity.length() < 2.0 {
                        self.select_offensive_action(closest_players)
                    } else if moved_in_range.is_some_and(|t| t.elapsed().as_secs_f64() >= 5.0) {
                        self.select_offensive_action(closest_players)
                    } else {
                        Action::Block {
                            moved_in_range: moved_in_range.or_else(|| Some(Instant::now())),
                        }
                    }
                } else {
                    Action::Block {
                        moved_in_range: None,
                    }
                }
            }
            Action::Avoid => {
                if self.table.ball.position.x > self.table.strikers.line_position() {
                    Action::Block {
                        moved_in_range: None,
                    }
                } else if self.table.ball.velocity.x < 0.5 {
                    Action::Block {
                        moved_in_range: None,
                    }
                } else {
                    Action::Avoid
                }
            }
            Action::Shoot => {
                if self.table.players_closest_to_ball().is_friendly() {
                    Action::Shoot
                } else {
                    Action::Block {
                        moved_in_range: None,
                    }
                }
            }
            Action::Pass => {
                let closest_players = self.table.players_closest_to_ball();
                if closest_players.is_friendly() && closest_players.kind() != PlayerKind::Striker {
                    Action::Pass
                } else {
                    Action::Block {
                        moved_in_range: None,
                    }
                }
            }
            Action::Centre => {
                let closest_players = self.table.players_closest_to_ball();
                if closest_players.is_friendly() {
                    if GOAL.contains(&self.table.ball.position.y)
                        && self.table.ball.velocity.x < 0.5
                    {
                        self.select_offensive_action(closest_players)
                    } else {
                        Action::Centre
                    }
                } else {
                    Action::Block {
                        moved_in_range: None,
                    }
                }
            }
        }
    }

    pub fn do_action(&mut self) {
        if !self.table.ball.on_table() {
            return;
        }
        let mut redis_con = self.redis_client.get_connection().unwrap();

        match self.action {
            Action::Block { .. } => {
                let ball = self.table.ball;
                let controlled_players = if ball.velocity.x < 10.0 {
                    &mut self.table.gk
                } else if self.table.ball.velocity.length() < 2.0 {
                    self.table
                        .friendly_players_in_range(..)
                        .min_by_key(|players| {
                            OrderedFloat((players.line_position() - ball.position.x).abs())
                        })
                        .unwrap()
                } else {
                    self.table
                        .friendly_players_in_range(..)
                        .filter(|players| {
                            if ball.velocity.x > 0.0 {
                                players.line_position() > ball.position.x
                            } else {
                                players.line_position() < ball.position.x
                            }
                        })
                        .min_by_key(|players| {
                            OrderedFloat((players.line_position() - ball.position.x).abs())
                        })
                        .unwrap()
                };

                let ball_to_players_distance = controlled_players.line_position() - ball.position.x;
                if !(ball_to_players_distance.abs() < 3.0 && ball.velocity.length() < 2.0) {
                    controlled_players.set_angle(20.0f64.copysign(ball.velocity.x), 1.0, &mut redis_con);
                }

                if ball.velocity.length() > 2.0 {
                    controlled_players.move_to_block(ball, &mut redis_con);
                } else {
                    controlled_players.move_to_align(ball.position.y, &mut redis_con);
                }

                if (2.0..=4.0).contains(&ball_to_players_distance) && ball.velocity.x > 0.5 {
                    controlled_players.set_angle(60.0, 1.0, &mut redis_con);
                    self.action = if controlled_players.kind() == PlayerKind::Striker {
                        Action::Block {
                            moved_in_range: None,
                        }
                    } else {
                        Action::Avoid
                    }
                }

                if controlled_players.kind() != PlayerKind::GoalKeeper {
                    self.table.gk.move_to(0.5, &mut redis_con);
                    if self.table.gk.target_angle().abs() < 30.0 {
                        self.table.gk.set_angle(0.0, 1.0, &mut redis_con);
                    }
                }
            }
            Action::Avoid => {
                let ball = self.table.ball;
                let controlled_players = self
                    .table
                    .friendly_players_in_range(ball.position.x..)
                    .next()
                    .unwrap();
                controlled_players.set_angle(45.0, 1.0, &mut redis_con);
            }
            Action::Pass => {
                let ball = self.table.ball;
                let friendly_players = self.table.friendly_players();
                let min_pos = friendly_players
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, players)| {
                        OrderedFloat((players.line_position() - ball.position.x).abs())
                    })
                    .unwrap()
                    .0;
                if min_pos == 3 {
                    self.action = Action::Shoot;
                    return;
                }

                let remaining_angle = friendly_players[min_pos].set_angle_avoiding_ball(-50.0, ball, &mut redis_con);
                friendly_players[min_pos + 1].set_angle(20.0, 1.0, &mut redis_con);
                let d2 = friendly_players[min_pos + 1].move_to_align(ball.position.y, &mut redis_con);
                if remaining_angle > 5.0 {
                    return;
                }

                let d1 = friendly_players[min_pos].move_to_align(ball.position.y, &mut redis_con);

                if d1.abs() < 0.5 && d2.abs() < 1.0 {
                    friendly_players[min_pos].set_angle(45.0, 0.5, &mut redis_con);
                    self.action = Action::Block { moved_in_range: None };
                }
            }
            Action::Centre => todo!(),
            Action::Shoot => {
                let ball = self.table.ball;
                let controlled_players = self.table.friendly_players_closest_to_ball();
                let remaining_angle = controlled_players.set_angle_avoiding_ball(-50.0, ball, &mut redis_con);
                if remaining_angle > 5.0 {
                    return;
                }
                let remaining_distance = controlled_players.move_to_kick_goal(ball, &mut redis_con);
                if remaining_distance.abs() > 0.5 {
                    return;
                }
                controlled_players.set_angle(45.0, 1.0, &mut redis_con);
                self.action = if controlled_players.kind() == PlayerKind::Striker {
                    Action::Block {
                        moved_in_range: None,
                    }
                } else {
                    Action::Avoid
                }
            }
        }
    }

    pub fn run(&mut self) {
        loop {
            let start = Instant::now();

            // TODO: get up to date information from server
            self.table.update();
            self.update_action();
            self.do_action();

            thread::sleep(Duration::from_millis(50) - start.elapsed())
        }
    }
}
