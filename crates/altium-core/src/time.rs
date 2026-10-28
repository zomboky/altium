//! Timestamps and deadlines as stored in `.altium/`.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, SubsecRound, Utc};

/// A UTC instant with second precision, stored as `2026-10-28T14:55:00Z`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    pub fn now() -> Self {
        Self::from_datetime(Utc::now())
    }

    pub fn from_datetime(at: DateTime<Utc>) -> Self {
        Self(at.trunc_subsecs(0))
    }

    pub fn as_datetime(self) -> DateTime<Utc> {
        self.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.format("%Y-%m-%dT%H:%M:%SZ"))
    }
}

impl FromStr for Timestamp {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        DateTime::parse_from_rfc3339(s)
            .map(|at| Self::from_datetime(at.with_timezone(&Utc)))
            .map_err(|e| format!("invalid timestamp `{s}`: {e}"))
    }
}

serde_via_str!(Timestamp);

/// A task deadline in the user's local time zone: a date, optionally with a
/// time of day. Stored as `2026-11-02` or `2026-11-02T18:00`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Deadline {
    pub date: NaiveDate,
    pub time: Option<NaiveTime>,
}

impl fmt::Display for Deadline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.date.format("%Y-%m-%d"))?;
        if let Some(time) = self.time {
            write!(f, "T{}", time.format("%H:%M"))?;
        }
        Ok(())
    }
}

impl FromStr for Deadline {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parsed = if s.contains('T') {
            NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M").map(|at| Deadline {
                date: at.date(),
                time: Some(at.time()),
            })
        } else {
            NaiveDate::parse_from_str(s, "%Y-%m-%d").map(|date| Deadline { date, time: None })
        };
        parsed.map_err(|e| format!("invalid deadline `{s}` (expected YYYY-MM-DD or YYYY-MM-DDTHH:MM): {e}"))
    }
}

serde_via_str!(Deadline);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_drops_subseconds() {
        let ts: Timestamp = "2026-10-28T14:55:07.123+01:00".parse().unwrap();
        assert_eq!(ts.to_string(), "2026-10-28T13:55:07Z");
    }

    #[test]
    fn deadline_round_trips_with_and_without_time() {
        for text in ["2026-11-02", "2026-11-02T18:00"] {
            assert_eq!(text.parse::<Deadline>().unwrap().to_string(), text);
        }
        assert!("02/11/2026".parse::<Deadline>().is_err());
    }
}
