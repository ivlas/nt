# Generic agentic primitives benchmark

This benchmark checks whether an external policy can select an arbitrary set of
note IDs and retrieve those notes with one `nt` process. It does not model
memory, ranking, context construction, or any other higher-level agent policy.

## Method

The ignored release-mode integration benchmark in
`tests/agentic_primitives_benchmark.rs` grows one database through 1,000,
100,000, and 1,000,000 canonical notes. Each note has an approximately 203-byte
CommonMark body, a change-feed row, an FTS entry maintained by the production
trigger, one of two collections, and a tag on every fourth note. The resulting
SQLite database is 1,068,471,816 bytes.

At every checkpoint the benchmark takes five samples and reports medians. Read
operations are warmed once. Direct SQLite timings prepare and consume the same
shape of statement used by `nt`, without process startup, domain decoding, JSON
encoding, or output. Process timings invoke the release binary with redirected
output. Peak RSS is sampled separately with `/usr/bin/time`. Arbitrary IDs are
spread deterministically through the corpus and supplied through `id:-`.

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
| 1,000 | collection read | 100 | 0.089 | 2.572 | 2.746 | 4.33 |
| 1,000 | ID read | 32 | 0.053 | 2.275 | 2.405 | 4.12 |
| 1,000 | ID read | 64 | 0.074 | 2.180 | 2.403 | 4.30 |
| 1,000 | ID read | 96 | 0.110 | 2.126 | 2.512 | 4.42 |
| 1,000 | ID read | 128 | 0.132 | 2.234 | 2.594 | 4.52 |
| 1,000 | recent changes | 10 | 0.004 | 2.111 | 2.070 | 3.61 |
| 1,000 | single add | 1 | n/a | 2.636 | 2.430 | 4.14 |
| 1,000 | single edit | 1 | n/a | 2.582 | 2.827 | 4.14 |
| 1,000 | batch move | 128 | n/a | 3.881 | 4.168 | 4.64 |
| 100,000 | collection read | 100 | 0.082 | 2.157 | 2.489 | 4.36 |
| 100,000 | ID read | 32 | 0.055 | 2.049 | 2.461 | 4.58 |
| 100,000 | ID read | 64 | 0.088 | 2.412 | 2.555 | 5.17 |
| 100,000 | ID read | 96 | 0.133 | 2.495 | 2.831 | 5.70 |
| 100,000 | ID read | 128 | 0.182 | 2.821 | 2.985 | 6.20 |
| 100,000 | recent changes | 10 | 0.003 | 2.072 | 1.970 | 3.61 |
| 100,000 | single add | 1 | n/a | 2.122 | 2.425 | 4.28 |
| 100,000 | single edit | 1 | n/a | 2.635 | 2.644 | 4.20 |
| 100,000 | batch move | 128 | n/a | 4.637 | 4.893 | 5.80 |
| 1,000,000 | collection read | 100 | 0.088 | 2.344 | 2.614 | 4.41 |
| 1,000,000 | ID read | 32 | 0.053 | 2.268 | 2.339 | 4.75 |
| 1,000,000 | ID read | 64 | 0.100 | 2.235 | 2.759 | 5.69 |
| 1,000,000 | ID read | 96 | 0.345 | 2.747 | 2.954 | 6.44 |
| 1,000,000 | ID read | 128 | 0.462 | 2.980 | 3.322 | 6.50 |
| 1,000,000 | recent changes | 10 | 0.003 | 2.035 | 1.969 | 3.62 |
| 1,000,000 | single add | 1 | n/a | 2.290 | 2.573 | 4.27 |
| 1,000,000 | single edit | 1 | n/a | 2.604 | 2.670 | 4.22 |
| 1,000,000 | batch move | 128 | n/a | 5.391 | 5.599 | 6.58 |

First-row and total columns come from separate sample sets, so sub-millisecond
noise can make a median first-row value slightly exceed the median total value
for short operations.

## Findings

Process startup and database validation dominate bounded reads. Direct SQLite
cost is 0.003-0.462 ms, while complete process time is 1.969-3.322 ms.

Collection reads and recent change reads remain flat as the database grows by
three orders of magnitude. Their indexes bound work by the requested range and
result count.

Exact-ID reads scale with K and point-lookup depth, not linearly with total
database contents. At K=128, increasing the database from 1,000 to 1,000,000
notes adds 0.330 ms of SQLite time and 0.728 ms of total process time. At one
million notes, increasing K from 32 to 128 adds 0.409 ms of SQLite time, 0.983
ms total, and 1.75 MiB RSS.

Single mutations remain flat with database size. A real 128-note batch move at
one million notes takes 5.599 ms and 6.58 MiB RSS, preserving one-process and
one-transaction semantics.

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
