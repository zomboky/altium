//! Who made a change.

use std::fmt;
use std::str::FromStr;

/// Stored as `user`, `claude` or `claude:<session id>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Actor {
    /// The user, from the dashboard or the CLI.
    User,
    /// Claude through MCP, with its Claude Code session id when known.
    Claude { session: Option<String> },
}

impl fmt::Display for Actor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Actor::User => f.write_str("user"),
            Actor::Claude { session: None } => f.write_str("claude"),
            Actor::Claude { session: Some(id) } => write!(f, "claude:{id}"),
        }
    }
}

impl FromStr for Actor {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user" => Ok(Actor::User),
            "claude" => Ok(Actor::Claude { session: None }),
            _ => match s.strip_prefix("claude:") {
                Some(id) if !id.is_empty() => Ok(Actor::Claude {
                    session: Some(id.to_owned()),
                }),
                _ => Err(format!(
                    "invalid actor `{s}` (expected user, claude or claude:<session>)"
                )),
            },
        }
    }
}

serde_via_str!(Actor);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for text in ["user", "claude", "claude:4f2a-91"] {
            assert_eq!(text.parse::<Actor>().unwrap().to_string(), text);
        }
        assert!("claude:".parse::<Actor>().is_err());
        assert!("bot".parse::<Actor>().is_err());
    }
}
