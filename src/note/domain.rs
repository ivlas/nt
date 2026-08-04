use std::fmt;
use std::str::FromStr;

use crate::error::{NtError, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoteKind {
    Note,
    Todo,
}

impl NoteKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Todo => "todo",
        }
    }
}

impl fmt::Display for NoteKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for NoteKind {
    type Err = NtError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "note" => Ok(Self::Note),
            "todo" => Ok(Self::Todo),
            _ => Err(NtError::Message(format!("invalid kind: {value}"))),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Open,
    Waiting,
    Done,
    Dropped,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Waiting => "waiting",
            Self::Done => "done",
            Self::Dropped => "dropped",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::Dropped)
    }
}

impl fmt::Display for Status {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Status {
    type Err = NtError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "open" => Ok(Self::Open),
            "waiting" => Ok(Self::Waiting),
            "done" => Ok(Self::Done),
            "dropped" => Ok(Self::Dropped),
            _ => Err(NtError::Message(format!("invalid status: {value}"))),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Priority {
    S,
    A,
    B,
    C,
    D,
}

impl Priority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::S => "S",
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::S => 0,
            Self::A => 1,
            Self::B => 2,
            Self::C => 3,
            Self::D => 4,
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Priority {
    type Err = NtError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "S" => Ok(Self::S),
            "A" => Ok(Self::A),
            "B" => Ok(Self::B),
            "C" => Ok(Self::C),
            "D" => Ok(Self::D),
            _ => Err(NtError::Message(format!(
                "invalid priority `{value}`; use S, A, B, C, or D"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NoteKind, Priority, Status};

    #[test]
    fn domain_enums_parse_only_canonical_values() {
        assert_eq!("todo".parse::<NoteKind>().unwrap(), NoteKind::Todo);
        assert_eq!("waiting".parse::<Status>().unwrap(), Status::Waiting);
        assert_eq!("A".parse::<Priority>().unwrap(), Priority::A);
        assert!("TODO".parse::<NoteKind>().is_err());
        assert!("paused".parse::<Status>().is_err());
        assert!("urgent".parse::<Priority>().is_err());
    }

    #[test]
    fn status_and_priority_expose_domain_behavior() {
        assert!(Status::Done.is_terminal());
        assert!(!Status::Open.is_terminal());
        assert_eq!(Priority::S.rank(), 0);
        assert_eq!(Priority::D.rank(), 4);
    }
}
