// Версия WS-протокола Kepler ↔ Electron-апки.
//
// Семантика:
//   MAJOR — breaking wire changes. Клиенты с другим MAJOR должны отказаться коннектиться
//           и показать toast «Update Kepler/app».
//   MINOR — additive, forward-compatible. При mismatch — warning в лог, работаем дальше.
//   PATCH — bugfix без wire impact. Ignored при сравнении.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROTOCOL_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl ProtocolVersion {
    pub const CURRENT: ProtocolVersion = ProtocolVersion {
        major: 1,
        minor: 0,
        patch: 0,
    };

    pub fn parse(s: &str) -> Result<Self, ParseError> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(ParseError::WrongShape(s.to_string()));
        }
        let major = parts[0]
            .parse()
            .map_err(|_| ParseError::InvalidComponent(parts[0].to_string()))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| ParseError::InvalidComponent(parts[1].to_string()))?;
        let patch = parts[2]
            .parse()
            .map_err(|_| ParseError::InvalidComponent(parts[2].to_string()))?;
        Ok(ProtocolVersion {
            major,
            minor,
            patch,
        })
    }

    pub fn is_compatible_with_server(&self, server: &ProtocolVersion) -> Compatibility {
        if self.major != server.major {
            Compatibility::Incompatible
        } else if self.minor != server.minor {
            Compatibility::MinorMismatch
        } else {
            Compatibility::Exact
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compatibility {
    Exact,
    MinorMismatch,
    Incompatible,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("expected MAJOR.MINOR.PATCH, got {0:?}")]
    WrongShape(String),
    #[error("invalid version component: {0:?}")]
    InvalidComponent(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_version() {
        let v = ProtocolVersion::parse("1.2.3").unwrap();
        assert_eq!(
            v,
            ProtocolVersion {
                major: 1,
                minor: 2,
                patch: 3,
            }
        );
    }

    #[test]
    fn rejects_wrong_shape() {
        assert!(matches!(
            ProtocolVersion::parse("1.2"),
            Err(ParseError::WrongShape(_))
        ));
        assert!(matches!(
            ProtocolVersion::parse("1.2.3.4"),
            Err(ParseError::WrongShape(_))
        ));
    }

    #[test]
    fn rejects_non_numeric_component() {
        assert!(matches!(
            ProtocolVersion::parse("v1.2.3"),
            Err(ParseError::InvalidComponent(_))
        ));
    }

    #[test]
    fn major_mismatch_is_incompatible() {
        let client = ProtocolVersion {
            major: 2,
            minor: 0,
            patch: 0,
        };
        assert_eq!(
            client.is_compatible_with_server(&ProtocolVersion::CURRENT),
            Compatibility::Incompatible
        );
    }

    #[test]
    fn minor_mismatch_is_warning() {
        let client = ProtocolVersion {
            major: 1,
            minor: 5,
            patch: 0,
        };
        assert_eq!(
            client.is_compatible_with_server(&ProtocolVersion::CURRENT),
            Compatibility::MinorMismatch
        );
    }

    #[test]
    fn patch_diff_still_exact() {
        let client = ProtocolVersion {
            major: 1,
            minor: 0,
            patch: 99,
        };
        assert_eq!(
            client.is_compatible_with_server(&ProtocolVersion::CURRENT),
            Compatibility::Exact
        );
    }

    #[test]
    fn protocol_version_const_parses() {
        let parsed = ProtocolVersion::parse(PROTOCOL_VERSION).unwrap();
        assert_eq!(parsed, ProtocolVersion::CURRENT);
    }
}
