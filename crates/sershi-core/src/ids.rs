//! Validated, namespaced identifiers for tools and permissions.
//!
//! Identifiers are dotted, lowercase paths such as `system.get_memory` or
//! `system.info.read`. Validation happens at construction and at
//! deserialization, so an identifier arriving over IPC (or, later, from a
//! skill manifest or a model's tool call) can never smuggle arbitrary text
//! into the registry or the audit log.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Longest identifier accepted. Generous for real names, small enough to
/// keep logs and UIs sane.
pub const MAX_ID_LEN: usize = 96;

#[derive(Debug, Error, PartialEq, Eq)]
#[error("invalid identifier `{value}`: expected dotted lowercase segments like `system.get_info`")]
pub struct InvalidId {
    value: String,
}

fn validate(value: &str) -> Result<(), InvalidId> {
    let segments_ok = || {
        let mut count = 0;
        for segment in value.split('.') {
            count += 1;
            let mut chars = segment.chars();
            let first_ok = chars.next().is_some_and(|c| c.is_ascii_lowercase());
            if !first_ok || !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                return false;
            }
        }
        count >= 2
    };
    if value.len() <= MAX_ID_LEN && segments_ok() {
        Ok(())
    } else {
        Err(InvalidId {
            // Truncate so a hostile payload cannot bloat error messages.
            value: value.chars().take(MAX_ID_LEN).collect(),
        })
    }
}

macro_rules! namespaced_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, type = "string"))]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, InvalidId> {
                let value = value.into();
                validate(&value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// The first segment, e.g. `system` for `system.get_info`.
            pub fn namespace(&self) -> &str {
                self.0.split('.').next().unwrap_or_default()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(deserializer)?;
                Self::new(raw).map_err(serde::de::Error::custom)
            }
        }
    };
}

namespaced_id!(
    /// Identifies a tool, e.g. `system.get_memory`.
    ToolId
);
namespaced_id!(
    /// Identifies a permission, e.g. `system.info.read`.
    PermissionId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_dotted_lowercase_ids() {
        for ok in [
            "system.get_info",
            "spotify.play",
            "system.info.read",
            "a1.b2",
        ] {
            assert!(ToolId::new(ok).is_ok(), "{ok} should be valid");
        }
    }

    #[test]
    fn rejects_malformed_or_hostile_ids() {
        for bad in [
            "",
            "system",
            "System.get_info",
            "system..get",
            ".system",
            "system.get info",
            "system.get;rm -rf",
            "system.../../etc",
            "1system.get",
            "system.ÿ",
        ] {
            assert!(ToolId::new(bad).is_err(), "{bad:?} should be rejected");
        }
        assert!(ToolId::new(format!("a.{}", "b".repeat(MAX_ID_LEN))).is_err());
    }

    #[test]
    fn deserialization_validates() {
        assert!(serde_json::from_str::<ToolId>("\"system.get_info\"").is_ok());
        assert!(serde_json::from_str::<ToolId>("\"rm -rf /\"").is_err());
    }

    #[test]
    fn exposes_namespace() {
        let id = PermissionId::new("system.info.read").unwrap();
        assert_eq!(id.namespace(), "system");
    }
}
