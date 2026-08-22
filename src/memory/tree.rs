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

#[cfg(test)]
mod tests {
    use super::{Children, children, level0_for_seq, parent, range, span};
    use crate::memory::MEMORY_FANOUT;

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
}
