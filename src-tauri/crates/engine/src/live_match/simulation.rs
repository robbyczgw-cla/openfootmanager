use rand::{Rng, RngExt};

use crate::event::{EventType, MatchEvent};
use crate::types::{Side, Zone};

use super::{LiveMatchState, MatchPhase, MinuteResult};

// ---------------------------------------------------------------------------
// Phase transitions
// ---------------------------------------------------------------------------

impl LiveMatchState {
    pub(super) fn start_match<R: Rng>(&mut self, rng: &mut R) -> MinuteResult {
        // Rebuild the pitch from the final kickoff lineup (reflects any pre-match
        // swaps / formation choices made before kickoff).
        self.pitch = super::positional::Pitch::new(&self.home, &self.away, self.config.clone());
        self.phase = MatchPhase::FirstHalf;
        self.current_minute = 0;
        self.ball_zone = Zone::Midfield;
        self.possession = Side::Home;
        self.first_half_stoppage = rng.random_range(0..=self.config.stoppage_time_max);

        let evt = MatchEvent::new(0, EventType::KickOff, Side::Home, Zone::Midfield);
        self.events.push(evt.clone());

        MinuteResult {
            minute: 0,
            phase: MatchPhase::FirstHalf,
            events: vec![evt],
            home_score: 0,
            away_score: 0,
            possession: Side::Home,
            ball_zone: Zone::Midfield,
            is_finished: false,
            frames: vec![self.pitch.frame()],
        }
    }

    pub(super) fn start_second_half<R: Rng>(&mut self, rng: &mut R) -> MinuteResult {
        self.phase = MatchPhase::SecondHalf;
        // Second half starts after halftime; use at least minute 46 but never before current_minute
        let start_min = self.current_minute.max(46);
        self.current_minute = start_min;
        self.ball_zone = Zone::Midfield;
        self.possession = Side::Away;
        self.pitch.reset_kickoff(Side::Away);
        self.second_half_stoppage = rng.random_range(0..=self.config.stoppage_time_max);

        let evt = MatchEvent::new(
            start_min,
            EventType::SecondHalfStart,
            Side::Away,
            Zone::Midfield,
        );
        self.events.push(evt.clone());

        MinuteResult {
            minute: start_min,
            phase: MatchPhase::SecondHalf,
            events: vec![evt],
            home_score: self.home_score,
            away_score: self.away_score,
            possession: Side::Away,
            ball_zone: Zone::Midfield,
            is_finished: false,
            frames: vec![self.pitch.frame()],
        }
    }

    pub(super) fn start_et_second_half<R: Rng>(&mut self, rng: &mut R) -> MinuteResult {
        self.phase = MatchPhase::ExtraTimeSecondHalf;
        let start_min = self.current_minute.max(106);
        self.current_minute = start_min;
        self.ball_zone = Zone::Midfield;
        self.possession = Side::Home;
        self.pitch.reset_kickoff(Side::Home);
        self.et_second_half_stoppage = rng.random_range(0..=2); // short stoppage in ET

        let evt = MatchEvent::new(
            start_min,
            EventType::SecondHalfStart,
            Side::Home,
            Zone::Midfield,
        );
        self.events.push(evt.clone());

        MinuteResult {
            minute: start_min,
            phase: MatchPhase::ExtraTimeSecondHalf,
            events: vec![evt],
            home_score: self.home_score,
            away_score: self.away_score,
            possession: Side::Home,
            ball_zone: Zone::Midfield,
            is_finished: false,
            frames: vec![self.pitch.frame()],
        }
    }

    pub(super) fn handle_full_time<R: Rng>(&mut self, rng: &mut R) -> MinuteResult {
        if self.allows_extra_time && self.home_score == self.away_score {
            // Go to extra time
            self.phase = MatchPhase::ExtraTimeFirstHalf;
            self.current_minute = 91;
            self.ball_zone = Zone::Midfield;
            self.possession = Side::Home;
            self.et_first_half_stoppage = rng.random_range(0..=2);
            self.pitch.reset_kickoff(Side::Home);

            let evt = MatchEvent::new(91, EventType::KickOff, Side::Home, Zone::Midfield);
            self.events.push(evt.clone());

            MinuteResult {
                minute: 91,
                phase: MatchPhase::ExtraTimeFirstHalf,
                events: vec![evt],
                home_score: self.home_score,
                away_score: self.away_score,
                possession: Side::Home,
                ball_zone: Zone::Midfield,
                is_finished: false,
                frames: vec![self.pitch.frame()],
            }
        } else {
            // Match decided in normal time
            self.phase = MatchPhase::Finished;
            self.make_result(true)
        }
    }

    pub(super) fn handle_et_end<R: Rng>(&mut self, _rng: &mut R) -> MinuteResult {
        if self.home_score == self.away_score {
            // Go to penalty shootout
            self.phase = MatchPhase::PenaltyShootout;
            self.penalty_state = super::PenaltyShootoutState::default();

            let evt = MatchEvent::new(
                self.current_minute,
                EventType::PenaltyAwarded,
                Side::Home,
                Zone::Midfield,
            );
            self.events.push(evt.clone());

            MinuteResult {
                minute: self.current_minute,
                phase: MatchPhase::PenaltyShootout,
                events: vec![evt],
                home_score: self.home_score,
                away_score: self.away_score,
                possession: self.possession,
                ball_zone: Zone::Midfield,
                is_finished: false,
                frames: vec![self.pitch.frame()],
            }
        } else {
            self.phase = MatchPhase::Finished;
            self.make_result(true)
        }
    }

