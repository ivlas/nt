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
100,011, and 1,000,022 notes. The resulting SQLite database is 1,068,471,792
bytes.

At every checkpoint the benchmark takes five samples and reports medians. Read
operations are warmed once. Direct SQLite timings prepare and consume the same
shape of statement used by `nt`, without process startup, domain decoding, JSON
encoding, or output. Process timings invoke the release binary with redirected
output. Peak RSS is sampled separately with `/usr/bin/time` on macOS and Linux;
the field is empty on unsupported platforms. Arbitrary IDs are spread
deterministically through the corpus and supplied through `id:-`.
Each population phase allocates synthetic revisions above the current stored
global revision, including revisions consumed by earlier measured mutations.
The recent-change cursor is derived from that resulting global revision.

The fixture connection uses the same five-second busy timeout and foreign-key
enforcement as production `nt` connections.

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

| Notes | Operation | K | SQLite | First output | Total | RSS |
|---:|---|---:|---:|---:|---:|---:|
| 1,000 | collection read | 100 | 0.086 | 2.416 | 2.675 | 4.34 |
| 1,000 | ID read | 32 | 0.047 | 2.172 | 2.364 | 4.16 |
| 1,000 | ID read | 64 | 0.076 | 2.184 | 2.343 | 4.34 |
| 1,000 | ID read | 96 | 0.111 | 2.220 | 2.362 | 4.41 |
| 1,000 | ID read | 128 | 0.158 | 2.126 | 2.576 | 4.50 |
| 1,000 | recent changes | 10 | 0.003 | 1.983 | 2.205 | 3.61 |
| 1,000 | single add | 1 | n/a | 2.285 | 2.370 | 4.16 |
| 1,000 | single edit | 1 | n/a | 2.598 | 2.647 | 4.14 |
| 1,000 | batch move | 128 | n/a | 3.668 | 3.638 | 4.62 |
| 100,011 | collection read | 100 | 0.082 | 2.275 | 2.707 | 4.36 |
| 100,011 | ID read | 32 | 0.051 | 2.272 | 2.462 | 4.61 |
| 100,011 | ID read | 64 | 0.103 | 2.213 | 2.642 | 5.16 |
| 100,011 | ID read | 96 | 0.149 | 2.504 | 2.692 | 5.70 |
| 100,011 | ID read | 128 | 0.163 | 2.693 | 2.985 | 6.22 |
| 100,011 | recent changes | 10 | 0.003 | 1.918 | 2.256 | 3.61 |
| 100,011 | single add | 1 | n/a | 2.402 | 2.452 | 4.28 |
| 100,011 | single edit | 1 | n/a | 2.589 | 2.659 | 4.20 |
| 100,011 | batch move | 128 | n/a | 5.352 | 4.889 | 5.81 |
| 1,000,022 | collection read | 100 | 0.095 | 2.217 | 2.650 | 4.41 |
| 1,000,022 | ID read | 32 | 0.056 | 2.459 | 2.375 | 4.75 |
| 1,000,022 | ID read | 64 | 0.105 | 2.435 | 2.866 | 5.61 |
| 1,000,022 | ID read | 96 | 0.355 | 2.632 | 2.795 | 6.45 |
| 1,000,022 | ID read | 128 | 0.495 | 2.778 | 3.214 | 6.50 |
| 1,000,022 | recent changes | 10 | 0.003 | 1.934 | 2.110 | 3.61 |
| 1,000,022 | single add | 1 | n/a | 2.470 | 2.611 | 4.27 |
| 1,000,022 | single edit | 1 | n/a | 2.504 | 2.604 | 4.22 |
| 1,000,022 | batch move | 128 | n/a | 5.561 | 5.554 | 6.56 |

First-output and total columns come from separate sample sets, so sub-millisecond
noise can make a median first-output value slightly exceed the median total value
for short operations.

## Findings

Process startup and database validation dominate bounded reads. Direct SQLite
cost is 0.003-0.495 ms, while complete process time is 2.110-3.214 ms.

Collection reads and recent change reads remain flat as the database grows by
three orders of magnitude. Their indexes bound work by the requested range and
result count.

Exact-ID reads scale with K and point-lookup depth, not linearly with total
database contents. At K=128, increasing the database from 1,000 to 1,000,022
notes adds 0.337 ms of SQLite time and 0.638 ms of total process time. At about
one million notes, increasing K from 32 to 128 adds 0.439 ms of SQLite time,
0.839 ms total, and 1.75 MiB RSS.

Single mutations remain flat with database size. A real 128-note batch move at
about one million notes takes 5.554 ms and 6.56 MiB RSS, preserving one-process
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
