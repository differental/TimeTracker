// TimeTracker - Rust-based web app that tracks and analyses user's daily routine to provide insight in time management.
// Copyright (C) 2025 Brian Chen (differental)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, version 3.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Next-activity prediction by interpolated multi-level context scoring.
//!
//! Every [`Level`] is a table of observation counts keyed on a different slice of
//! context, from the coarsest (the global activity mix) to the finest (weekday,
//! time of day and the activity being left). A state's score is the sum, over all
//! levels, of that level's share of the evidence it holds:
//!
//! ```text
//! score(s) = Σ  weight · count[level][context][s] / (total[level][context] + smoothing)
//! ```
//!
//! The `smoothing` term is what makes this a backoff model: a level that has only
//! seen a context once or twice contributes very little, so a fine-grained level
//! refines the ranking where it has evidence and stays out of the way where it
//! does not. That matters because an activity log is small — a year of tracking is
//! a few thousand events — and conditioning hard on weekday *and* hour of day
//! splits it into cells holding a handful of samples each.
//!
//! Counts decay geometrically per event, so a routine that changed a month ago
//! stops outvoting the current one. Decay is applied lazily: a row records the
//! step it was last touched and is scaled on read.
//!
//! This replaces an earlier TAGE-style (TAgged GEometric history length) predictor
//! ported from the iOS client. Measured walk-forward over a real 3709-entry log,
//! that design scored 49.6% top-1 / 83.3% top-3 / 93.9% top-5 — below a plain
//! `(time of day, current activity)` frequency table — because it selected a
//! single winning table and concatenated its at-most-three candidates, letting one
//! entry allocated from a single observation fill every suggestion slot. Scoring
//! every state against every level instead gives 62.9% / 90.3% / 94.8%.

use std::collections::HashMap;

use chrono::{DateTime, Datelike, FixedOffset, TimeZone, Timelike};

use crate::constants::STATE_COUNT;

/// A slice of context to condition on. Each level owns an independent table, so
/// two levels never share a row even if their keys collide numerically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// The overall activity mix, ignoring all context.
    Global,
    /// The activity being left.
    Current,
    /// Block of the day (see [`Configuration::daypart_hours`]) and current activity.
    DaypartCurrent,
    /// Hour of day and current activity.
    HourCurrent,
    /// Weekend/weekday, block of the day, and current activity.
    WeekendDaypartCurrent,
    /// Weekend/weekday, hour of day, and current activity.
    WeekendHourCurrent,
    /// The two activities most recently left.
    SecondOrder,
    /// How long the current activity has been running, and the current activity.
    ElapsedCurrent,
    /// Exact weekday, block of the day, and current activity.
    WeekdayDaypartCurrent,
}

/// Tunables for the predictor.
#[derive(Clone, Debug)]
pub struct Configuration {
    /// Levels to score against, each with a relative weight.
    pub levels: Vec<(Level, f64)>,
    /// Pseudo-count added to every level's denominator. Higher values make
    /// sparsely-observed levels contribute less.
    pub smoothing: f64,
    /// Per-event multiplier applied to stored counts. 1.0 disables decay; 0.995 is
    /// a half-life of roughly 140 events.
    pub decay: f64,
    /// Width of a [`Level::DaypartCurrent`] bucket, in hours.
    pub daypart_hours: u32,
    /// Only the most recent N entries are trained on.
    pub maximum_training_entries: usize,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            levels: vec![
                (Level::Global, 1.0),
                (Level::Current, 1.0),
                (Level::DaypartCurrent, 1.0),
                (Level::HourCurrent, 1.0),
                (Level::WeekendDaypartCurrent, 1.0),
                (Level::WeekendHourCurrent, 1.0),
                (Level::SecondOrder, 1.0),
                (Level::ElapsedCurrent, 1.0),
                (Level::WeekdayDaypartCurrent, 1.0),
            ],
            smoothing: 4.0,
            decay: 0.995,
            daypart_hours: 4,
            maximum_training_entries: 10_000,
        }
    }
}

