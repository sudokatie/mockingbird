//! Operating modes for mockingbird.

use serde::{Deserialize, Serialize};

/// Operating mode for HTTP interception.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Record real HTTP interactions to cassette.
    Record,
    
    /// Replay from cassette, fail if no match found.
    Replay,
    
    /// Replay if cassette exists, record otherwise.
    #[default]
    Auto,
    
    /// Pass through to real server, don't record.
    Passthrough,
}

impl Mode {
    /// Check if this mode records interactions.
    pub fn records(&self) -> bool {
        matches!(self, Mode::Record | Mode::Auto)
    }
    
    /// Check if this mode replays from cassette.
    pub fn replays(&self) -> bool {
        matches!(self, Mode::Replay | Mode::Auto)
    }
    
    /// Check if this mode allows real HTTP requests.
    pub fn allows_real_requests(&self) -> bool {
        matches!(self, Mode::Record | Mode::Auto | Mode::Passthrough)
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Record => write!(f, "record"),
            Mode::Replay => write!(f, "replay"),
            Mode::Auto => write!(f, "auto"),
            Mode::Passthrough => write!(f, "passthrough"),
        }
    }
}

impl std::str::FromStr for Mode {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "record" => Ok(Mode::Record),
            "replay" | "playback" => Ok(Mode::Replay),
            "auto" => Ok(Mode::Auto),
            "passthrough" | "pass" => Ok(Mode::Passthrough),
            _ => Err(format!("Invalid mode: {}. Use record, replay, auto, or passthrough", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_default() {
        assert_eq!(Mode::default(), Mode::Auto);
    }

    #[test]
    fn test_records() {
        assert!(Mode::Record.records());
        assert!(Mode::Auto.records());
        assert!(!Mode::Replay.records());
        assert!(!Mode::Passthrough.records());
    }

    #[test]
    fn test_replays() {
        assert!(Mode::Replay.replays());
        assert!(Mode::Auto.replays());
        assert!(!Mode::Record.replays());
        assert!(!Mode::Passthrough.replays());
    }

    #[test]
    fn test_allows_real_requests() {
        assert!(Mode::Record.allows_real_requests());
        assert!(Mode::Auto.allows_real_requests());
        assert!(Mode::Passthrough.allows_real_requests());
        assert!(!Mode::Replay.allows_real_requests());
    }

    #[test]
    fn test_display() {
        assert_eq!(Mode::Record.to_string(), "record");
        assert_eq!(Mode::Replay.to_string(), "replay");
        assert_eq!(Mode::Auto.to_string(), "auto");
        assert_eq!(Mode::Passthrough.to_string(), "passthrough");
    }

    #[test]
    fn test_from_str() {
        assert_eq!("record".parse::<Mode>().unwrap(), Mode::Record);
        assert_eq!("REPLAY".parse::<Mode>().unwrap(), Mode::Replay);
        assert_eq!("playback".parse::<Mode>().unwrap(), Mode::Replay);
        assert_eq!("PLAYBACK".parse::<Mode>().unwrap(), Mode::Replay);
        assert_eq!("Auto".parse::<Mode>().unwrap(), Mode::Auto);
        assert_eq!("passthrough".parse::<Mode>().unwrap(), Mode::Passthrough);
        assert_eq!("pass".parse::<Mode>().unwrap(), Mode::Passthrough);
    }

    #[test]
    fn test_from_str_invalid() {
        assert!("invalid".parse::<Mode>().is_err());
    }

    #[test]
    fn test_serde() {
        let mode = Mode::Record;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"record\"");
        
        let parsed: Mode = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Mode::Record);
    }
}
