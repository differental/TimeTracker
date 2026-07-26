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

//! Next-activity prediction using a TAGE-style (TAgged GEometric history length)
//! predictor, borrowed from branch prediction: a base bimodal-ish table indexed by
//! the immediate context, plus a series of tagged tables indexed by geometrically
//! increasing lengths of activity history. The longest matching tagged table
//! provides the prediction, with the second-longest acting as the alternate, and
//! per-entry usefulness counters deciding which of the two to trust.
//!
//! This is a port of the iOS client's `ActivityPredictor`, moved server-side so
//! the client doesn't have to download its whole history to get suggestions. The
//! hashing, counter widths and allocation policy are kept bit-for-bit identical to
//! the Swift implementation so both produce the same suggestions.

use chrono::{DateTime, Datelike, FixedOffset, TimeZone, Timelike};

use crate::constants::STATE_COUNT;

/// Tunables for the predictor. The defaults mirror the iOS client's defaults.
#[derive(Clone, Debug)]
pub struct Configuration {
    /// Geometrically increasing history lengths, one tagged table per entry.
    pub history_lengths: Vec<usize>,
    pub base_table_size: usize,
    pub tagged_table_size: usize,
    /// Every N training steps, usefulness counters are halved so stale entries
    /// become replaceable again.
    pub usefulness_aging_interval: u64,
    /// Only the most recent N entries are trained on.
    pub maximum_training_entries: usize,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            history_lengths: vec![1, 2, 4, 8, 16, 32],
            base_table_size: 512,
            tagged_table_size: 512,
            usefulness_aging_interval: 256,
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
    weekday: u32,
    hour: u32,
    current_state_id: Option<usize>,
    is_tracking: bool,
    history: Vec<usize>,
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    state_id: usize,
    counter: u8,
    last_seen: i64,
}

/// A tiny (at most three) set of competing next-activity candidates, each with a
/// saturating 3-bit confidence counter.
#[derive(Clone, Debug, Default)]
struct CandidateSet {
    values: Vec<Candidate>,
}

impl CandidateSet {
    fn ordered(&self) -> Vec<Candidate> {
        let mut ordered = self.values.clone();
        ordered.sort_by(|a, b| {
            b.counter
                .cmp(&a.counter)
                .then(b.last_seen.cmp(&a.last_seen))
                .then(a.state_id.cmp(&b.state_id))
        });
        ordered
    }

    fn top_state_id(&self) -> Option<usize> {
        self.ordered().first().map(|c| c.state_id)
    }

    fn is_confident(&self) -> bool {
        let ranked = self.ordered();
        let Some(first) = ranked.first() else {
            return false;
        };
        let second = ranked.get(1).map_or(0, |c| c.counter);
        first.counter >= 4 && i16::from(first.counter) - i16::from(second) >= 2
    }

    fn observe(&mut self, state_id: usize, sequence: i64) {
        if let Some(existing) = self.values.iter_mut().find(|c| c.state_id == state_id) {
            existing.counter = existing.counter.saturating_add(1).min(7);
            existing.last_seen = sequence;
            return;
        }

        if self.values.len() < 3 {
            self.values.push(Candidate {
                state_id,
                counter: 1,
                last_seen: sequence,
            });
            return;
        }

        // Set is full of other states: decay everyone, evict whoever hits zero,
        // and only then let the new state in.
        for candidate in self.values.iter_mut().filter(|c| c.counter > 0) {
            candidate.counter -= 1;
        }
        self.values.retain(|c| c.counter != 0);
        if self.values.len() < 3 {
            self.values.push(Candidate {
                state_id,
                counter: 1,
                last_seen: sequence,
            });
        }
    }
}

#[derive(Clone, Debug)]
struct TaggedEntry {
    tag: u16,
    candidates: CandidateSet,
    usefulness: u8,
}

struct Match {
    table: usize,
    index: usize,
}

pub struct ActivityPredictor {
    configuration: Configuration,
    offset: FixedOffset,
    base_table: Vec<CandidateSet>,
    tagged_tables: Vec<Vec<Option<TaggedEntry>>>,
    global_counts: [u64; STATE_COUNT],
    recency_scores: [f64; STATE_COUNT],
    global_last_seen: [i64; STATE_COUNT],
    recent_history: Vec<usize>,
    training_sequence: i64,
}