    // -----------------------------------------------------------------------
    // Core minute simulation
    // -----------------------------------------------------------------------

    pub(super) fn play_minute<R: Rng>(&mut self, rng: &mut R) -> MinuteResult {
        self.current_minute += 1;
        let minute = self.current_minute;

        // Deplete stamina for all on-pitch players
        self.deplete_stamina_tick();

        // Run the positional engine for this minute: each tick advances movement
        // and may resolve an action (pass/shot/tackle) into events. The frames
        // are the per-tick positions the 2D/3D renderer plays back.
        let mut minute_events = Vec::new();
        let mut frames = Vec::with_capacity(super::positional::TICKS_PER_MINUTE as usize);
        for _ in 0..super::positional::TICKS_PER_MINUTE {
            let tick_events = self.pitch.tick(minute, rng);
            for evt in &tick_events {
                if evt.is_goal() {
                    self.add_goal(evt.side);
                }
                self.events.push(evt.clone());
            }
            minute_events.extend(tick_events);
            frames.push(self.pitch.frame());
        }

        // Discipline: turn positional fouls into cards / penalties, reusing the
        // existing rules. Collect descriptors first to avoid borrow conflicts.
        let fouls: Vec<(Side, Option<String>, Zone)> = minute_events
            .iter()
            .filter(|e| e.event_type == EventType::Foul)
            .map(|e| (e.side, e.player_id.clone(), e.zone))
            .collect();
        for (fouling_side, fouler_id, zone) in fouls {
            let att_side = fouling_side.opposite();
            if zone.is_box_for(att_side)
                && rng.random_range(0.0..1.0f64) < self.config.penalty_probability
            {
                let awarded = MatchEvent::new(minute, EventType::PenaltyAwarded, att_side, zone);
                self.events.push(awarded.clone());
                minute_events.push(awarded);
                let pen = self.resolve_in_match_penalty(minute, att_side, rng);
                let scored = pen.iter().any(|e| e.event_type == EventType::PenaltyGoal);
                minute_events.extend(pen);
                self.pitch
                    .reset_kickoff(if scored { att_side.opposite() } else { att_side });
            }
            if let Some(fid) = fouler_id {
                let cards = self.maybe_card(minute, fouling_side, &fid, zone, rng);
                for c in &cards {
                    if matches!(c.event_type, EventType::RedCard | EventType::SecondYellow) {
                        self.pitch.send_off(&fid);
                    }
                }
                minute_events.extend(cards);
            }
        }

        // Possession % + sync the coarse snapshot fields from the pitch.
        match self.pitch.possession {
            Side::Home => self.home_possession_ticks += 1,
            Side::Away => self.away_possession_ticks += 1,
        }
        self.possession = self.pitch.possession;
        self.ball_zone = self.pitch.ball_zone();

        // Check for phase transitions
        let transition_events = self.check_phase_end(minute, rng);
        minute_events.extend(transition_events);

        MinuteResult {
            minute,
            phase: self.phase,
            events: minute_events,
            home_score: self.home_score,
            away_score: self.away_score,
            possession: self.possession,
            ball_zone: self.ball_zone,
            is_finished: self.phase == MatchPhase::Finished,
            frames,
        }
    }

    fn check_phase_end<R: Rng>(&mut self, minute: u8, _rng: &mut R) -> Vec<MatchEvent> {
        let mut events = Vec::new();
        match self.phase {
            MatchPhase::FirstHalf => {
                if minute >= 45 + self.first_half_stoppage {
                    self.phase = MatchPhase::HalfTime;
                    let evt =
                        MatchEvent::new(minute, EventType::HalfTime, Side::Home, Zone::Midfield);
                    self.events.push(evt.clone());
                    events.push(evt);
                }
            }
            MatchPhase::SecondHalf => {
                if minute >= 90 + self.second_half_stoppage {
                    self.phase = MatchPhase::FullTime;
                    let evt =
                        MatchEvent::new(minute, EventType::FullTime, Side::Home, Zone::Midfield);
                    self.events.push(evt.clone());
                    events.push(evt);
                }
            }
            MatchPhase::ExtraTimeFirstHalf => {
                if minute >= 105 + self.et_first_half_stoppage {
                    self.phase = MatchPhase::ExtraTimeHalfTime;
                    let evt =
                        MatchEvent::new(minute, EventType::HalfTime, Side::Home, Zone::Midfield);
                    self.events.push(evt.clone());
                    events.push(evt);
                }
            }
            MatchPhase::ExtraTimeSecondHalf => {
                if minute >= 120 + self.et_second_half_stoppage {
                    self.phase = MatchPhase::ExtraTimeEnd;
                    let evt =
                        MatchEvent::new(minute, EventType::FullTime, Side::Home, Zone::Midfield);
                    self.events.push(evt.clone());
                    events.push(evt);
                }
            }
            _ => {}
        }
        events
    }

    pub(super) fn make_result(&self, _is_finished: bool) -> MinuteResult {
        MinuteResult {
            minute: self.current_minute,
            phase: self.phase,
            events: Vec::new(),
            home_score: self.home_score,
            away_score: self.away_score,
            possession: self.possession,
            ball_zone: self.ball_zone,
            is_finished: true,
            frames: vec![self.pitch.frame()],
        }
    }
}
