use std::fmt;
use std::str::FromStr;

use crate::error::{NtError, Result};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MemoryRange {
    lo: u64,
    hi: u64,
}

impl MemoryRange {
    pub(crate) fn new(lo: u64, hi: u64) -> Result<Self> {
        let Some(size) = hi.checked_sub(lo) else {
            return Err(invalid_range(format!("{lo}-{hi}")));
        };
        if size < 2 || !size.is_power_of_two() || !lo.is_multiple_of(size) || hi > i64::MAX as u64 {
            return Err(invalid_range(format!("{lo}-{}", hi.saturating_sub(1))));
        }
        Ok(Self { lo, hi })
    }

    pub(crate) fn from_parts(lo: u64, hi: u64) -> Self {
        debug_assert!(Self::new(lo, hi).is_ok());
        Self { lo, hi }
    }

    pub(crate) fn lo(self) -> u64 {
        self.lo
    }

    pub(crate) fn hi(self) -> u64 {
        self.hi
    }

    pub(crate) fn size(self) -> u64 {
        self.hi - self.lo
    }

    pub(crate) fn children(self) -> (WakeNode, WakeNode) {
        let mid = self.lo + self.size() / 2;
        if self.size() == 2 {
            (WakeNode::Raw(self.lo), WakeNode::Raw(mid))
        } else {
            (
                WakeNode::Summary(Self::from_parts(self.lo, mid)),
                WakeNode::Summary(Self::from_parts(mid, self.hi)),
            )
        }
    }

    #[cfg(test)]
    pub(crate) fn parent(self) -> Option<Self> {
        let size = self.size().checked_mul(2)?;
        let lo = self.lo - self.lo % size;
        let hi = lo.checked_add(size)?;
        Self::new(lo, hi).ok()
    }

    #[cfg(test)]
    pub(crate) fn contains(self, other: Self) -> bool {
        self.lo <= other.lo && self.hi >= other.hi
    }
}

impl fmt::Display for MemoryRange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}-{}", self.lo, self.hi - 1)
    }
}

impl FromStr for MemoryRange {
    type Err = NtError;

