use super::{MEMORY_FANOUT, SummaryNodeId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RawRange {
    start: u64,
    end: u64,
}

impl RawRange {
    pub(crate) fn start(self) -> u64 {
        self.start
    }

    pub(crate) fn end(self) -> u64 {
        self.end
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Children {
    Raw(RawRange),
    Nodes(Box<[SummaryNodeId; MEMORY_FANOUT as usize]>),
}

pub(crate) fn span(level: u64) -> Option<u64> {
    let exponent = u32::try_from(level.checked_add(1)?).ok()?;
    MEMORY_FANOUT.checked_pow(exponent)
}

pub(crate) fn range(level: u64, block: u64) -> Option<RawRange> {
    SummaryNodeId::new(level, block).ok()?;
    let span = span(level)?;
    let zero_based_start = block.checked_mul(span)?;
    let end = zero_based_start.checked_add(span)?;
    if end > i64::MAX as u64 {
        return None;
    }
    Some(RawRange {
        start: zero_based_start.checked_add(1)?,
        end,
    })
}

pub(crate) fn parent(level: u64, block: u64) -> Option<SummaryNodeId> {
    range(level, block)?;
    let parent = SummaryNodeId::new(level.checked_add(1)?, block / MEMORY_FANOUT).ok()?;
    range(parent.level(), parent.block())?;
    Some(parent)
}

pub(crate) fn children(level: u64, block: u64) -> Option<Children> {
    range(level, block)?;
    if level == 0 {
        return range(level, block).map(Children::Raw);
    }

    let child_level = level.checked_sub(1)?;
    let first_block = block.checked_mul(MEMORY_FANOUT)?;
    first_block.checked_add(MEMORY_FANOUT - 1)?;
    let nodes = (0..MEMORY_FANOUT)
        .map(|offset| SummaryNodeId::new(child_level, first_block + offset).ok())
        .collect::<Option<Vec<_>>>()?;
    Some(Children::Nodes(Box::new(nodes.try_into().ok()?)))
}

pub(crate) fn level0_for_seq(seq: i64) -> Option<SummaryNodeId> {
    let zero_based = u64::try_from(seq.checked_sub(1)?).ok()?;
    let node = SummaryNodeId::new(0, zero_based / MEMORY_FANOUT).ok()?;
    range(node.level(), node.block())?;
    Some(node)
}

pub(crate) fn frontier<E>(
    highest_seq: u64,
    recent_start: u64,
    limit: usize,
    mut next_summary_block: impl FnMut(u64, u64) -> std::result::Result<Option<u64>, E>,
) -> std::result::Result<Vec<SummaryNodeId>, E> {
    let history_end = highest_seq
        .min(recent_start.saturating_sub(1))
        .min(i64::MAX as u64);
    let completed_level_zero_blocks = history_end / MEMORY_FANOUT;
    let mut widths = Vec::new();
    let mut width = 1_u64;
    while width <= completed_level_zero_blocks {
        widths.push(width);
        let Some(next) = width.checked_mul(MEMORY_FANOUT) else {
            break;
        };
        width = next;
    }
    let mut nodes = Vec::new();
    let mut level_zero_block = 0;

    while level_zero_block < completed_level_zero_blocks && nodes.len() < limit {
        let mut selected = None;
        let mut next_start = None;
        for (level, width) in widths.iter().copied().enumerate() {
            let first_block = level_zero_block.div_ceil(width);
            let complete_blocks = completed_level_zero_blocks / width;
            if first_block >= complete_blocks {
                continue;
            }
            let Some(block) = next_summary_block(level as u64, first_block)? else {
                continue;
            };
            if block < first_block || block >= complete_blocks {
                continue;
            }
            let start = block
                .checked_mul(width)
                .expect("frontier nodes fit the canonical memory range");
            next_start = Some(next_start.map_or(start, |next: u64| next.min(start)));
            if start == level_zero_block {
                let node = SummaryNodeId::new(level as u64, block)
                    .expect("frontier nodes fit the canonical memory range");
                selected = Some((node, width));
            }
        }

        if let Some((node, width)) = selected {
            nodes.push(node);
            level_zero_block += width;
        } else if let Some(next) = next_start {
            level_zero_block = next;
        } else {
            break;
        }
    }

    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::BTreeSet;
    use std::convert::Infallible;

    use super::{Children, children, frontier, level0_for_seq, parent, range, span};
    use crate::memory::{MEMORY_FANOUT, SummaryNodeId};

    fn node(level: u64, block: u64) -> SummaryNodeId {
        SummaryNodeId::new(level, block).unwrap()
    }

    fn complete_frontier(highest_seq: u64, recent_start: u64) -> Vec<SummaryNodeId> {
        frontier(highest_seq, recent_start, usize::MAX, |_, first_block| {
            Ok::<_, Infallible>(Some(first_block))
        })
        .unwrap()
    }

    fn available_frontier(
        highest_seq: u64,
        recent_start: u64,
        available: &BTreeSet<SummaryNodeId>,
    ) -> Vec<SummaryNodeId> {
        frontier(
            highest_seq,
            recent_start,
            usize::MAX,
            |level, first_block| {
                Ok::<_, Infallible>(
                    available
                        .range(node(level, first_block)..)
                        .next()
                        .filter(|candidate| candidate.level() == level)
                        .map(|candidate| candidate.block()),
                )
            },
        )
        .unwrap()
    }

    #[test]
    fn spans_are_checked_powers_of_fanout() {
        assert_eq!(span(0), Some(16));
        assert_eq!(span(1), Some(256));
        assert_eq!(span(14), Some(1_u64 << 60));
        assert_eq!(span(15), None);
        assert_eq!(span(u64::MAX), None);
    }

    #[test]
    fn ranges_are_inclusive_positive_and_checked() {
        let first = range(0, 0).unwrap();
        assert_eq!((first.start(), first.end()), (1, 16));
        let second = range(0, 1).unwrap();
        assert_eq!((second.start(), second.end()), (17, 32));

        let high = range(14, 6).unwrap();
        assert_eq!(high.end(), 7_u64 << 60);
        assert!(range(14, 7).is_none());
        assert!(range(14, 15).is_none());
        assert!(range(0, i64::MAX as u64).is_none());
        assert!(range(0, i64::MAX as u64 + 1).is_none());
    }

    #[test]
    fn parents_and_children_use_previous_level_blocks() {
        let node = parent(0, 31).unwrap();
        assert_eq!((node.level(), node.block()), (1, 1));

        let Children::Nodes(nodes) = children(2, 3).unwrap() else {
            panic!("higher-level node returned raw children");
        };
        assert_eq!((nodes[0].level(), nodes[0].block()), (1, 48));
        assert_eq!((nodes[15].level(), nodes[15].block()), (1, 63));
        assert!(
            nodes
                .iter()
                .all(|child| parent(child.level(), child.block()).unwrap().block() == 3)
        );

        let Children::Raw(raw) = children(0, 2).unwrap() else {
            panic!("level zero node returned summary children");
        };
        assert_eq!((raw.start(), raw.end()), (33, 48));
    }

    #[test]
    fn caller_controlled_overflow_returns_none() {
        assert!(parent(i64::MAX as u64, 0).is_none());
        assert!(parent(i64::MAX as u64 + 1, 0).is_none());
        assert!(children(1, i64::MAX as u64 / MEMORY_FANOUT + 1).is_none());
        assert!(children(u64::MAX, 0).is_none());
        assert!(level0_for_seq(0).is_none());
        assert!(level0_for_seq(i64::MIN).is_none());
    }

    #[test]
    fn every_positive_sequence_maps_to_its_level_zero_range() {
        for seq in [1, 16, 17, 32, 33, 1_000, i64::MAX - 15] {
            let node = level0_for_seq(seq).unwrap();
            let raw = range(node.level(), node.block()).unwrap();
            let seq = seq as u64;
            assert!(raw.start() <= seq && seq <= raw.end());
        }
        assert!(level0_for_seq(i64::MAX).is_none());
    }

    #[test]
    fn child_ranges_partition_parent_ranges() {
        for level in 1..=5 {
            for block in 0..100 {
                let parent_range = range(level, block).unwrap();
                let Children::Nodes(nodes) = children(level, block).unwrap() else {
                    unreachable!();
                };
                let first = range(nodes[0].level(), nodes[0].block()).unwrap();
                let last = range(nodes[15].level(), nodes[15].block()).unwrap();
                assert_eq!(first.start(), parent_range.start());
                assert_eq!(last.end(), parent_range.end());
                for pair in nodes.windows(2) {
                    let left = range(pair[0].level(), pair[0].block()).unwrap();
                    let right = range(pair[1].level(), pair[1].block()).unwrap();
                    assert_eq!(left.end().checked_add(1), Some(right.start()));
                }
            }
        }
    }

    #[test]
    fn valid_children_exhaustively_partition_sampled_parent_ranges() {
        for level in 1..=14 {
            let width = span(level).unwrap();
            let max_block = i64::MAX as u64 / width - 1;
            let mut blocks = (0..=max_block.min(1_024)).collect::<BTreeSet<_>>();
            for block in max_block.saturating_sub(2)..=max_block {
                blocks.insert(block);
            }

            for block in blocks {
                let parent_node = node(level, block);
                let parent_range = range(level, block).unwrap();
                let Children::Nodes(nodes) = children(level, block).unwrap() else {
                    unreachable!();
                };
                let child_ranges = nodes
                    .iter()
                    .map(|child| {
                        assert_eq!(parent(child.level(), child.block()), Some(parent_node));
                        range(child.level(), child.block()).unwrap()
                    })
                    .collect::<Vec<_>>();

                assert_eq!(child_ranges[0].start(), parent_range.start());
                assert_eq!(child_ranges[15].end(), parent_range.end());
                assert!(child_ranges.iter().all(|raw| {
                    raw.start() > 0 && raw.start() <= raw.end() && raw.end() <= i64::MAX as u64
                }));
                assert!(
                    child_ranges
                        .windows(2)
                        .all(|pair| pair[0].end().checked_add(1) == Some(pair[1].start()))
                );
                assert_eq!(
                    child_ranges
                        .iter()
                        .map(|raw| raw.end() - raw.start() + 1)
                        .sum::<u64>(),
                    parent_range.end() - parent_range.start() + 1
                );
            }
        }
    }

    #[test]
    fn level_zero_mapping_contains_sequences_around_fanout_powers_and_ceiling() {
        let mut sequences = (1..=100_000_i64).collect::<BTreeSet<_>>();
        let mut power = MEMORY_FANOUT;
        while power <= i64::MAX as u64 {
            for candidate in [power.saturating_sub(1), power, power.saturating_add(1)] {
                if let Ok(candidate) = i64::try_from(candidate) {
                    sequences.insert(candidate);
                }
            }
            let Some(next) = power.checked_mul(MEMORY_FANOUT) else {
                break;
            };
            power = next;
        }
        for offset in 0..=32 {
            sequences.insert(i64::MAX - offset);
        }

        for seq in sequences {
            if let Some(node) = level0_for_seq(seq) {
                let raw = range(node.level(), node.block()).unwrap();
                let seq = seq as u64;
                assert!(raw.start() <= seq && seq <= raw.end());
                assert!(raw.start() > 0);
                assert!(raw.end() <= i64::MAX as u64);
            }
        }
        assert!(level0_for_seq(i64::MAX).is_none());
    }

    #[test]
    fn frontier_is_empty_without_completed_historical_blocks() {
        for (highest_seq, recent_start) in [(0, 1), (15, 16), (1_000, 1)] {
            let nodes = frontier(
                highest_seq,
                recent_start,
                usize::MAX,
                |_, _| -> Result<Option<u64>, Infallible> {
                    panic!("an empty frontier must not inspect summaries")
                },
            )
            .unwrap();
            assert!(nodes.is_empty());
        }
    }

    #[test]
    fn frontier_uses_highest_nodes_at_exact_pyramid_boundaries() {
        assert_eq!(complete_frontier(16, 17), [node(0, 0)]);
        assert_eq!(complete_frontier(256, 257), [node(1, 0)]);
        assert_eq!(complete_frontier(4_096, 4_097), [node(2, 0)]);
    }

    #[test]
    fn frontier_decomposes_partial_levels_canonically() {
        let nodes = complete_frontier(496, 497);
        assert_eq!(nodes.first(), Some(&node(1, 0)));
        assert_eq!(nodes.len(), 16);
        assert_eq!(
            &nodes[1..],
            &(16..31).map(|block| node(0, block)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn frontier_falls_back_to_completed_children_when_parents_are_missing() {
        let level_zero = (0..16).map(|block| node(0, block)).collect::<BTreeSet<_>>();
        let nodes = available_frontier(256, 257, &level_zero);
        assert_eq!(nodes, level_zero.into_iter().collect::<Vec<_>>());

        let level_one = (0..16).map(|block| node(1, block)).collect::<BTreeSet<_>>();
        let nodes = available_frontier(4_096, 4_097, &level_one);
        assert_eq!(nodes, level_one.into_iter().collect::<Vec<_>>());
    }

    #[test]
    fn frontier_covers_available_ranges_without_overlap_in_chronological_order() {
        let available = (0..16)
            .filter(|block| *block != 3)
            .map(|block| node(0, block))
            .collect::<BTreeSet<_>>();
        let nodes = available_frontier(256, 257, &available);
        assert_eq!(nodes.len(), 15);

        let ranges = nodes
            .iter()
            .map(|candidate| range(candidate.level(), candidate.block()).unwrap())
            .collect::<Vec<_>>();
        assert!(
            ranges
                .windows(2)
                .all(|pair| pair[0].end() < pair[1].start())
        );
        assert_eq!(
            ranges
                .iter()
                .map(|range| range.end() - range.start() + 1)
                .sum::<u64>(),
            240
        );
    }

    #[test]
    fn complete_frontiers_are_chronological_and_non_overlapping_at_boundaries() {
        let mut highest_values =
            BTreeSet::from([1_u64, 15, 16, 17, 255, 256, 257, i64::MAX as u64]);
        let mut power = MEMORY_FANOUT;
        while power <= i64::MAX as u64 {
            highest_values.insert(power.saturating_sub(1));
            highest_values.insert(power);
            if power < i64::MAX as u64 {
                highest_values.insert(power + 1);
            }
            let Some(next) = power.checked_mul(MEMORY_FANOUT) else {
                break;
            };
            power = next;
        }

        for highest_seq in highest_values {
            let nodes = complete_frontier(highest_seq, highest_seq.saturating_add(1));
            let ranges = nodes
                .iter()
                .map(|candidate| range(candidate.level(), candidate.block()).unwrap())
                .collect::<Vec<_>>();
            assert!(ranges.iter().all(|raw| {
                raw.start() > 0
                    && raw.start() <= raw.end()
                    && raw.end() <= highest_seq.min(i64::MAX as u64)
            }));
            assert!(
                ranges
                    .windows(2)
                    .all(|pair| pair[0].end() < pair[1].start())
            );
        }
    }

    #[test]
    fn frontier_excludes_the_recent_raw_tail() {
        let nodes = complete_frontier(1_000_000, 999_745);
        assert!(nodes.iter().all(|candidate| {
            range(candidate.level(), candidate.block()).unwrap().end() < 999_745
        }));
    }

    #[test]
    fn complete_frontier_size_grows_with_tree_depth_not_history_size() {
        for highest_seq in [1_000_000, 10_000_000, 100_000_000] {
            let nodes = complete_frontier(highest_seq, highest_seq + 1);
            let levels = (0..)
                .take_while(|level| span(*level).is_some_and(|width| width <= highest_seq))
                .count();
            assert!(
                nodes.len() <= 15 * levels,
                "{} nodes for {highest_seq}",
                nodes.len()
            );
            assert_eq!(
                nodes
                    .iter()
                    .map(|candidate| span(candidate.level()).unwrap())
                    .sum::<u64>(),
                highest_seq / MEMORY_FANOUT * MEMORY_FANOUT
            );
        }
    }

    #[test]
    fn frontier_prefix_bounds_incomplete_pyramids_without_changing_order() {
        let nodes = frontier(10_000_000, 10_000_001, 256, |level, first_block| {
            Ok::<_, Infallible>((level == 0).then_some(first_block))
        })
        .unwrap();
        assert_eq!(
            nodes,
            (0..256).map(|block| node(0, block)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn frontier_jumps_across_sparse_history() {
        let probes = Cell::new(0);
        let nodes = frontier(
            100_000_000,
            100_000_001,
            usize::MAX,
            |level, first_block| {
                probes.set(probes.get() + 1);
                Ok::<_, Infallible>((level == 0 && first_block <= 6_249_999).then_some(6_249_999))
            },
        )
        .unwrap();
        assert_eq!(nodes, [node(0, 6_249_999)]);
        assert!(probes.get() < 20, "{} availability probes", probes.get());
    }
}