impl ActivityPredictor {
    /// Trains a predictor on `entries`. Weekday/hour context is derived in the
    /// `offset` timezone, so the caller must pass the user's local UTC offset for
    /// time-of-day patterns to line up.
    pub fn new(
        entries: &[TrainingEntry],
        offset: FixedOffset,
        configuration: Configuration,
    ) -> Self {
        assert!(configuration.base_table_size > 0);
        assert!(configuration.tagged_table_size > 0);
        assert!(configuration.usefulness_aging_interval > 0);

        let base_table = vec![CandidateSet::default(); configuration.base_table_size];
        let tagged_tables = configuration
            .history_lengths
            .iter()
            .map(|_| vec![None; configuration.tagged_table_size])
            .collect();

        let mut predictor = Self {
            configuration,
            offset,
            base_table,
            tagged_tables,
            global_counts: [0; STATE_COUNT],
            recency_scores: [0.0; STATE_COUNT],
            global_last_seen: [-1; STATE_COUNT],
            recent_history: Vec::new(),
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
    /// back to globally recent/frequent activities, then canonical order).
    pub fn predictions(
        &self,
        at: i64,
        current_state_id: Option<usize>,
        limit: usize,
    ) -> Vec<usize> {
        if limit == 0 {
            return Vec::new();
        }

        let context = self.make_context(at, current_state_id, self.recent_history.clone());
        let matches = self.matching_entries(&context);
        let base = &self.base_table[self.base_index(&context)];
        let provider = matches.last();
        let alternate = if matches.len() >= 2 {
            matches.get(matches.len() - 2)
        } else {
            None
        };

        let entry_at = |m: &Match| self.tagged_tables[m.table][m.index].as_ref().unwrap();

        let mut candidate_sets: Vec<&CandidateSet> = Vec::new();
        if let Some(provider) = provider {
            let provider_entry = entry_at(provider);
            // Trust the longest match when it has proven useful or is confident;
            // otherwise let the shorter-history alternate go first.
            if provider_entry.usefulness >= 2 || provider_entry.candidates.is_confident() {
                candidate_sets.push(&provider_entry.candidates);
                if let Some(alternate) = alternate {
                    candidate_sets.push(&entry_at(alternate).candidates);
                }
            } else {
                if let Some(alternate) = alternate {
                    candidate_sets.push(&entry_at(alternate).candidates);
                }
                candidate_sets.push(&provider_entry.candidates);
            }
        }
        candidate_sets.push(base);

        let mut state_ids: Vec<usize> = Vec::with_capacity(limit);
        for set in candidate_sets {
            for candidate in set.ordered() {
                push_unique(&mut state_ids, candidate.state_id, current_state_id, limit);
                if state_ids.len() == limit {
                    break;
                }
            }
            if state_ids.len() == limit {
                break;
            }
        }

        if state_ids.len() < limit {
            let mut fallback: Vec<usize> = (0..STATE_COUNT).collect();
            fallback.sort_by(|&a, &b| {
                self.recency_scores[b]
                    .partial_cmp(&self.recency_scores[a])
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(self.global_counts[b].cmp(&self.global_counts[a]))
                    .then(self.global_last_seen[b].cmp(&self.global_last_seen[a]))
                    .then(a.cmp(&b))
            });
            for state_id in fallback {
                push_unique(&mut state_ids, state_id, current_state_id, limit);
                if state_ids.len() == limit {
                    break;
                }
            }
        }

        state_ids
    }

    fn train(&mut self, target_state_id: usize, at: i64) {
        let context = self.make_context(
            at,
            self.recent_history.last().copied(),
            self.recent_history.clone(),
        );
        let matches = self.matching_entries(&context);
        let base = self.base_index(&context);
        let base_top = self.base_table[base].top_state_id();

        let provider = matches.last().map(|m| (m.table, m.index));
        let alternate_top = if matches.len() >= 2 {
            let m = &matches[matches.len() - 2];
            self.tagged_tables[m.table][m.index]
                .as_ref()
                .and_then(|e| e.candidates.top_state_id())
        } else {
            base_top
        };
        let provider_top = match provider {
            Some((table, index)) => self.tagged_tables[table][index]
                .as_ref()
                .and_then(|e| e.candidates.top_state_id()),
            None => base_top,
        };

        self.base_table[base].observe(target_state_id, self.training_sequence);

        if let Some((table, index)) = provider {
            let sequence = self.training_sequence;
            let entry = self.tagged_tables[table][index].as_mut().unwrap();
            entry.candidates.observe(target_state_id, sequence);
            // Usefulness rises only when the provider was right where the
            // alternate was wrong, and falls in the mirror-image case.
            let provider_correct = provider_top == Some(target_state_id);
            let alternate_correct = alternate_top == Some(target_state_id);
            if provider_correct && !alternate_correct {
                entry.usefulness = (entry.usefulness + 1).min(3);
            } else if !provider_correct && alternate_correct {
                entry.usefulness = entry.usefulness.saturating_sub(1);
            }
        }

        if provider_top != Some(target_state_id) {
            let after = provider.map_or(-1, |(table, _)| table as isize);
            self.allocate(target_state_id, after, &context);
        }

        for score in self.recency_scores.iter_mut() {
            *score *= 0.95;
        }
        self.global_counts[target_state_id] += 1;
        self.recency_scores[target_state_id] += 1.0;
        self.global_last_seen[target_state_id] = self.training_sequence;
        self.recent_history.push(target_state_id);
        let max_history = self
            .configuration
            .history_lengths
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        if self.recent_history.len() > max_history {
            let excess = self.recent_history.len() - max_history;
            self.recent_history.drain(0..excess);
        }

        self.training_sequence += 1;
        if self
            .training_sequence
            .rem_euclid(self.configuration.usefulness_aging_interval as i64)
            == 0
        {
            self.age_usefulness();
        }
    }

    /// On a misprediction, claim up to two entries in tables longer than the one
    /// that provided the (wrong) prediction, stealing only from entries whose
    /// usefulness has already decayed to zero.
    fn allocate(&mut self, target_state_id: usize, after_table: isize, context: &Context) {
        let mut allocations = 0;
        for table in 0..self.tagged_tables.len() {
            if (table as isize) <= after_table {
                continue;
            }
            if context.history.len() < self.configuration.history_lengths[table] {
                continue;
            }
            let (index, tag) = self.tagged_location(context, table);
            if let Some(existing) = self.tagged_tables[table][index].as_mut() {
                if existing.tag == tag {
                    continue;
                }
                if existing.usefulness > 0 {
                    existing.usefulness -= 1;
                    continue;
                }
            }

            let mut candidates = CandidateSet::default();
            candidates.observe(target_state_id, self.training_sequence);
            self.tagged_tables[table][index] = Some(TaggedEntry {
                tag,
                candidates,
                usefulness: 0,
            });
            allocations += 1;
            if allocations == 2 {
                return;
            }
        }
    }

    fn age_usefulness(&mut self) {
        for table in self.tagged_tables.iter_mut() {
            for entry in table.iter_mut().flatten() {
                entry.usefulness >>= 1;
            }
        }
    }

    fn make_context(
        &self,
        at: i64,
        current_state_id: Option<usize>,
        history: Vec<usize>,
    ) -> Context {
        let local: DateTime<FixedOffset> = self
            .offset
            .timestamp_millis_opt(at)
            .single()
            .unwrap_or_else(|| self.offset.timestamp_nanos(0));
        Context {
            // Matches Foundation's `Calendar.component(.weekday:)`: 1 = Sunday.
            weekday: local.weekday().num_days_from_sunday() + 1,
            hour: local.hour(),
            current_state_id,
            is_tracking: current_state_id.is_some(),
            history,
        }
    }

    fn matching_entries(&self, context: &Context) -> Vec<Match> {
        let mut result = Vec::new();
        for table in 0..self.tagged_tables.len() {
            if context.history.len() < self.configuration.history_lengths[table] {
                continue;
            }
            let (index, tag) = self.tagged_location(context, table);
            match &self.tagged_tables[table][index] {
                Some(entry) if entry.tag == tag => result.push(Match { table, index }),
                _ => continue,
            }
        }
        result
    }

    fn base_index(&self, context: &Context) -> usize {
        (self.context_hash(context, 0, 0x243f_6a88) % self.base_table.len() as u64) as usize
    }

    fn tagged_location(&self, context: &Context, table: usize) -> (usize, u16) {
        let length = self.configuration.history_lengths[table];
        let index_hash = self.context_hash(
            context,
            length,
            (table as u64 + 1).wrapping_mul(0x9e37_79b9),
        );
        let tag_hash = self.context_hash(
            context,
            length,
            (table as u64 + 1).wrapping_mul(0x85eb_ca6b),
        );
        (
            (index_hash % self.configuration.tagged_table_size as u64) as usize,
            (tag_hash ^ (tag_hash >> 32)) as u16,
        )
    }

    fn context_hash(&self, context: &Context, history_length: usize, salt: u64) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64 ^ salt;
        hash_value(u64::from(context.weekday), &mut hash);
        hash_value(u64::from(context.hour), &mut hash);
        let current = context.current_state_id.map_or(-1i64, |id| id as i64) + 1;
        hash_value(current as u64, &mut hash);
        hash_value(u64::from(context.is_tracking), &mut hash);
        if history_length > 0 {
            let skip = context.history.len().saturating_sub(history_length);
            for &state_id in &context.history[skip..] {
                hash_value(state_id as u64 + 1, &mut hash);
            }
        }
        avalanche(hash)
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

fn hash_value(value: u64, hash: &mut u64) {
    *hash ^= value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    *hash ^= *hash >> 32;
}

fn avalanche(value: u64) -> u64 {
    let mut result = value;
    result ^= result >> 30;
    result = result.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    result ^= result >> 27;
    result = result.wrapping_mul(0x94d0_49bb_1331_11eb);
    result ^= result >> 31;
    result
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
        let predictor = ActivityPredictor::new(
            &entries,
            utc(),
            Configuration {
                history_lengths: vec![1, 2, 4, 8],
                base_table_size: 1_024,
                tagged_table_size: 1_024,
                ..Configuration::default()
            },
        );
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
                history_lengths: vec![1, 2],
                base_table_size: 128,
                tagged_table_size: 128,
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

        let predictor = ActivityPredictor::new(
            &entries,
            utc,
            Configuration {
                history_lengths: vec![1, 2, 4],
                base_table_size: 4_096,
                tagged_table_size: 1_024,
                ..Configuration::default()
            },
        );

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
}

#[cfg(test)]
mod parity {
    //! Cross-checks this port against the iOS `ActivityPredictor`. The golden
    //! output below was produced by compiling the Swift `ActivityPredictor`
    //! verbatim against the same pseudo-random log (same xorshift seed, same
    //! draw order, UTC calendar) and printing its predictions. Any divergence
    //! here means the two implementations have drifted apart.
    use super::*;

    const SWIFT_REFERENCE: &str = "\
0 cur=- -> [13, 5, 2, 3, 7]
1 cur=13 -> [0, 1, 9, 2, 3]
2 cur=11 -> [14, 5, 9, 2, 3]
3 cur=5 -> [13, 1, 2, 3, 7]
4 cur=- -> [9, 4, 2, 3, 7]
5 cur=12 -> [2, 3, 7, 14, 13]
6 cur=14 -> [4, 2, 3, 7, 13]
7 cur=13 -> [1, 7, 10, 2, 3]
8 cur=- -> [2, 3, 7, 14, 13]
9 cur=11 -> [4, 2, 3, 7, 14]
10 cur=8 -> [0, 1, 9, 2, 3]
11 cur=5 -> [7, 12, 2, 3, 14]
12 cur=- -> [12, 2, 3, 7, 14]
13 cur=5 -> [1, 4, 2, 3, 7]
14 cur=12 -> [11, 8, 1, 2, 3]
15 cur=6 -> [3, 1, 2, 7, 14]
16 cur=- -> [7, 12, 1, 2, 3]
17 cur=11 -> [3, 10, 2, 7, 14]
18 cur=2 -> [4, 3, 9, 7, 14]
19 cur=2 -> [6, 11, 3, 7, 14]
20 cur=- -> [13, 2, 3, 7, 14]
21 cur=10 -> [0, 11, 2, 3, 7]
22 cur=3 -> [7, 5, 2, 14, 13]
23 cur=11 -> [7, 8, 2, 3, 14]
24 cur=- -> [9, 2, 3, 7, 14]
25 cur=0 -> [3, 5, 2, 7, 14]
26 cur=1 -> [6, 5, 2, 3, 7]
27 cur=13 -> [4, 11, 0, 2, 3]
28 cur=- -> [4, 3, 7, 2, 14]
29 cur=4 -> [3, 2, 7, 14, 13]
30 cur=4 -> [2, 9, 3, 7, 14]
31 cur=8 -> [3, 10, 2, 7, 14]
32 cur=- -> [7, 9, 5, 2, 3]
33 cur=12 -> [10, 5, 7, 2, 3]
34 cur=6 -> [3, 13, 1, 2, 7]
35 cur=1 -> [7, 8, 2, 3, 14]
36 cur=- -> [13, 2, 3, 7, 14]
37 cur=11 -> [8, 0, 9, 2, 3]
38 cur=10 -> [2, 3, 7, 14, 13]
39 cur=9 -> [6, 10, 2, 3, 7]";

    struct Xorshift(u64);
    impl Xorshift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    #[test]
    fn matches_swift_reference_output() {
        let mut rng = Xorshift(0x2545_F491_4F6C_DD1D);
        let mut entries = Vec::new();
        let mut t: i64 = 1_700_000_000_000;
        for _ in 0..3_000 {
            let state_id = (rng.next() % 15) as usize;
            t += (rng.next() % 7_200_000) as i64;
            entries.push(TrainingEntry {
                state_id,
                start_timestamp: t,
            });
        }

        let predictor = ActivityPredictor::new(
            &entries,
            FixedOffset::east_opt(0).unwrap(),
            Configuration::default(),
        );

        let mut out = Vec::new();
        for step in 0..40i64 {
            let at = t + step * 3_600_000;
            let current = if step % 4 == 0 {
                None
            } else {
                Some((rng.next() % 15) as usize)
            };
            let p = predictor.predictions(at, current, 5);
            out.push(format!(
                "{step} cur={} -> {p:?}",
                current.map_or("-".to_string(), |c| c.to_string())
            ));
        }
        assert_eq!(out.join("\n"), SWIFT_REFERENCE);
    }
}
