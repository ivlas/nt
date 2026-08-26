# Generic agentic primitives benchmark

This benchmark checks whether an external policy can select an arbitrary set of
note IDs and retrieve those notes with one `nt` process. It does not model
memory, ranking, context construction, or any other higher-level agent policy.

## Method

The ignored release-mode integration benchmark in
`tests/agentic_primitives_benchmark.rs` grows one database through 1,000,
100,000, and 1,000,000 canonical notes. Each note has an approximately 203-byte
CommonMark body, a change-feed row, an FTS entry maintained by the production
trigger, one of two collections, and a tag on every fourth note. Real add
samples remain in the database, so the measured checkpoint counts are 1,000,
100,011, and 1,000,022 notes. The resulting SQLite database is 1,068,475,912
bytes.

At every checkpoint the benchmark takes five samples and reports medians. Read
operations are warmed once. Direct SQLite timings prepare and consume the same
shape of statement used by `nt`, without process startup, domain decoding, JSON
encoding, or output. Process timings invoke the release binary with redirected
output. Peak RSS is sampled separately with `/usr/bin/time`. Arbitrary IDs are
spread deterministically through the corpus and supplied through `id:-`.
Each population phase allocates synthetic revisions above the current stored
global revision, including revisions consumed by earlier measured mutations.
The recent-change cursor is derived from that resulting global revision.

Single add and edit timings perform real mutations. The 128-note batch mutation
alternates between two collections so every sample changes all requested notes
in one transaction. Mutation results include process and SQLite transaction
cost; direct SQLite time is intentionally not reported because bypassing the
repository would not exercise the canonical mutation and revision semantics.

Run it with:

```sh
cargo test --locked --release --test agentic_primitives_benchmark \
  benchmark_generic_agentic_primitives -- \
  --ignored --exact --nocapture
```

## Results

Times are milliseconds and RSS is MiB. `SQLite` is the direct query time.

| Notes | Operation | K | SQLite | First row | Total | RSS |
|---:|---|---:|---:|---:|---:|---:|
| 1,000 | collection read | 100 | 0.087 | 2.678 | 2.870 | 4.33 |
| 1,000 | ID read | 32 | 0.048 | 2.252 | 2.648 | 4.16 |
| 1,000 | ID read | 64 | 0.075 | 2.234 | 2.519 | 4.31 |
| 1,000 | ID read | 96 | 0.124 | 2.271 | 2.434 | 4.44 |
| 1,000 | ID read | 128 | 0.134 | 2.234 | 2.638 | 4.52 |
| 1,000 | recent changes | 10 | 0.003 | 1.983 | 2.058 | 3.61 |
| 1,000 | single add | 1 | n/a | 2.326 | 2.436 | 4.14 |
| 1,000 | single edit | 1 | n/a | 2.579 | 2.870 | 4.14 |
| 1,000 | batch move | 128 | n/a | 3.698 | 3.667 | 4.64 |
| 100,011 | collection read | 100 | 0.081 | 2.374 | 2.796 | 4.38 |
| 100,011 | ID read | 32 | 0.051 | 2.287 | 2.613 | 4.58 |
| 100,011 | ID read | 64 | 0.094 | 2.371 | 2.593 | 5.17 |
| 100,011 | ID read | 96 | 0.125 | 2.436 | 2.801 | 5.72 |
| 100,011 | ID read | 128 | 0.171 | 2.582 | 2.917 | 6.16 |
| 100,011 | recent changes | 10 | 0.003 | 1.922 | 2.066 | 3.62 |
| 100,011 | single add | 1 | n/a | 2.295 | 2.496 | 4.27 |
| 100,011 | single edit | 1 | n/a | 2.522 | 2.624 | 4.22 |
| 100,011 | batch move | 128 | n/a | 4.767 | 4.839 | 5.81 |
| 1,000,022 | collection read | 100 | 0.096 | 2.231 | 2.509 | 4.41 |
| 1,000,022 | ID read | 32 | 0.054 | 2.189 | 2.471 | 4.75 |
| 1,000,022 | ID read | 64 | 0.109 | 2.483 | 2.720 | 5.69 |
| 1,000,022 | ID read | 96 | 0.361 | 2.697 | 2.813 | 6.44 |
| 1,000,022 | ID read | 128 | 0.441 | 2.918 | 3.153 | 6.53 |
| 1,000,022 | recent changes | 10 | 0.003 | 1.808 | 2.079 | 3.61 |
| 1,000,022 | single add | 1 | n/a | 2.620 | 2.412 | 4.25 |
| 1,000,022 | single edit | 1 | n/a | 2.578 | 2.792 | 4.20 |
| 1,000,022 | batch move | 128 | n/a | 5.751 | 5.691 | 6.59 |

First-row and total columns come from separate sample sets, so sub-millisecond
noise can make a median first-row value slightly exceed the median total value
for short operations.

## Findings

Process startup and database validation dominate bounded reads. Direct SQLite
cost is 0.003-0.441 ms, while complete process time is 2.058-3.153 ms.

Collection reads and recent change reads remain flat as the database grows by
three orders of magnitude. Their indexes bound work by the requested range and
result count.

Exact-ID reads scale with K and point-lookup depth, not linearly with total
database contents. At K=128, increasing the database from 1,000 to 1,000,022
notes adds 0.307 ms of SQLite time and 0.515 ms of total process time. At about
one million notes, increasing K from 32 to 128 adds 0.387 ms of SQLite time,
0.682 ms total, and 1.78 MiB RSS.

Single mutations remain flat with database size. A real 128-note batch move at
about one million notes takes 5.691 ms and 6.59 MiB RSS, preserving one-process
and one-transaction semantics.

The measurements support the intended architecture:

```text
large canonical note database
            |
external policy selects K relevant IDs
            |
one nt invocation
            |
O(K) notes returned
```

The caller chooses K. The sampled value 96 has no distinct behavior and does
not justify a hardcoded limit or architectural constant. No meaningful generic
primitive bottleneck was found, so no production optimization or
memory-specific behavior was added.
