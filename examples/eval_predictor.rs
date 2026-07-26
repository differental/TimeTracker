#![allow(dead_code)]

use std::collections::HashMap;
use std::env;

use chrono::{DateTime, Datelike, FixedOffset, TimeZone, Timelike};

#[path = "../src/constants.rs"]
mod constants;
#[path = "../src/predictor.rs"]
mod predictor;

use constants::{ALL_STATES_DETAILS, STATE_COUNT};
use predictor::{ActivityPredictor, Configuration, Ranking, TrainingEntry, WeekdayMode};

#[derive(Default, Clone)]
struct Metrics {
    scored: usize,
    skipped_self: usize,
    top1: usize,
    top3: usize,
    reciprocal_rank: f64,
}

impl Metrics {
    fn record(&mut self, truth: usize, predictions: &[usize], current: Option<usize>) {
        if current == Some(truth) {
            self.skipped_self += 1;
            return;
        }
        self.scored += 1;
        if predictions.first() == Some(&truth) {
            self.top1 += 1;
        }
        if let Some(rank) = predictions.iter().position(|&s| s == truth) {
            if rank < 3 {
                self.top3 += 1;
            }
            self.reciprocal_rank += 1.0 / (rank as f64 + 1.0);
        }
    }

    fn top1_rate(&self) -> f64 {
        ratio(self.top1, self.scored)
    }

    fn top3_rate(&self) -> f64 {
        ratio(self.top3, self.scored)
    }

    fn mrr(&self) -> f64 {
        if self.scored == 0 {
            0.0
        } else {
            self.reciprocal_rank / self.scored as f64
        }
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        100.0 * numerator as f64 / denominator as f64
    }
}

fn load_json(path: &str) -> Vec<TrainingEntry> {
    let text = std::fs::read_to_string(path).expect("read export");
    let document: serde_json::Value = serde_json::from_str(&text).expect("parse export");
    let rows = document["entries"].as_array().expect("entries array");
    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        let state_id = row["new_state"].as_u64().expect("new_state") as usize;
        let start_timestamp = row["start_timestamp"].as_i64().expect("start_timestamp");
        if state_id >= STATE_COUNT {
            continue;
        }
        if chrono::Utc.timestamp_millis_opt(start_timestamp).single().is_none() {
            continue;
        }
        entries.push(TrainingEntry {
            state_id,
            start_timestamp,
        });
    }
    entries.sort_by_key(|entry| entry.start_timestamp);
    entries
}

fn load_entries(path: &str) -> Vec<TrainingEntry> {
    let db = sled::open(path).expect("open sled db");
    let events = db.open_tree("events").expect("events tree");
    let meta = db.open_tree("meta").expect("meta tree");

    let length = match meta.get(b"len").expect("read len") {
        Some(value) => {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&value[0..8]);
            u64::from_ne_bytes(bytes)
        }
        None => 0,
    };

    let mut entries = Vec::with_capacity(length as usize);
    for i in 0..length {
        let Ok(Some(bytes)) = events.get(i.to_ne_bytes()) else {
            continue;
        };
        if bytes.len() < 9 {
            continue;
        }
        let state_id = bytes[0] as usize;
        let mut time_bytes = [0u8; 8];
        time_bytes.copy_from_slice(&bytes[1..9]);
        let start_timestamp = i64::from_ne_bytes(time_bytes);
        if state_id >= STATE_COUNT {
            continue;
        }
        if chrono::Utc.timestamp_millis_opt(start_timestamp).single().is_none() {
            continue;
        }
        entries.push(TrainingEntry {
            state_id,
            start_timestamp,
        });
    }

    entries.sort_by_key(|entry| entry.start_timestamp);
    entries
}