    fn from_str(value: &str) -> Result<Self> {
        let Some((lo, inclusive_hi)) = value.split_once('-') else {
            return Err(invalid_range(value.to_string()));
        };
        if !canonical_integer(lo) || !canonical_integer(inclusive_hi) {
            return Err(invalid_range(value.to_string()));
        }
        let lo = lo
            .parse::<u64>()
            .map_err(|_| invalid_range(value.to_string()))?;
        let inclusive_hi = inclusive_hi
            .parse::<u64>()
            .map_err(|_| invalid_range(value.to_string()))?;
        let hi = inclusive_hi
            .checked_add(1)
            .ok_or_else(|| invalid_range(value.to_string()))?;
        Self::new(lo, hi).map_err(|_| invalid_range(value.to_string()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WakeNode {
    Raw(u64),
    Summary(MemoryRange),
}

impl WakeNode {
    pub(crate) fn lo(self) -> u64 {
        match self {
            Self::Raw(seq) => seq,
            Self::Summary(range) => range.lo(),
        }
    }

    pub(crate) fn hi(self) -> u64 {
        match self {
            Self::Raw(seq) => seq + 1,
            Self::Summary(range) => range.hi(),
        }
    }

    pub(crate) fn size(self) -> u64 {
        self.hi() - self.lo()
    }
}

pub(crate) fn wake_cover(raw_count: u64, budget: usize) -> Result<Vec<WakeNode>> {
    if raw_count == 0 {
        return Ok(Vec::new());
    }
    if raw_count > i64::MAX as u64 {
        return Err(invalid_range("history exceeds SQLite identity".to_string()));
    }

    let mut cover = canonical_cover(raw_count);
    if cover.len() > budget {
        return Err(NtError::InvalidValue {
            field: "wake budget",
            value: format!("{budget} entries cannot cover this history"),
        });
    }
    while cover.len() < budget {
        let Some(index) = cover
            .iter()
            .enumerate()
            .filter(|(_, node)| node.size() > 1)
            .max_by(|(_, left), (_, right)| relative_coarseness(**left, **right, raw_count))
            .map(|(index, _)| index)
        else {
            break;
        };
        let WakeNode::Summary(range) = cover[index] else {
            unreachable!();
        };
        let (left, right) = range.children();
        cover.splice(index..=index, [left, right]);
    }
    Ok(cover)
}

fn relative_coarseness(left: WakeNode, right: WakeNode, raw_count: u64) -> std::cmp::Ordering {
    let left_age = raw_count - left.hi();
    let right_age = raw_count - right.hi();
    match (left_age, right_age) {
        (0, 0) => left.lo().cmp(&right.lo()),
        (0, _) => std::cmp::Ordering::Greater,
        (_, 0) => std::cmp::Ordering::Less,
        _ => (u128::from(left.size()) * u128::from(right_age))
            .cmp(&(u128::from(right.size()) * u128::from(left_age)))
            .then_with(|| left.lo().cmp(&right.lo())),
    }
}

fn canonical_cover(raw_count: u64) -> Vec<WakeNode> {
    let mut cover = Vec::new();
    let mut lo = 0_u64;
    let mut bit = 1_u64 << (63 - raw_count.leading_zeros());
    while bit != 0 {
        if raw_count & bit != 0 {
            if bit == 1 {
                cover.push(WakeNode::Raw(lo));
            } else {
                cover.push(WakeNode::Summary(MemoryRange::from_parts(lo, lo + bit)));
            }
            lo += bit;
        }
        bit >>= 1;
    }
    cover
}

fn canonical_integer(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
}

fn invalid_range(value: String) -> NtError {
    NtError::InvalidValue {
        field: "memory range",
        value,
    }
}

#[cfg(test)]
mod tests {
    use super::{MemoryRange, WakeNode, wake_cover};

    fn range(value: &str) -> MemoryRange {
        value.parse().unwrap()
    }

    #[test]
    fn ranges_parse_display_and_partition_with_checked_arithmetic() {
        for value in ["0-1", "0-3", "4-7", "24-31"] {
            assert_eq!(range(value).to_string(), value);
        }
        for value in [
            "",
            "0",
            "1-2",
            "0-2",
            "2-5",
            "04-7",
            "0-9223372036854775807",
        ] {
            assert!(value.parse::<MemoryRange>().is_err(), "accepted {value}");
        }

        let parent = range("8-15");
        let (WakeNode::Summary(left), WakeNode::Summary(right)) = parent.children() else {
            panic!("expected summaries");
        };
        assert_eq!((left, right), (range("8-11"), range("12-15")));
        assert_eq!(left.parent(), Some(parent));
        assert!(parent.contains(left));

        let (WakeNode::Raw(0), WakeNode::Raw(1)) = range("0-1").children() else {
            panic!("expected raw memories");
        };
        assert!(MemoryRange::new(u64::MAX - 1, u64::MAX).is_err());
    }

    #[test]
    fn wake_cover_properties_hold_across_histories_and_budgets() {
        assert!(wake_cover(0, 0).unwrap().is_empty());
        for count in 1..=512_u64 {
            let minimum = count.count_ones() as usize;
            let maximum = usize::try_from(count.min(128)).unwrap();
            let mut previous = None;
            for budget in minimum..=maximum {
                let cover = wake_cover(count, budget).unwrap();
                assert!(cover.len() <= budget);
                assert_eq!(cover.first().unwrap().lo(), 0);
                assert_eq!(cover.last().unwrap().hi(), count);
                assert!(cover.windows(2).all(|pair| pair[0].hi() == pair[1].lo()));
                assert!(
                    cover
                        .windows(2)
                        .all(|pair| pair[0].size() >= pair[1].size())
                );
                assert!(cover.iter().all(|node| match node {
                    WakeNode::Raw(_) => true,
                    WakeNode::Summary(range) => MemoryRange::new(range.lo(), range.hi()).is_ok(),
                }));
                if count <= budget as u64 {
                    assert!(cover.iter().all(|node| matches!(node, WakeNode::Raw(_))));
                }
                if let Some(old) = previous.replace(cover.clone()) {
                    for old_node in old {
                        assert!(cover.iter().any(|new_node| {
                            old_node.lo() <= new_node.lo() && old_node.hi() >= new_node.hi()
                        }));
                    }
                }
                assert_eq!(cover, wake_cover(count, budget).unwrap());
            }
        }

        for count in [513, 777, 1_000, 4_095, 4_096, 1_000_000, i64::MAX as u64] {
            let minimum = count.count_ones() as usize;
            for budget in [minimum, minimum.max(64), minimum.max(128)] {
                let cover = wake_cover(count, budget).unwrap();
                assert!(cover.len() <= budget);
                assert_eq!(cover.first().unwrap().lo(), 0);
                assert_eq!(cover.last().unwrap().hi(), count);
                assert!(cover.windows(2).all(|pair| pair[0].hi() == pair[1].lo()));
                assert!(
                    cover
                        .windows(2)
                        .all(|pair| pair[0].size() >= pair[1].size())
                );
            }
        }
    }

    #[test]
    fn wake_rejects_a_budget_below_the_minimum_cover() {
        assert!(wake_cover(7, 2).is_err());
    }

    #[test]
    fn wake_uses_age_relative_sizes_instead_of_a_precise_right_tail() {
        let cover = wake_cover(1_000_000, 128).unwrap();
        let raw = cover
            .iter()
            .filter(|node| matches!(node, WakeNode::Raw(_)))
            .count();
        let largest = cover.iter().map(|node| node.size()).max().unwrap();
        assert!(raw < 32, "unexpectedly precise raw tail: {raw}");
        assert!(largest <= 131_072, "oldest block is too coarse: {largest}");
        assert!(
            cover
                .windows(2)
                .all(|pair| pair[0].size() / pair[1].size() <= 2),
            "adjacent ages decay by more than one binary level: {cover:?}"
        );
    }
}
