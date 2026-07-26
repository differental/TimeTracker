# Tuning the activity predictor

This documents how the TAGE predictor added in #7 was measured against real
history, which knobs actually move top-3 accuracy, and why the defaults are what
they are.

## Why

The predictor feeds `GET /api/suggest`, whose whole job is to put the activity
you are about to start into a list of three. So the metric that matters is
**top-3 hit rate**: how often the activity actually logged next appears in the
three suggestions.

Measured that way, the configuration ported from the iOS client was doing worse
than a one-line baseline. On the real log it scored 86.0% top-3, while ranking
candidates by "what usually follows the current activity, in this part of the
day" scored 88.2%. Since it costs a table walk over the whole log per request,
being beaten by a bigram counter is not a good trade.

## How accuracy is measured

`examples/eval_predictor.rs` replays an event log in order. For every entry it
asks the predictor for suggestions *before* training on that entry, so no
prediction ever sees its own answer:

```
cargo run --release --example eval_predictor -- --json export.json --tz 120
cargo run --release --example eval_predictor -- --db ../timetracker.db --tz 120
```

`--json` reads the export written by `GET /api/export` (which lives on `main`,
not on this branch), the easy way to evaluate against a deployed instance
without copying its database. `--tz` is the
UTC offset in minutes that the log was recorded in; get it wrong and every
time-of-day pattern is smeared.

Modes:

- default — baselines, then a one-knob-at-a-time ablation over the iOS config
- `--mode grid` — searches the knobs that interact, picking on one window and
  reporting on a later, untouched one
- `--mode stage2` — one-knob-at-a-time ablation over the current defaults

Entries where the next activity equals the current one are excluded from
scoring, because `predictions` never suggests the current activity by design.

Reference numbers below come from a 3,709-entry log covering 2025-08 to 2026-07,
scored over the most recent half (n=1855). With that many samples one percentage
point is roughly 1.4 standard errors, so treat sub-1% differences as noise.

## Results

| configuration | top-1 | top-3 |
| --- | --- | --- |
| baseline: global frequency | 57.0% | 77.7% |
| baseline: recency | 52.8% | 85.6% |
| baseline: order-1 Markov | 56.9% | 86.4% |
| baseline: order-1 Markov x 4-hour bucket | 62.4% | 88.2% |
| TAGE, iOS configuration (`Configuration::legacy`) | 54.8% | 86.0% |
| TAGE, tuned (`Configuration::default`) | **72.2%** | **93.7%** |

Confirmed on a held-out window the tuning never looked at (the most recent
quarter of the log): 88.4% -> 96.0% top-3, 59.8% -> 78.1% top-1.

## What moved the needle

**Blending the tables instead of ranking them (`Ranking::Fusion`).** The
original walks the trusted table's candidates first, then the alternate's, then
the base table's. That is the right structure for branch prediction, where the
answer is one bit and only the top choice is used — but here slots two and three
end up filled from whichever single table won, including its low-confidence
tail, while a state that every other table agrees on can be pushed out. Fusion
scores each state by a weighted sum of the confidence counters across every
matching table, weighting longer history and proven-useful entries higher, then
ranks. Alone: 84.3% top-3; with the recency prior below, 91.1%.

**Mixing a recency prior into the score (`fusion_prior_weight`).** The
already-maintained `recency_scores` decay makes a good tie-breaker for the third
slot, which is otherwise filled by whatever the tables happened to allocate.
Sweeping 0.1 to 1.0 gives 90.4% to 91.5%, all well above the 84.3% without it;
0.4 is a flat spot in the middle. This is the single biggest contributor.

**Dropping weekday from the context (`WeekdayMode::Ignored`).** Worth about four
points on its own: 86.0% -> 89.9%. Weekday splits every table seven ways, and at
a few thousand entries there is not enough per-weekday data to pay that back —
the tables stay half-trained and the tag matches get rarer. `WeekdayMode::Weekend`
(a weekday/weekend bit, 88.8%) is the middle ground and is the option to revisit
once the log is much longer, since a genuine weekend routine does exist; it is
just cheaper to learn it through recency than through a 7-way split.

**Bucketing hour-of-day (`hour_bucket`).** Exact hours are nearly as sparse as
weekdays. 4-hour blocks land at 87.2% on their own, and the grid search prefers
2 to 4 over 1. 24 (no hour context at all) is worse than any bucketing, so the
time signal is real — it just needs coarser bins.

## What was tried and rejected

**Elapsed time in the current activity.** Bucketing how long the current
activity has been running looked like the most promising missing feature — the
predictor otherwise ignores duration entirely, using timestamps only for
weekday and hour. It measurably hurts: 84.1% under the iOS config, 94.3% vs
96.0% held-out under the tuned one. It adds another 9-way split to a context
that is already short of data, and the gap between logged entries is dominated
by when the user happens to open the app. The knob (`elapsed_context`) is kept
so it can be re-measured on a longer log.

**Wider candidate sets.** Under priority ranking, raising the cap from 3 costs
around three points (86.0% -> 83.2%), because the extra low-confidence entries
displace better suggestions from other tables. Under fusion it is roughly
neutral. Not worth the memory.

**Bigger tables, different history lengths, wider counters, aging interval.**
All within noise on this log. The tables are not under real pressure: 512 entries
against a few thousand training events means collisions are not the constraint,
data sparsity is. Long history lengths (16, 32) are inert for the same reason —
a 32-activity sequence essentially never repeats — but they cost nothing, so the
geometric series is left alone.

## Compatibility

`Configuration::legacy()` reproduces the iOS client's behaviour exactly, and
`predictor::parity::matches_swift_reference_output` pins it against golden output
from the Swift implementation. That test still passes; the tuning is entirely in
the defaults, not the mechanism.

This does mean the server and the iOS client now disagree about what to suggest.
The options, in the order they should probably be considered: port the tuned
configuration to Swift, have the client stop predicting locally and call
`/api/suggest`, or accept the divergence. Nothing here forces that decision.

## Retuning later

The tuning above is fitted to one person's log. The numbers will drift as the
routine changes, and the right defaults for a different user with a different
activity mix are not necessarily these. Rerunning `--mode grid` on a fresh export
is cheap — a few seconds — and the knobs it sweeps are the ones that interact.
Worth doing after a few more months of data, particularly to recheck
`weekday_mode`, which is the tuning most likely to flip as the log grows.