fn describe(entries: &[TrainingEntry], offset: FixedOffset) {
    println!("== dataset ==");
    println!("entries: {}", entries.len());
    if entries.is_empty() {
        return;
    }
    let first = local(entries[0].start_timestamp, offset);
    let last = local(entries[entries.len() - 1].start_timestamp, offset);
    println!("range:   {first} .. {last}");

    let mut counts = [0usize; STATE_COUNT];
    let mut self_transitions = 0usize;
    let mut gaps = Vec::with_capacity(entries.len());
    for (i, entry) in entries.iter().enumerate() {
        counts[entry.state_id] += 1;
        if i > 0 {
            if entries[i - 1].state_id == entry.state_id {
                self_transitions += 1;
            }
            gaps.push(entry.start_timestamp - entries[i - 1].start_timestamp);
        }
    }
    gaps.sort_unstable();
    let median = gaps.get(gaps.len() / 2).copied().unwrap_or(0);
    println!(
        "median gap between entries: {:.1} min",
        median as f64 / 60_000.0
    );
    println!(
        "self-transitions (next == current): {} ({:.1}%)",
        self_transitions,
        ratio(self_transitions, entries.len().saturating_sub(1))
    );

    let mut ranked: Vec<usize> = (0..STATE_COUNT).collect();
    ranked.sort_by_key(|&s| std::cmp::Reverse(counts[s]));
    println!("state distribution:");
    for state_id in ranked {
        if counts[state_id] == 0 {
            continue;
        }
        println!(
            "  {:>2} {:<12} {:>5}  {:>5.1}%",
            state_id,
            ALL_STATES_DETAILS[state_id].name,
            counts[state_id],
            ratio(counts[state_id], entries.len())
        );
    }
    println!();
}

fn local(at: i64, offset: FixedOffset) -> String {
    let time: DateTime<FixedOffset> = offset.timestamp_millis_opt(at).single().unwrap();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        time.year(),
        time.month(),
        time.day(),
        time.hour(),
        time.minute()
    )
}

fn eval_tage(
    entries: &[TrainingEntry],
    offset: FixedOffset,
    configuration: Configuration,
    warmup: usize,
    limit: usize,
) -> Metrics {
    eval_tage_window(entries, offset, configuration, warmup, entries.len(), limit)
}

fn eval_tage_window(
    entries: &[TrainingEntry],
    offset: FixedOffset,
    configuration: Configuration,
    score_from: usize,
    score_to: usize,
    limit: usize,
) -> Metrics {
    let mut predictor = ActivityPredictor::new(&[], offset, configuration);
    let mut metrics = Metrics::default();
    for (i, entry) in entries.iter().enumerate() {
        if i >= score_from && i < score_to {
            let current = entries[i - 1].state_id;
            let predictions = predictor.predictions(entry.start_timestamp, Some(current), limit);
            metrics.record(entry.state_id, &predictions, Some(current));
        }
        predictor.train(entry.state_id, entry.start_timestamp);
    }
    metrics
}

fn eval_baseline<F>(entries: &[TrainingEntry], warmup: usize, limit: usize, mut rank: F) -> Metrics
where
    F: FnMut(&[TrainingEntry], usize, Option<usize>) -> Vec<usize>,
{
    let mut metrics = Metrics::default();
    for i in warmup..entries.len() {
        let current = entries[i - 1].state_id;
        let mut ranked = rank(entries, i, Some(current));
        ranked.retain(|&s| s != current);
        ranked.truncate(limit);
        metrics.record(entries[i].state_id, &ranked, Some(current));
    }
    metrics
}

fn rank_global_frequency(entries: &[TrainingEntry], upto: usize, _current: Option<usize>) -> Vec<usize> {
    let mut counts = [0usize; STATE_COUNT];
    for entry in &entries[..upto] {
        counts[entry.state_id] += 1;
    }
    let mut ranked: Vec<usize> = (0..STATE_COUNT).collect();
    ranked.sort_by_key(|&s| std::cmp::Reverse(counts[s]));
    ranked
}

fn rank_recency(entries: &[TrainingEntry], upto: usize, _current: Option<usize>) -> Vec<usize> {
    let mut last_seen = [usize::MAX; STATE_COUNT];
    for (i, entry) in entries[..upto].iter().enumerate() {
        last_seen[entry.state_id] = i;
    }
    let mut ranked: Vec<usize> = (0..STATE_COUNT).collect();
    ranked.sort_by_key(|&s| std::cmp::Reverse(last_seen[s].wrapping_add(1)));
    ranked
}