/// One historical activity entry: which state was started, and when.
#[derive(Clone, Copy, Debug)]
pub struct TrainingEntry {
    pub state_id: usize,
    pub start_timestamp: i64,
}

struct Context {
    hour: u32,
    daypart: u32,
    weekday: u32,
    is_weekend: u32,
    elapsed_bucket: u32,
    current_state_id: Option<usize>,
    previous_state_id: Option<usize>,
}

impl Context {
    fn key(&self, level: Level) -> Option<u64> {
        let hour = u64::from(self.hour);
        let daypart = u64::from(self.daypart);
        let weekend = u64::from(self.is_weekend);
        match level {
            Level::Global => Some(0),
            Level::Current => Some(self.current_state_id? as u64),
            Level::DaypartCurrent => Some(daypart * 16 + self.current_state_id? as u64),
            Level::HourCurrent => Some(hour * 16 + self.current_state_id? as u64),
            Level::WeekendDaypartCurrent => {
                Some(weekend * 512 + daypart * 16 + self.current_state_id? as u64)
            }
            Level::WeekendHourCurrent => {
                Some(weekend * 512 + hour * 16 + self.current_state_id? as u64)
            }
            Level::SecondOrder => Some(
                self.current_state_id? as u64 * 16
                    + self.previous_state_id.map_or(15, |id| id as u64),
            ),
            Level::ElapsedCurrent => {
                Some(u64::from(self.elapsed_bucket) * 16 + self.current_state_id? as u64)
            }
            Level::WeekdayDaypartCurrent => {
                Some(u64::from(self.weekday) * 512 + daypart * 16 + self.current_state_id? as u64)
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Row {
    counts: [f32; STATE_COUNT],
    total: f32,
    last_touch: u64,
}

struct Table {
    level: Level,
    weight: f64,
    rows: HashMap<u64, Row>,
}

impl Table {
    fn observe(&mut self, key: u64, state_id: usize, step: u64, decay: f64) {
        let row = self.rows.entry(key).or_insert_with(|| Row {
            counts: [0.0; STATE_COUNT],
            total: 0.0,
            last_touch: step,
        });
        let factor = decay_factor(decay, row.last_touch, step) as f32;
        if factor != 1.0 {
            for count in row.counts.iter_mut() {
                *count *= factor;
            }
            row.total *= factor;
        }
        row.last_touch = step;
        row.counts[state_id] += 1.0;
        row.total += 1.0;
    }

    fn score_into(&self, key: u64, step: u64, decay: f64, smoothing: f64, out: &mut [f64]) {
        let Some(row) = self.rows.get(&key) else {
            return;
        };
        let factor = decay_factor(decay, row.last_touch, step);
        let denominator = f64::from(row.total) * factor + smoothing;
        if denominator <= 0.0 {
            return;
        }
        let scale = self.weight * factor / denominator;
        for (state_id, count) in row.counts.iter().enumerate() {
            if *count > 0.0 {
                out[state_id] += f64::from(*count) * scale;
            }
        }
    }
}

fn decay_factor(decay: f64, last_touch: u64, step: u64) -> f64 {
    if decay >= 1.0 || step <= last_touch {
        return 1.0;
    }
    decay.powf((step - last_touch) as f64)
}

fn elapsed_bucket(elapsed_ms: i64) -> u32 {
    match elapsed_ms / 60_000 {
        0..=15 => 1,
        16..=45 => 2,
        46..=120 => 3,
        121..=300 => 4,
        _ => 5,
    }
}

pub struct ActivityPredictor {
    configuration: Configuration,
    offset: FixedOffset,
    tables: Vec<Table>,
    current_state_id: Option<usize>,
    previous_state_id: Option<usize>,
    last_event_at: Option<i64>,
    training_sequence: u64,
}

impl ActivityPredictor {
    /// Trains a predictor on `entries`. Weekday and time-of-day context is derived
    /// in the `offset` timezone, so the caller must pass the user's local UTC
    /// offset for time-of-day patterns to line up.
    pub fn new(
        entries: &[TrainingEntry],
        offset: FixedOffset,
        configuration: Configuration,
    ) -> Self {
        assert!(configuration.smoothing >= 0.0);
        assert!(configuration.decay > 0.0);

        let tables = configuration
            .levels
            .iter()
            .map(|&(level, weight)| Table {
                level,
                weight,
                rows: HashMap::new(),
            })
            .collect();

        let mut predictor = Self {
            configuration,
            offset,
            tables,
            current_state_id: None,
            previous_state_id: None,
            last_event_at: None,
            training_sequence: 0,
        };

        let mut ordered = entries.to_vec();
        ordered.sort_by_key(|entry| entry.start_timestamp);
        let skip = ordered
            .len()
            .saturating_sub(predictor.configuration.maximum_training_entries);

        for entry in &ordered[skip..] {
            predictor.train(entry.state_id, entry.start_timestamp);
        }

        predictor
    }

    /// Returns up to `limit` suggested next activities, best first. Never suggests
    /// `current_state_id`, never repeats, and always fills up to `limit` (falling
    /// back to unobserved activities in canonical order).
    pub fn predictions(
        &self,
        at: i64,
        current_state_id: Option<usize>,
        limit: usize,
    ) -> Vec<usize> {
        if limit == 0 {
            return Vec::new();
        }

        let context = self.make_context(at, current_state_id);
        let mut scores = [0.0f64; STATE_COUNT];
        for table in &self.tables {
            if let Some(key) = context.key(table.level) {
                table.score_into(
                    key,
                    self.training_sequence,
                    self.configuration.decay,
                    self.configuration.smoothing,
                    &mut scores,
                );
            }
        }

        let mut ranked: Vec<usize> = (0..STATE_COUNT).collect();
        ranked.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]).then(a.cmp(&b)));

        let mut state_ids: Vec<usize> = Vec::with_capacity(limit);
        for state_id in ranked {
            push_unique(&mut state_ids, state_id, current_state_id, limit);
            if state_ids.len() == limit {
                break;
            }
        }

        state_ids
    }

    /// Folds one observed activity into every level's counts.
    pub fn train(&mut self, target_state_id: usize, at: i64) {
        let context = self.make_context(at, self.current_state_id);
        let step = self.training_sequence;
        let decay = self.configuration.decay;

        for table in self.tables.iter_mut() {
            if let Some(key) = context.key(table.level) {
                table.observe(key, target_state_id, step, decay);
            }
        }

        self.previous_state_id = self.current_state_id;
        self.current_state_id = Some(target_state_id);
        self.last_event_at = Some(at);
        self.training_sequence += 1;
    }

    fn make_context(&self, at: i64, current_state_id: Option<usize>) -> Context {
        let local: DateTime<FixedOffset> = self
            .offset
            .timestamp_millis_opt(at)
            .single()
            .unwrap_or_else(|| self.offset.timestamp_nanos(0));
        let hour = local.hour();
        let weekday = local.weekday().num_days_from_sunday();
        let previous_state_id = if current_state_id == self.current_state_id {
            self.previous_state_id
        } else {
            self.current_state_id
        };

        Context {
            hour,
            daypart: hour / self.configuration.daypart_hours.max(1),
            weekday,
            is_weekend: u32::from(weekday == 0 || weekday == 6),
            elapsed_bucket: match self.last_event_at {
                Some(previous) => elapsed_bucket((at - previous).max(0)),
                None => 0,
            },
            current_state_id,
            previous_state_id,
        }
    }
}

fn push_unique(
    state_ids: &mut Vec<usize>,
    state_id: usize,
    current_state_id: Option<usize>,
    limit: usize,
) {
    if state_ids.len() >= limit
        || Some(state_id) == current_state_id
        || state_ids.contains(&state_id)
    {
        return;
    }
    state_ids.push(state_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc() -> FixedOffset {
        FixedOffset::east_opt(0).unwrap()
    }

    fn make_entries(states: &[usize]) -> Vec<TrainingEntry> {
        states
            .iter()
            .enumerate()
            .map(|(offset, &state_id)| TrainingEntry {
                state_id,
                start_timestamp: 1_700_000_000_000 + offset as i64 * 60_000,
            })
            .collect()
    }

    #[test]
    fn cold_start_uses_canonical_order() {
        let predictor = ActivityPredictor::new(&[], utc(), Configuration::default());
        assert_eq!(
            predictor.predictions(1_700_000_000_000, None, 3),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn always_returns_unique_non_current_activities() {
        let entries = make_entries(&[0, 1, 0, 2, 0, 1, 0, 3]);
        let predictor = ActivityPredictor::new(&entries, utc(), Configuration::default());
        let last = entries.last().unwrap().start_timestamp + 60_000;
        let predictions = predictor.predictions(last, Some(3), 3);

        assert_eq!(predictions.len(), 3);
        assert!(!predictions.contains(&3));
        let mut unique = predictions.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 3);
    }

    #[test]
    fn learns_repeating_action_sequence() {
        let states: Vec<usize> = std::iter::repeat([0, 1, 2]).take(20).flatten().collect();
        let entries = make_entries(&states);
        let predictor = ActivityPredictor::new(&entries, utc(), Configuration::default());
        let last = entries.last().unwrap().start_timestamp + 60_000;

        assert_eq!(predictor.predictions(last, Some(2), 3).first(), Some(&0));
    }

    #[test]
    fn honors_training_entry_limit() {
        let mut states = vec![1usize; 20];
        states.extend_from_slice(&[2, 2, 2]);
        let entries = make_entries(&states);
        let predictor = ActivityPredictor::new(
            &entries,
            utc(),
            Configuration {
                maximum_training_entries: 3,
                ..Configuration::default()
            },
        );
        let last = entries.last().unwrap().start_timestamp + 60_000;

        assert_eq!(predictor.predictions(last, None, 3).first(), Some(&2));
    }

    #[test]
    fn uses_weekday_and_hour_context() {
        let utc = utc();
        let mut entries = Vec::new();
        for week in 0..8i64 {
            let day = 5 + week * 7;
            for (hour, minute, state_id) in [(8, 59, 0), (9, 0, 1), (17, 59, 0), (18, 0, 2)] {
                let date = utc
                    .with_ymd_and_hms(2026, 1, 1, hour, minute, 0)
                    .single()
                    .unwrap()
                    + chrono::Duration::days(day - 1);
                entries.push(TrainingEntry {
                    state_id,
                    start_timestamp: date.timestamp_millis(),
                });
            }
        }

        let predictor = ActivityPredictor::new(&entries, utc, Configuration::default());

        let morning = utc.with_ymd_and_hms(2026, 3, 2, 9, 0, 0).single().unwrap();
        let evening = utc.with_ymd_and_hms(2026, 3, 2, 18, 0, 0).single().unwrap();

        assert_eq!(
            predictor
                .predictions(morning.timestamp_millis(), Some(0), 3)
                .first(),
            Some(&1)
        );
        assert_eq!(
            predictor
                .predictions(evening.timestamp_millis(), Some(0), 3)
                .first(),
            Some(&2)
        );
    }

    #[test]
    fn limit_of_zero_returns_nothing() {
        let predictor =
            ActivityPredictor::new(&make_entries(&[0, 1, 2]), utc(), Configuration::default());
        assert!(predictor.predictions(1_700_000_000_000, None, 0).is_empty());
    }

    #[test]
    fn recency_decay_prefers_recent_routine() {
        let mut states: Vec<usize> = Vec::new();
        for _ in 0..60 {
            states.extend_from_slice(&[6, 4]);
        }
        for _ in 0..60 {
            states.extend_from_slice(&[6, 5]);
        }
        let entries = make_entries(&states);
        let last = entries.last().unwrap().start_timestamp + 60_000;

        let decaying = ActivityPredictor::new(&entries, utc(), Configuration::default());
        assert_eq!(decaying.predictions(last, Some(6), 3).first(), Some(&5));

        let stationary = ActivityPredictor::new(
            &entries,
            utc(),
            Configuration {
                decay: 1.0,
                ..Configuration::default()
            },
        );
        let ranked = stationary.predictions(last, Some(6), 3);
        assert!(ranked.contains(&4) && ranked.contains(&5));
    }

    #[test]
    fn backs_off_for_unseen_context() {
        let utc = utc();
        let mut entries = Vec::new();
        for day in 0..30i64 {
            let base = utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).single().unwrap()
                + chrono::Duration::days(day);
            entries.push(TrainingEntry {
                state_id: 6,
                start_timestamp: base.timestamp_millis(),
            });
            entries.push(TrainingEntry {
                state_id: 0,
                start_timestamp: (base + chrono::Duration::minutes(30)).timestamp_millis(),
            });
        }

        let predictor = ActivityPredictor::new(&entries, utc, Configuration::default());
        let never_seen = utc.with_ymd_and_hms(2026, 3, 4, 3, 0, 0).single().unwrap();

        assert_eq!(
            predictor
                .predictions(never_seen.timestamp_millis(), Some(6), 3)
                .first(),
            Some(&0)
        );
    }

    struct Xorshift(u64);

    impl Xorshift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    /// Generates a log with a weekday routine, a different weekend routine, and a
    /// tenth of the entries replaced by uniform noise.
    fn synthetic_log(events: usize) -> Vec<TrainingEntry> {
        let utc = utc();
        let weekday_routine = [7usize, 6, 0, 6, 0, 6, 5, 6, 4];
        let weekend_routine = [7usize, 6, 4, 6, 5, 4, 6, 11, 6];
        let slots = weekday_routine.len();
        let mut rng = Xorshift(0x2545_F491_4F6C_DD1D);
        let mut entries = Vec::with_capacity(events);
        let start = utc.with_ymd_and_hms(2026, 1, 1, 6, 0, 0).single().unwrap();

        for index in 0..events {
            let day = (index / slots) as i64;
            let slot = index % slots;
            let at = start + chrono::Duration::days(day) + chrono::Duration::hours(slot as i64 * 2);
            let routine = if matches!(at.weekday().num_days_from_sunday(), 0 | 6) {
                weekend_routine
            } else {
                weekday_routine
            };
            let state_id = if rng.next() % 10 == 0 {
                (rng.next() % STATE_COUNT as u64) as usize
            } else {
                routine[slot]
            };
            entries.push(TrainingEntry {
                state_id,
                start_timestamp: at.timestamp_millis(),
            });
        }

        entries
    }

    /// Walk-forward accuracy: every entry is predicted using only the entries
    /// before it. Guards against regressions in the scoring core.
    #[test]
    fn accuracy_regression() {
        let entries = synthetic_log(2_000);
        let mut predictor = ActivityPredictor::new(&[], utc(), Configuration::default());
        let mut top1 = 0usize;
        let mut top3 = 0usize;
        let mut top5 = 0usize;

        for (i, entry) in entries.iter().enumerate() {
            let current = if i == 0 {
                None
            } else {
                Some(entries[i - 1].state_id)
            };
            let ranked = predictor.predictions(entry.start_timestamp, current, 5);
            if let Some(rank) = ranked.iter().position(|&s| s == entry.state_id) {
                top1 += usize::from(rank == 0);
                top3 += usize::from(rank < 3);
                top5 += usize::from(rank < 5);
            }
            predictor.train(entry.state_id, entry.start_timestamp);
        }

        let total = entries.len();
        let pct = |k: usize| k as f64 / total as f64 * 100.0;
        assert!(
            pct(top1) > 78.0 && pct(top3) > 85.0 && pct(top5) > 89.0,
            "accuracy regressed: top1={:.2}% top3={:.2}% top5={:.2}%",
            pct(top1),
            pct(top3),
            pct(top5)
        );
    }
}
