//! Typed identifiers for Linear entities.
//!
//! Linear hands out every id as an opaque string, so a team id and an issue id
//! look identical and one can be passed where the other is meant without the
//! compiler noticing. Each entity gets its own newtype instead.
//!
//! Only ids are wrapped. Names, keys, identifiers like `ENG-123`, and values a
//! workspace defines for itself (state names, favorite kinds, project states)
//! stay plain strings: they are data to show, not handles to pass back.

use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
        $(#[$doc])*
        #[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            // Generated for every id; not every one is read back as text.
            #[allow(dead_code)]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(id: &str) -> Self {
                Self::new(id)
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }
    )*};
}

id_type!(
    IssueId,
    TeamId,
    UserId,
    WorkflowStateId,
    ProjectId,
    CycleId,
    CustomViewId,
    FavoriteId,
    LabelId,
    CommentId,
    MilestoneId,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_deserializes_from_and_serializes_to_a_bare_string() {
        let id: IssueId = serde_json::from_str(r#""abc""#).unwrap();
        assert_eq!(id, "abc");
        assert_eq!(serde_json::to_string(&id).unwrap(), r#""abc""#);
        assert_eq!(id.to_string(), "abc");
    }
}