fn rank_markov(entries: &[TrainingEntry], upto: usize, current: Option<usize>) -> Vec<usize> {
    let Some(current) = current else {
        return rank_global_frequency(entries, upto, None);
    };
    let mut counts = [0usize; STATE_COUNT];
    let mut totals = [0usize; STATE_COUNT];
    for pair in entries[..upto].windows(2) {
        if pair[0].state_id == current {
            counts[pair[1].state_id] += 1;
        }
        totals[pair[1].state_id] += 1;
    }
    let mut ranked: Vec<usize> = (0..STATE_COUNT).collect();
    ranked.sort_by_key(|&s| std::cmp::Reverse((counts[s], totals[s])));
    ranked
}

fn rank_markov_hour(entries: &[TrainingEntry], upto: usize, current: Option<usize>) -> Vec<usize> {
    let Some(current) = current else {
        return rank_global_frequency(entries, upto, None);
    };
    let offset = FixedOffset::east_opt(0).unwrap();
    let target_hour = offset
        .timestamp_millis_opt(entries[upto].start_timestamp)
        .single()
        .map(|t| t.hour() / 4)
        .unwrap_or(0);
    let mut counts: HashMap<usize, usize> = HashMap::new();
    let mut fallback = [0usize; STATE_COUNT];
    for pair in entries[..upto].windows(2) {
        let hour = offset
            .timestamp_millis_opt(pair[1].start_timestamp)
            .single()
            .map(|t| t.hour() / 4)
            .unwrap_or(0);
        if pair[0].state_id == current && hour == target_hour {
            *counts.entry(pair[1].state_id).or_default() += 1;
        }
        if pair[0].state_id == current {
            fallback[pair[1].state_id] += 1;
        }
    }
    let mut ranked: Vec<usize> = (0..STATE_COUNT).collect();
    ranked.sort_by_key(|&s| {
        std::cmp::Reverse((counts.get(&s).copied().unwrap_or(0), fallback[s]))
    });
    ranked
}

fn report(label: &str, metrics: &Metrics) {
    println!(
        "{:<28} top1 {:>5.1}%   top3 {:>5.1}%   mrr {:.3}   n={}",
        label,
        metrics.top1_rate(),
        metrics.top3_rate(),
        metrics.mrr(),
        metrics.scored
    );
}

