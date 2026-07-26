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

//! Walk-forward accuracy harness for [`predictor`].
//!
//! Every entry is predicted using only the entries before it, which is what the
//! deployed `/api/suggest` would have produced at that moment — training on the
//! whole log and then scoring it would measure memorisation instead.
//!
//! ```text
//! cargo run --release --example eval -- <export.json> [tz_offset_minutes]
//! ```
//!
//! Takes a file in `/api/export` format. Set `DECAY_SWEEP=1` to compare decay
//! rates instead of printing the full breakdown.

#[path = "../src/constants.rs"]
#[allow(dead_code)]
mod constants;
#[path = "../src/predictor.rs"]
mod predictor;

use std::collections::HashMap;
use std::env;

use chrono::{DateTime, Datelike, FixedOffset, TimeZone, Timelike};
use serde::Deserialize;

use constants::{ALL_STATES_DETAILS, STATE_COUNT};
use predictor::{ActivityPredictor, Configuration, TrainingEntry};

const LIMIT: usize = 5;

#[derive(Deserialize)]
struct ExportEntry {
    new_state: u8,
    start_timestamp: i64,
}

#[derive(Deserialize)]
struct Export {
    entries: Vec<ExportEntry>,
}

#[derive(Default, Clone)]
struct Score {
    n: usize,
    top1: usize,
    top3: usize,
    top5: usize,
    reciprocal_rank: f64,
}

impl Score {
    fn add(&mut self, rank: Option<usize>) {
        self.n += 1;
        if let Some(rank) = rank {
            self.top1 += usize::from(rank == 0);
            self.top3 += usize::from(rank < 3);
            self.top5 += usize::from(rank < 5);
            self.reciprocal_rank += 1.0 / (rank as f64 + 1.0);
        }
    }

    fn line(&self, label: &str) -> String {
        let pct = |k: usize| {
            if self.n == 0 {
                0.0
            } else {
                k as f64 / self.n as f64 * 100.0
            }
        };
        format!(
            "{label:<36} n={:<5} top1={:>6.2}%  top3={:>6.2}%  top5={:>6.2}%  MRR={:.3}",
            self.n,
            pct(self.top1),
            pct(self.top3),
            pct(self.top5),
            if self.n == 0 {
                0.0
            } else {
                self.reciprocal_rank / self.n as f64
            }
        )
    }
}

#[derive(Default)]
struct FrequencyTable {
    rows: HashMap<u64, [u32; STATE_COUNT]>,
}

impl FrequencyTable {
    fn observe(&mut self, key: u64, state_id: usize) {
        self.rows.entry(key).or_insert([0; STATE_COUNT])[state_id] += 1;
    }

    fn ranked(&self, key: u64, exclude: Option<usize>) -> Vec<usize> {
        let Some(row) = self.rows.get(&key) else {
            return Vec::new();
        };
        let mut ids: Vec<usize> = (0..STATE_COUNT)
            .filter(|id| row[*id] > 0 && Some(*id) != exclude)
            .collect();
        ids.sort_by(|&a, &b| row[b].cmp(&row[a]).then(a.cmp(&b)));
        ids
    }
}

fn extend_unique(out: &mut Vec<usize>, from: &[usize], exclude: Option<usize>) {
    for &state_id in from {
        if out.len() >= LIMIT || Some(state_id) == exclude || out.contains(&state_id) {
            continue;
        }
        out.push(state_id);
    }
}

fn rank_of(predictions: &[usize], actual: usize) -> Option<usize> {
    predictions.iter().position(|&id| id == actual)
}

fn load(path: &str) -> Vec<TrainingEntry> {
    let raw = std::fs::read_to_string(path).expect("cannot read export file");
    let export: Export = serde_json::from_str(&raw).expect("cannot parse export file");
    let mut entries: Vec<TrainingEntry> = export
        .entries
        .into_iter()
        .filter(|entry| (entry.new_state as usize) < STATE_COUNT)
        .map(|entry| TrainingEntry {
            state_id: entry.new_state as usize,
            start_timestamp: entry.start_timestamp,
        })
        .collect();
    entries.sort_by_key(|entry| entry.start_timestamp);
    entries
}

