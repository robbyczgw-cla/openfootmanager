//! Stage 1 positional match simulation (v1).
//!
//! A continuous-coordinate model: 22 players + ball have real positions in
//! `[0,1]^2` (x: 0 = Home goal, 1 = Away goal; y: 0..1 across the pitch). Each
//! tick players steer toward role-based targets relative to the ball, and the
//! ball-carrier's spatial situation (pressure, shooting range, open team-mates)
//! decides the action. Outcomes reuse the same attribute-based probabilities as
//! the zone engine, so positions DRIVE the events/results — and the existing
//! `MatchEvent` stream keeps the report/stats pipeline working unchanged.
//!
//! Per tick the sim emits a `MatchFrame` (consumed by the 2D renderer now, the
//! 3D renderer later). This module is self-contained and unit-testable; it does
//! not touch `LiveMatchState` — the integration layer drives it.
#![allow(dead_code)]

use rand::{Rng, RngExt};
use serde::{Deserialize, Serialize};

use crate::event::{EventType, MatchEvent};
use crate::shared::{PlayerSnap, TraitContext, trait_bonus};
use crate::types::{MatchConfig, PlayStyle, Position, Side, TeamData, Zone};

/// Simulation ticks per match minute. Each tick produces one frame.
pub const TICKS_PER_MINUTE: u32 = 30;