fn main() {
    let mut args = env::args().skip(1);
    let mut db_path = String::from("../timetracker.db");
    let mut json_path: Option<String> = None;
    let mut tz_minutes = 0i32;
    let mut warmup_fraction = 0.5f64;
    let mut mode = String::from("ablation");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => db_path = args.next().expect("--db needs a path"),
            "--json" => json_path = Some(args.next().expect("--json needs a path")),
            "--mode" => mode = args.next().expect("--mode needs a value"),
            "--tz" => tz_minutes = args.next().expect("--tz needs minutes").parse().unwrap(),
            "--warmup" => {
                warmup_fraction = args.next().expect("--warmup needs a fraction").parse().unwrap()
            }
            other => panic!("unknown argument {other}"),
        }
    }

    let offset = FixedOffset::east_opt(tz_minutes * 60).expect("valid tz offset");
    let entries = match &json_path {
        Some(path) => load_json(path),
        None => load_entries(&db_path),
    };
    describe(&entries, offset);

    let warmup = ((entries.len() as f64) * warmup_fraction) as usize;
    let limit = 3;
    println!("== accuracy on the last {} entries ==", entries.len() - warmup);

    report(
        "baseline: frequency",
        &eval_baseline(&entries, warmup, limit, rank_global_frequency),
    );
    report(
        "baseline: recency",
        &eval_baseline(&entries, warmup, limit, rank_recency),
    );
    report(
        "baseline: markov-1",
        &eval_baseline(&entries, warmup, limit, rank_markov),
    );
    report(
        "baseline: markov-1 x hour/4",
        &eval_baseline(&entries, warmup, limit, rank_markov_hour),
    );
    if mode == "grid" || mode == "stage2" {
        let selection_from = warmup;
        let selection_to = warmup + (entries.len() - warmup) / 2;
        let candidates = if mode == "grid" { grid() } else { stage_two() };
        let mut scored: Vec<(f64, f64, String, Configuration)> = candidates
            .into_iter()
            .map(|(label, configuration)| {
                let metrics = eval_tage_window(
                    &entries,
                    offset,
                    configuration.clone(),
                    selection_from,
                    selection_to,
                    limit,
                );
                (metrics.top3_rate(), metrics.top1_rate(), label, configuration)
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(b.1.partial_cmp(&a.1).unwrap()));

        println!();
        println!(
            "== grid: selection window {}..{}, held-out {}..{} ==",
            selection_from,
            selection_to,
            selection_to,
            entries.len()
        );
        let shown = if mode == "grid" { 15 } else { scored.len() };
        for (top3, top1, label, configuration) in scored.iter().take(shown) {
            let held_out = eval_tage_window(
                &entries,
                offset,
                configuration.clone(),
                selection_to,
                entries.len(),
                limit,
            );
            println!(
                "{label:<52} sel top3 {top3:>5.1}% top1 {top1:>5.1}%  |  held-out top3 {:>5.1}% top1 {:>5.1}%",
                held_out.top3_rate(),
                held_out.top1_rate()
            );
        }

        let legacy_held_out = eval_tage_window(
            &entries,
            offset,
            Configuration::legacy(),
            selection_to,
            entries.len(),
            limit,
        );
        println!(
            "{:<52} {:>28}  |  held-out top3 {:>5.1}% top1 {:>5.1}%",
            "legacy (PR#7)",
            "",
            legacy_held_out.top3_rate(),
            legacy_held_out.top1_rate()
        );
        return;
    }

    println!();
    println!("== ablation, one knob at a time on top of PR#7 ==");
    for (label, configuration) in ablations() {
        report(&label, &eval_tage(&entries, offset, configuration, warmup, limit));
    }

    println!();
    println!("== combined ==");
    report(
        "PR#7 TAGE (legacy)",
        &eval_tage(&entries, offset, Configuration::legacy(), warmup, limit),
    );
    report(
        "tuned default",
        &eval_tage(&entries, offset, Configuration::default(), warmup, limit),
    );
}

fn stage_two() -> Vec<(String, Configuration)> {
    let mut variants = vec![("tuned default".to_string(), Configuration::default())];

    for size in [1_024, 4_096, 16_384] {
        variants.push((
            format!("tables {size}"),
            Configuration {
                base_table_size: size,
                tagged_table_size: size,
                ..Configuration::default()
            },
        ));
    }
    variants.push((
        "elapsed context".to_string(),
        Configuration {
            elapsed_context: true,
            ..Configuration::default()
        },
    ));
    for lengths in [vec![1, 2, 3, 4], vec![1, 2, 4, 8], vec![1, 2, 4, 8, 16, 32, 64]] {
        variants.push((
            format!("history {lengths:?}"),
            Configuration {
                history_lengths: lengths,
                ..Configuration::default()
            },
        ));
    }
    for counter_max in [3, 15, 31] {
        variants.push((
            format!("counter max {counter_max}"),
            Configuration {
                counter_max,
                ..Configuration::default()
            },
        ));
    }
    for base in [1.0, 1.5, 3.0, 4.0] {
        variants.push((
            format!("length base {base}"),
            Configuration {
                fusion_length_base: base,
                ..Configuration::default()
            },
        ));
    }
    for decay in [0.85, 0.9, 0.99] {
        variants.push((
            format!("recency decay {decay}"),
            Configuration {
                recency_decay: decay,
                ..Configuration::default()
            },
        ));
    }
    for interval in [64, 1_024] {
        variants.push((
            format!("aging interval {interval}"),
            Configuration {
                usefulness_aging_interval: interval,
                ..Configuration::default()
            },
        ));
    }
    variants
}