fn walk_forward(entries: &[TrainingEntry], offset: FixedOffset, configuration: Configuration) -> Score {
    let mut predictor = ActivityPredictor::new(&[], offset, configuration);
    let mut score = Score::default();
    for (i, entry) in entries.iter().enumerate() {
        let current = i.checked_sub(1).map(|prev| entries[prev].state_id);
        let ranked = predictor.predictions(entry.start_timestamp, current, LIMIT);
        score.add(rank_of(&ranked, entry.state_id));
        predictor.train(entry.state_id, entry.start_timestamp);
    }
    score
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).expect("usage: eval <export.json> [tz_offset_minutes]");
    let tz_minutes: i32 = args.get(2).map_or(0, |v| v.parse().expect("bad tz offset"));
    let offset = FixedOffset::east_opt(tz_minutes * 60).expect("bad tz offset");

    let entries = load(path);
    println!(
        "== walk-forward evaluation: {} entries, tz_offset={} min, limit={} ==\n",
        entries.len(),
        tz_minutes,
        LIMIT
    );

    if env::var("DECAY_SWEEP").is_ok() {
        for decay in [1.0, 0.999, 0.998, 0.995, 0.99, 0.985, 0.98, 0.97] {
            let score = walk_forward(
                &entries,
                offset,
                Configuration {
                    decay,
                    ..Configuration::default()
                },
            );
            println!("{}", score.line(&format!("  decay={decay}")));
        }
        return;
    }

    let mut predictor = ActivityPredictor::new(&[], offset, Configuration::default());
    let mut overall = Score::default();
    let mut by_month: Vec<(String, Score)> = Vec::new();
    let mut by_current: Vec<Score> = vec![Score::default(); STATE_COUNT];

    let mut static_top = Score::default();
    let mut most_frequent = Score::default();
    let mut markov = Score::default();
    let mut hour_current = Score::default();
    let mut daypart_current = Score::default();

    let mut global_counts = FrequencyTable::default();
    let mut markov_counts = FrequencyTable::default();
    let mut hour_counts = FrequencyTable::default();
    let mut daypart_counts = FrequencyTable::default();

    for (i, entry) in entries.iter().enumerate() {
        let current = i.checked_sub(1).map(|prev| entries[prev].state_id);
        let actual = entry.state_id;
        let at = entry.start_timestamp;

        let local: DateTime<FixedOffset> = offset.timestamp_millis_opt(at).single().unwrap();
        let hour = u64::from(local.hour());
        let daypart = hour / 4;

        let ranked = predictor.predictions(at, current, LIMIT);
        if env::var("STEP").is_ok_and(|v| v.parse::<usize>() == Ok(i)) {
            println!("STEP {i}: at={at} current={current:?} predictions={ranked:?}");
        }
        let rank = rank_of(&ranked, actual);
        overall.add(rank);
        if let Some(current) = current {
            by_current[current].add(rank);
        }

        let month = format!("{:04}-{:02}", local.year(), local.month());
        if by_month.last().map(|(m, _)| m.as_str()) != Some(month.as_str()) {
            by_month.push((month, Score::default()));
        }
        by_month.last_mut().unwrap().1.add(rank);

        let global = global_counts.ranked(0, current);

        static_top.add(rank_of(&[6, 5, 0, 7, 4], actual));

        let mut guess = Vec::new();
        extend_unique(&mut guess, &global, current);
        most_frequent.add(rank_of(&guess, actual));

        let mut guess = Vec::new();
        if let Some(current) = current {
            extend_unique(&mut guess, &markov_counts.ranked(current as u64, Some(current)), Some(current));
        }
        extend_unique(&mut guess, &global, current);
        markov.add(rank_of(&guess, actual));

        let mut guess = Vec::new();
        if let Some(current) = current {
            extend_unique(&mut guess, &hour_counts.ranked(hour * 16 + current as u64, Some(current)), Some(current));
            extend_unique(&mut guess, &markov_counts.ranked(current as u64, Some(current)), Some(current));
        }
        extend_unique(&mut guess, &global, current);
        hour_current.add(rank_of(&guess, actual));

        let mut guess = Vec::new();
        if let Some(current) = current {
            extend_unique(&mut guess, &daypart_counts.ranked(daypart * 16 + current as u64, Some(current)), Some(current));
            extend_unique(&mut guess, &markov_counts.ranked(current as u64, Some(current)), Some(current));
        }
        extend_unique(&mut guess, &global, current);
        daypart_current.add(rank_of(&guess, actual));

        global_counts.observe(0, actual);
        if let Some(current) = current {
            markov_counts.observe(current as u64, actual);
            hour_counts.observe(hour * 16 + current as u64, actual);
            daypart_counts.observe(daypart * 16 + current as u64, actual);
        }
        predictor.train(actual, at);
    }

    println!("{}", overall.line("interpolated predictor"));
    println!("\n--- online baselines, identical loop ---");
    println!("{}", static_top.line("static [Maint,Expl,Study,Sleep,Ent]"));
    println!("{}", most_frequent.line("most-frequent-so-far"));
    println!("{}", markov.line("1st-order Markov"));
    println!("{}", hour_current.line("hour x current"));
    println!("{}", daypart_current.line("daypart x current"));

    println!("\n--- accuracy over time ---");
    for (month, score) in &by_month {
        println!("{}", score.line(&format!("  {month}")));
    }

    println!("\n--- accuracy by current activity ---");
    let mut order: Vec<usize> = (0..STATE_COUNT).collect();
    order.sort_by_key(|&id| std::cmp::Reverse(by_current[id].n));
    for id in order {
        if by_current[id].n > 0 {
            println!(
                "{}",
                by_current[id].line(&format!("  from {}", ALL_STATES_DETAILS[id].name))
            );
        }
    }
}