// ---------------------------------------------------------------------------
// Frame contract — mirrors src/components/match/matchFrame.ts (snake_case)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameBall {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FramePlayer {
    pub id: String,
    pub side: Side,
    pub x: f64,
    pub y: f64,
    pub has_ball: bool,
    pub number: u8,
    pub name: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchFrame {
    pub t: f64,
    pub ball: FrameBall,
    pub players: Vec<FramePlayer>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn role_str(p: Position) -> String {
    match p {
        Position::Goalkeeper => "GK",
        Position::Defender => "DF",
        Position::Midfielder => "MF",
        Position::Forward => "FW",
    }
    .to_string()
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Map a continuous ball x to the coarse `Zone` (for event tagging + snapshot).
pub fn zone_from_x(x: f64) -> Zone {
    if x < 0.18 {
        Zone::HomeBox
    } else if x < 0.40 {
        Zone::HomeDefense
    } else if x < 0.60 {
        Zone::Midfield
    } else if x < 0.82 {
        Zone::AwayDefense
    } else {
        Zone::AwayBox
    }
}

/// Formation string ("4-4-2", "4-2-3-1", …) → 11 `(x, y, role)` slots with the
/// home-attacks-+x convention (mirrored for the away side).
fn formation_positions(formation: &str, side: Side) -> Vec<(f64, f64, Position)> {
    let lines: Vec<usize> = formation
        .split('-')
        .filter_map(|s| s.trim().parse::<usize>().ok())
        .filter(|&n| n > 0)
        .collect();

    let mut slots: Vec<(f64, f64, Position)> = Vec::with_capacity(11);
    slots.push((0.05, 0.5, Position::Goalkeeper));

    let line_count = lines.len().max(1);
    for (li, &count) in lines.iter().enumerate() {
        let depth = if line_count <= 1 {
            0.32
        } else {
            lerp(0.18, 0.47, li as f64 / (line_count as f64 - 1.0))
        };
        let role = if li == 0 {
            Position::Defender
        } else if li == line_count - 1 {
            Position::Forward
        } else {
            Position::Midfielder
        };
        for j in 0..count {
            let lateral = if count == 1 {
                0.5
            } else {
                lerp(0.12, 0.88, j as f64 / (count as f64 - 1.0))
            };
            slots.push((depth, lateral, role));
        }
    }

    slots
        .into_iter()
        .map(|(d, l, r)| {
            if side == Side::Home {
                (d, l, r)
            } else {
                (1.0 - d, 1.0 - l, r)
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Mover / ball / pitch state
// ---------------------------------------------------------------------------

struct Mover {
    id: String,
    name: String,
    number: u8,
    side: Side,
    role: Position,
    home: (f64, f64),
    pos: (f64, f64),
    snap: PlayerSnap,
    active: bool,
}

impl Mover {
    /// Per-tick max movement (normalized pitch fraction), scaled by pace.
    fn speed(&self) -> f64 {
        0.010 + (self.snap.pace as f64 / 100.0) * 0.020
    }
}

/// What happens when an in-flight ball reaches its target.
enum Pending {
    /// Ball arrives to this player → they become the carrier (possession = side).
    Possess(Side, usize),
    /// A goal was scored by `side` → reset to kickoff for the opponent.
    GoalReset(Side),
}

struct Flight {
    target: (f64, f64),
    speed: f64,
    pending: Pending,
}

pub struct Pitch {
    home: Vec<Mover>,
    away: Vec<Mover>,
    ball: (f64, f64),
    ball_z: f64,
    carrier: Option<(Side, usize)>,
    flight: Option<Flight>,
    home_style: PlayStyle,
    away_style: PlayStyle,
    config: MatchConfig,
    pub possession: Side,
    seconds: f64,
}

impl Pitch {
    pub fn new(home: &TeamData, away: &TeamData, config: MatchConfig) -> Self {
        let home_movers = build_movers(home, Side::Home);
        let away_movers = build_movers(away, Side::Away);
        let mut pitch = Pitch {
            home: home_movers,
            away: away_movers,
            ball: (0.5, 0.5),
            ball_z: 0.0,
            carrier: None,
            flight: None,
            home_style: home.play_style,
            away_style: away.play_style,
            config,
            possession: Side::Home,
            seconds: 0.0,
        };
        pitch.reset_kickoff(Side::Home);
        pitch
    }

    // -- accessors ----------------------------------------------------------

    fn list(&self, side: Side) -> &Vec<Mover> {
        match side {
            Side::Home => &self.home,
            Side::Away => &self.away,
        }
    }
    fn list_mut(&mut self, side: Side) -> &mut Vec<Mover> {
        match side {
            Side::Home => &mut self.home,
            Side::Away => &mut self.away,
        }
    }
    fn mref(&self, side: Side, idx: usize) -> &Mover {
        &self.list(side)[idx]
    }

    pub fn ball_zone(&self) -> Zone {
        zone_from_x(self.ball.0)
    }

    /// Index of the `side` player (incl. GK) nearest to `pos`.
    fn nearest_to(&self, side: Side, pos: (f64, f64)) -> Option<usize> {
        self.list(side)
            .iter()
            .enumerate()
            .filter(|(_, m)| m.active)
            .min_by(|(_, a), (_, b)| {
                dist(a.pos, pos)
                    .partial_cmp(&dist(b.pos, pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
    }

    // -- kickoff ------------------------------------------------------------

    pub fn reset_kickoff(&mut self, side: Side) {
        for m in self.home.iter_mut().chain(self.away.iter_mut()) {
            m.pos = m.home;
        }
        self.ball = (0.5, 0.5);
        self.ball_z = 0.0;
        self.flight = None;
        self.possession = side;
        // central-most outfielder of `side` takes kickoff
        let idx = self
            .list(side)
            .iter()
            .enumerate()
            .filter(|(_, m)| m.active && m.role != Position::Goalkeeper)
            .min_by(|(_, a), (_, b)| {
                (a.pos.1 - 0.5)
                    .abs()
                    .partial_cmp(&(b.pos.1 - 0.5).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.carrier = Some((side, idx));
        let p = self.mref(side, idx).pos;
        self.ball = p;
    }

    // -- per-tick -----------------------------------------------------------

    pub fn tick<R: Rng>(&mut self, minute: u8, rng: &mut R) -> Vec<MatchEvent> {
        self.seconds += 60.0 / TICKS_PER_MINUTE as f64;
        self.update_movement();

        if self.flight.is_some() {
            self.advance_flight();
            return Vec::new();
        }

        self.carrier_action(minute, rng)
    }

    fn update_movement(&mut self) {
        let ball = self.ball;
        let carrier = self.carrier;
        let poss = self.possession;
        let carrier_pos = carrier.map(|(s, i)| self.mref(s, i).pos);

        for side in [Side::Home, Side::Away] {
            for idx in 0..self.list(side).len() {
                if !self.list(side)[idx].active {
                    continue;
                }
                let is_carrier = carrier == Some((side, idx));
                let (home, role, pos, speed) = {
                    let m = &self.list(side)[idx];
                    (m.home, m.role, m.pos, m.speed())
                };
                let target = if role == Position::Goalkeeper {
                    // Sweeper-keeper: step off the line when the ball is upfield.
                    let own_goal = if side == Side::Home { 0.0 } else { 1.0 };
                    let advance = ((ball.0 - own_goal).abs() * 0.12).min(0.10);
                    let gx = if side == Side::Home { 0.03 + advance } else { 0.97 - advance };
                    (gx, 0.5 + (ball.1 - 0.5) * 0.35)
                } else if is_carrier {
                    let fwd = if side == Side::Home { 0.09 } else { -0.09 };
                    ((pos.0 + fwd).clamp(0.03, 0.97), lerp(pos.1, 0.5, 0.08))
                } else {
                    // When our team has the ball, midfielders/forwards push up to
                    // support the attack so passes can reach the final third.
                    let attacking = poss == side;
                    let dir = if side == Side::Home { 1.0 } else { -1.0 };
                    let style = if side == Side::Home {
                        self.home_style
                    } else {
                        self.away_style
                    };
                    let style_push = match style {
                        PlayStyle::Attacking | PlayStyle::HighPress => 1.4,
                        PlayStyle::Defensive | PlayStyle::Counter => 0.65,
                        _ => 1.0,
                    };
                    let push = if attacking {
                        style_push
                            * match role {
                                Position::Forward => 0.26,
                                Position::Midfielder => 0.13,
                                _ => 0.0,
                            }
                    } else {
                        0.0
                    };
                    let mut tx = (home.0 + (ball.0 - 0.5) * 0.32 + dir * push).clamp(0.03, 0.97);
                    let mut ty = lerp(home.1, ball.1, 0.22);
                    // Off-ball run: forwards crash the box when the ball is advanced.
                    if attacking && role == Position::Forward {
                        let adv = if side == Side::Home { ball.0 } else { 1.0 - ball.0 };
                        if adv > 0.60 {
                            tx = if side == Side::Home { 0.84 } else { 0.16 };
                            ty = lerp(home.1, 0.5, 0.45);
                        }
                    }
                    (tx, ty)
                };
                let m = &mut self.list_mut(side)[idx];
                step_toward(&mut m.pos, target, speed);
            }
        }

        // Defending side's nearest player closes down the carrier.
        if let Some(cp) = carrier_pos {
            let def_side = poss.opposite();
            if let Some(i) = self.nearest_to(def_side, cp) {
                let speed = self.mref(def_side, i).speed() * 1.0;
                let m = &mut self.list_mut(def_side)[i];
                step_toward(&mut m.pos, cp, speed);
            }
        }

        // Ball glued to the carrier (slightly ahead in the attacking direction).
        if let Some((s, i)) = self.carrier {
            let p = self.mref(s, i).pos;
            let ahead = if s == Side::Home { 0.012 } else { -0.012 };
            self.ball = ((p.0 + ahead).clamp(0.0, 1.0), p.1);
            self.ball_z = 0.0;
        }
    }

    fn advance_flight(&mut self) {
        let arrived = {
            let f = self.flight.as_ref().unwrap();
            let d = dist(self.ball, f.target);
            if d <= f.speed {
                self.ball = f.target;
                true
            } else {
                let dx = f.target.0 - self.ball.0;
                let dy = f.target.1 - self.ball.1;
                self.ball.0 += dx / d * f.speed;
                self.ball.1 += dy / d * f.speed;
                // simple arc for visual/3D height
                self.ball_z = (0.0_f64).max(0.06 * (1.0 - (2.0 * (d / 0.5) - 1.0).abs()));
                false
            }
        };
        if arrived {
            self.ball_z = 0.0;
            let pending = self.flight.take().unwrap().pending;
            match pending {
                Pending::Possess(s, i) => {
                    self.carrier = Some((s, i));
                    self.possession = s;
                }
                Pending::GoalReset(scorer) => self.reset_kickoff(scorer.opposite()),
            }
        }
    }

    // -- carrier decision ---------------------------------------------------

    fn carrier_action<R: Rng>(&mut self, minute: u8, rng: &mut R) -> Vec<MatchEvent> {
        let (cside, cidx) = match self.carrier {
            Some(c) => c,
            None => return Vec::new(),
        };
        let cpos = self.mref(cside, cidx).pos;

        // A goalkeeper in possession distributes the ball upfield rather than
        // holding it (otherwise play stalls at the keeper's own goal).
        if self.mref(cside, cidx).role == Position::Goalkeeper {
            return self.do_pass(minute, cside, cidx, rng);
        }

        let def_side = cside.opposite();
        let near_def = self.nearest_to(def_side, cpos);
        let press_d = near_def
            .map(|i| dist(self.mref(def_side, i).pos, cpos))
            .unwrap_or(1.0);

        let in_shoot_range = if cside == Side::Home {
            cpos.0 > 0.68
        } else {
            cpos.0 < 0.32
        };
        let central = (cpos.1 - 0.5).abs() < 0.38;

        // 1) Shoot when in range and roughly central.
        if in_shoot_range && central && rng.random_range(0.0..1.0f64) < 0.62 {
            return self.do_shot(minute, cside, cidx, rng);
        }
        // 2) Tackle by the closest defender.
        if press_d < 0.04 {
            if let Some(di) = near_def {
                if rng.random_range(0.0..1.0f64) < 0.10 {
                    return self.do_challenge(minute, cside, cidx, def_side, di, rng);
                }
            }
        }
        // 3) Pass out of pressure.
        if press_d < 0.07 && rng.random_range(0.0..1.0f64) < 0.55 {
            return self.do_pass(minute, cside, cidx, rng);
        }
        // 4) Otherwise keep dribbling (movement already advanced the carrier).
        Vec::new()
    }

    fn do_shot<R: Rng>(&mut self, minute: u8, att: Side, cidx: usize, rng: &mut R) -> Vec<MatchEvent> {
        let def = att.opposite();
        let zone = Zone::attacking_box(att);
        let shooter = self.mref(att, cidx).snap.clone();
        let gk_idx = self.gk_index(def);
        let gk = gk_idx.map(|i| self.mref(def, i).snap.clone());

        let shoot_raw =
            (shooter.shooting as f64 + shooter.composure as f64 + shooter.decisions as f64) / 3.0;
        let shoot_rating = shoot_raw * trait_bonus(&shooter, TraitContext::Shooting);
        let gk_rating = gk
            .as_ref()
            .map(|g| {
                (g.handling as f64 + g.reflexes as f64 + g.positioning as f64) / 3.0
                    * trait_bonus(g, TraitContext::Goalkeeping)
            })
            .unwrap_or(45.0);

        let accuracy =
            (self.config.shot_accuracy_base + 0.10 + (shoot_rating - 50.0) / 200.0).clamp(0.15, 0.90);

        let mut events = Vec::new();
        let shooter_id = shooter.id.clone();

        // Aim at the goal mouth.
        let goal = (if att == Side::Home { 1.0 } else { 0.0 }, 0.5);

        if rng.random_range(0.0..1.0f64) > accuracy {
            let off = rng.random_range(0.0..1.0f64) < 0.4;
            events.push(MatchEvent::new(
                minute,
                if off {
                    EventType::ShotBlocked
                } else {
                    EventType::ShotOffTarget
                },
                att,
                zone,
            ).with_player(&shooter_id));
            // ball goes to the keeper / out → opponent restarts
            self.start_flight(goal, Pending::Possess(def, self.gk_index(def).unwrap_or(0)));
            return events;
        }

        let conversion = (self.config.goal_conversion_base + 0.12 + (shoot_rating - gk_rating) / 150.0)
            .clamp(0.10, 0.75);

        if rng.random_range(0.0..1.0f64) < conversion {
            events.push(
                MatchEvent::new(minute, EventType::Goal, att, zone).with_player(&shooter_id),
            );
            self.start_flight(goal, Pending::GoalReset(att));
        } else {
            events.push(
                MatchEvent::new(minute, EventType::ShotSaved, att, zone).with_player(&shooter_id),
            );
            self.start_flight(goal, Pending::Possess(def, self.gk_index(def).unwrap_or(0)));
        }
        events
    }

    fn do_pass<R: Rng>(&mut self, minute: u8, att: Side, cidx: usize, rng: &mut R) -> Vec<MatchEvent> {
        let def = att.opposite();
        let zone = zone_from_x(self.ball.0);
        let passer = self.mref(att, cidx).snap.clone();
        let passer_id = passer.id.clone();

        let receiver = match self.best_pass_target(att, cidx) {
            Some(r) => r,
            None => return Vec::new(),
        };
        let recv_pos = self.mref(att, receiver).pos;

        let pass_skill = (passer.passing as f64
            + passer.vision as f64
            + passer.composure as f64
            + passer.teamwork as f64)
            / 4.0
            * trait_bonus(&passer, TraitContext::Passing);

        // Pressure = nearest opponent to the receiving target.
        let interceptor = self.nearest_to(def, recv_pos).unwrap_or(0);
        let press = {
            let d = dist(self.mref(def, interceptor).pos, recv_pos);
            let raw = self.mref(def, interceptor).snap.positioning as f64;
            raw * (0.04 / d.max(0.02)).min(2.0)
        };

        let success = (pass_skill * 1.9) / (pass_skill * 1.9 + press);
        let mut events = Vec::new();
        if rng.random_range(0.0..1.0f64) < success {
            events.push(
                MatchEvent::new(minute, EventType::PassCompleted, att, zone).with_player(&passer_id),
            );
            self.start_flight(recv_pos, Pending::Possess(att, receiver));
        } else {
            let int_id = self.mref(def, interceptor).snap.id.clone();
            events.push(
                MatchEvent::new(minute, EventType::PassIntercepted, att, zone)
                    .with_player(&passer_id),
            );
            events.push(
                MatchEvent::new(minute, EventType::Interception, def, zone).with_player(&int_id),
            );
            let ipos = self.mref(def, interceptor).pos;
            self.start_flight(ipos, Pending::Possess(def, interceptor));
        }
        events
    }

    fn do_challenge<R: Rng>(
        &mut self,
        minute: u8,
        att: Side,
        cidx: usize,
        def: Side,
        didx: usize,
        rng: &mut R,
    ) -> Vec<MatchEvent> {
        let zone = zone_from_x(self.ball.0);
        let attacker = self.mref(att, cidx).snap.clone();
        let defender = self.mref(def, didx).snap.clone();

        let att_rating = (attacker.dribbling as f64
            + attacker.pace as f64
            + attacker.agility as f64
            + attacker.composure as f64)
            / 4.0
            * trait_bonus(&attacker, TraitContext::Dribbling);
        let def_rating = (defender.tackling as f64
            + defender.positioning as f64
            + defender.strength as f64)
            / 3.0
            * trait_bonus(&defender, TraitContext::Tackling);

        let att_wins = att_rating / (att_rating + def_rating);
        let mut events = Vec::new();
        if rng.random_range(0.0..1.0f64) < att_wins {
            // dribble past — no turnover
            return events;
        }

        events.push(
            MatchEvent::new(minute, EventType::DribbleTackled, att, zone)
                .with_player(&attacker.id)
                .with_secondary(&defender.id),
        );
        events.push(MatchEvent::new(minute, EventType::Tackle, def, zone).with_player(&defender.id));

        // Possible foul (no cards in v1 — that re-enters via integration later).
        let foul_chance = self.config.foul_probability
            * (0.6 + defender.aggression as f64 / 100.0 * 0.8)
            * trait_bonus(&defender, TraitContext::Foul);
        if rng.random_range(0.0..1.0f64) < foul_chance {
            events.push(
                MatchEvent::new(minute, EventType::Foul, def, zone)
                    .with_player(&defender.id)
                    .with_secondary(&attacker.id),
            );
            // free kick: attacking side (the fouled team) keeps the ball
            self.carrier = Some((att, cidx));
            self.possession = att;
        } else {
            self.carrier = Some((def, didx));
            self.possession = def;
            self.ball = self.mref(def, didx).pos;
        }
        events
    }

    fn start_flight(&mut self, target: (f64, f64), pending: Pending) {
        self.carrier = None;
        self.flight = Some(Flight {
            target,
            speed: 0.05,
            pending,
        });
    }

    fn gk_index(&self, side: Side) -> Option<usize> {
        self.list(side)
            .iter()
            .position(|m| m.active && m.role == Position::Goalkeeper)
    }

    /// Replace an on-pitch player (substitution), keeping the same slot/position.
    pub fn substitute(&mut self, off_id: &str, on: &crate::types::PlayerData) {
        for m in self.home.iter_mut().chain(self.away.iter_mut()) {
            if m.id == off_id {
                m.id = on.id.clone();
                m.name = on.name.clone();
                m.snap = PlayerSnap::from(on);
                m.active = true;
            }
        }
    }

    /// Reposition a side's players to a new formation (mid-match tactic change).
    pub fn apply_formation(&mut self, side: Side, formation: &str) {
        let slots = formation_positions(formation, side);
        let list = match side {
            Side::Home => &mut self.home,
            Side::Away => &mut self.away,
        };
        for (i, m) in list.iter_mut().enumerate() {
            if let Some(&(x, y, role)) = slots.get(i) {
                m.home = (x, y);
                m.role = role;
            }
        }
    }

    /// Update a side's play-style (affects how high the team pushes).
    pub fn set_play_style(&mut self, side: Side, style: PlayStyle) {
        match side {
            Side::Home => self.home_style = style,
            Side::Away => self.away_style = style,
        }
    }

    /// Send a player off (red card): mark inactive so indices stay stable; if
    /// they had the ball it passes to the nearest remaining player.
    pub fn send_off(&mut self, player_id: &str) {
        for m in self.home.iter_mut().chain(self.away.iter_mut()) {
            if m.id == player_id {
                m.active = false;
            }
        }
        if let Some((s, i)) = self.carrier {
            if !self.list(s)[i].active {
                self.carrier = None;
                self.flight = None;
                let ball = self.ball;
                let mut best: Option<(Side, usize, f64)> = None;
                for side in [Side::Home, Side::Away] {
                    for (idx, m) in self.list(side).iter().enumerate() {
                        if !m.active {
                            continue;
                        }
                        let d = dist(m.pos, ball);
                        if best.map(|(_, _, bd)| d < bd).unwrap_or(true) {
                            best = Some((side, idx, d));
                        }
                    }
                }
                if let Some((s2, i2, _)) = best {
                    self.carrier = Some((s2, i2));
                    self.possession = s2;
                }
            }
        }
    }

    /// Best forward, open team-mate to receive a pass.
    fn best_pass_target(&self, att: Side, cidx: usize) -> Option<usize> {
        let def = att.opposite();
        let mut best: Option<(usize, f64)> = None;
        for (i, m) in self.list(att).iter().enumerate() {
            if i == cidx || !m.active || m.role == Position::Goalkeeper {
                continue;
            }
            let forwardness = if att == Side::Home { m.pos.0 } else { 1.0 - m.pos.0 };
            let openness = self
                .nearest_to(def, m.pos)
                .map(|di| dist(self.mref(def, di).pos, m.pos))
                .unwrap_or(0.2);
            let score = forwardness * 0.6 + openness.min(0.2) * 2.0;
            if best.map(|(_, s)| score > s).unwrap_or(true) {
                best = Some((i, score));
            }
        }
        best.map(|(i, _)| i)
    }

    // -- frame --------------------------------------------------------------

    pub fn frame(&self) -> MatchFrame {
        let mut players = Vec::with_capacity(22);
        for side in [Side::Home, Side::Away] {
            for (idx, m) in self.list(side).iter().enumerate() {
                if !m.active {
                    continue;
                }
                players.push(FramePlayer {
                    id: m.id.clone(),
                    side,
                    x: m.pos.0,
                    y: m.pos.1,
                    has_ball: self.carrier == Some((side, idx)),
                    number: m.number,
                    name: m.name.clone(),
                    role: role_str(m.role),
                });
            }
        }
        MatchFrame {
            t: self.seconds,
            ball: FrameBall {
                x: self.ball.0,
                y: self.ball.1,
                z: self.ball_z,
            },
            players,
        }
    }
}

fn step_toward(pos: &mut (f64, f64), target: (f64, f64), step: f64) {
    let dx = target.0 - pos.0;
    let dy = target.1 - pos.1;
    let d = (dx * dx + dy * dy).sqrt();
    if d > 1e-9 {
        let f = (step / d).min(1.0);
        pos.0 += dx * f;
        pos.1 += dy * f;
    }
    pos.0 = pos.0.clamp(0.0, 1.0);
    pos.1 = pos.1.clamp(0.0, 1.0);
}

fn build_movers(team: &TeamData, side: Side) -> Vec<Mover> {
    let slots = formation_positions(&team.formation, side);
    team.players
        .iter()
        .take(slots.len())
        .enumerate()
        .map(|(i, p)| {
            let (x, y, role) = slots[i];
            Mover {
                id: p.id.clone(),
                name: p.name.clone(),
                number: (i + 1) as u8,
                side,
                role,
                home: (x, y),
                pos: (x, y),
                snap: PlayerSnap::from(p),
                active: true,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PlayStyle;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn mk_player(id: &str, pos: Position) -> crate::types::PlayerData {
        crate::types::PlayerData {
            id: id.to_string(),
            name: format!("P{id}"),
            position: pos,
            ovr: 70,
            condition: 100,
            fitness: 80,
            pace: 70,
            stamina: 70,
            strength: 70,
            agility: 70,
            passing: 70,
            shooting: 70,
            tackling: 70,
            dribbling: 70,
            defending: 70,
            positioning: 70,
            vision: 70,
            decisions: 70,
            composure: 70,
            aggression: 60,
            teamwork: 70,
            leadership: 60,
            handling: 70,
            reflexes: 70,
            aerial: 70,
            traits: vec![],
        }
    }

    fn mk_team(id: &str) -> TeamData {
        let mut players = vec![mk_player(&format!("{id}-gk"), Position::Goalkeeper)];
        for i in 0..4 {
            players.push(mk_player(&format!("{id}-d{i}"), Position::Defender));
        }
        for i in 0..4 {
            players.push(mk_player(&format!("{id}-m{i}"), Position::Midfielder));
        }
        for i in 0..2 {
            players.push(mk_player(&format!("{id}-f{i}"), Position::Forward));
        }
        TeamData {
            id: id.to_string(),
            name: id.to_string(),
            formation: "4-4-2".to_string(),
            play_style: PlayStyle::Balanced,
            players,
        }
    }

    #[test]
    fn simulates_without_panic_and_emits_events_and_frames() {
        let home = mk_team("H");
        let away = mk_team("A");
        let mut pitch = Pitch::new(&home, &away, MatchConfig::default());
        let mut rng = StdRng::seed_from_u64(42);

        let mut events = 0usize;
        let mut frames = 0usize;
        for minute in 1..=90u8 {
            for _ in 0..TICKS_PER_MINUTE {
                events += pitch.tick(minute, &mut rng).len();
                let f = pitch.frame();
                assert_eq!(f.players.len(), 22);
                // positions stay on the pitch
                for p in &f.players {
                    assert!((0.0..=1.0).contains(&p.x) && (0.0..=1.0).contains(&p.y));
                }
                frames += 1;
            }
        }
        assert_eq!(frames, 90 * TICKS_PER_MINUTE as usize);
        assert!(events > 0, "expected the positional sim to emit events");
    }
}