fn grid() -> Vec<(String, Configuration)> {
    let mut variants = Vec::new();
    for prior in [0.15, 0.25, 0.4, 0.6, 0.8] {
        for weekday_mode in [WeekdayMode::Ignored, WeekdayMode::Weekend, WeekdayMode::Exact] {
            for hour_bucket in [1, 2, 3, 4, 6] {
                for capacity in [3, 5, 8] {
                    for time_context_tables in [usize::MAX, 2] {
                        let configuration = Configuration {
                            ranking: Ranking::Fusion,
                            fusion_prior_weight: prior,
                            weekday_mode,
                            hour_bucket,
                            candidate_capacity: capacity,
                            time_context_tables,
                            ..Configuration::legacy()
                        };
                        let scope = if time_context_tables == usize::MAX {
                            "all".to_string()
                        } else {
                            time_context_tables.to_string()
                        };
                        variants.push((
                            format!(
                                "prior {prior} wd {weekday_mode:?} hb {hour_bucket} cap {capacity} tct {scope}"
                            ),
                            configuration,
                        ));
                    }
                }
            }
        }
    }
    variants
}

fn ablations() -> Vec<(String, Configuration)> {
    let mut variants = Vec::new();

    variants.push(("legacy".to_string(), Configuration::legacy()));

    for size in [1_024, 4_096, 16_384] {
        variants.push((
            format!("+ tables {size}"),
            Configuration {
                base_table_size: size,
                tagged_table_size: size,
                ..Configuration::legacy()
            },
        ));
    }

    for bucket in [2, 3, 4, 6, 24] {
        variants.push((
            format!("+ hour bucket {bucket}"),
            Configuration {
                hour_bucket: bucket,
                ..Configuration::legacy()
            },
        ));
    }

    for (label, mode) in [
        ("weekend", WeekdayMode::Weekend),
        ("ignored", WeekdayMode::Ignored),
    ] {
        variants.push((
            format!("+ weekday {label}"),
            Configuration {
                weekday_mode: mode,
                ..Configuration::legacy()
            },
        ));
    }

    for tables in [0, 1, 2, 3, 4] {
        variants.push((
            format!("+ time in first {tables} tables"),
            Configuration {
                time_context_tables: tables,
                ..Configuration::legacy()
            },
        ));
    }

    variants.push((
        "+ elapsed context".to_string(),
        Configuration {
            elapsed_context: true,
            ..Configuration::legacy()
        },
    ));

    for lengths in [
        vec![1, 2, 3, 4],
        vec![1, 2, 3, 4, 6, 8],
        vec![1, 2, 4, 8],
        vec![1, 2, 4, 8, 16, 32, 64],
    ] {
        variants.push((
            format!("+ history {lengths:?}"),
            Configuration {
                history_lengths: lengths,
                ..Configuration::legacy()
            },
        ));
    }

    for capacity in [4, 6, 8, 15] {
        variants.push((
            format!("+ candidates {capacity}"),
            Configuration {
                candidate_capacity: capacity,
                ..Configuration::legacy()
            },
        ));
    }

    for counter_max in [3, 15, 31] {
        variants.push((
            format!("+ counter max {counter_max}"),
            Configuration {
                counter_max,
                ..Configuration::legacy()
            },
        ));
    }

    variants.push((
        "+ fusion".to_string(),
        Configuration {
            ranking: Ranking::Fusion,
            ..Configuration::legacy()
        },
    ));

    for prior in [0.1, 0.25, 0.5, 1.0] {
        variants.push((
            format!("+ fusion, prior {prior}"),
            Configuration {
                ranking: Ranking::Fusion,
                fusion_prior_weight: prior,
                ..Configuration::legacy()
            },
        ));
    }

    for base in [1.0, 1.5, 3.0] {
        variants.push((
            format!("+ fusion, length base {base}"),
            Configuration {
                ranking: Ranking::Fusion,
                fusion_length_base: base,
                ..Configuration::legacy()
            },
        ));
    }

    for decay in [0.9, 0.99, 1.0] {
        variants.push((
            format!("+ recency decay {decay}"),
            Configuration {
                recency_decay: decay,
                ..Configuration::legacy()
            },
        ));
    }

    variants
}
