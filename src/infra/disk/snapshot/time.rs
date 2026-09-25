//! The clock, in the shape snapshots write it
//! ([`crate::core::entity::timestamp`]).

use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since the Unix epoch, now.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub fn timestamp_now() -> String {
    crate::core::entity::timestamp::format_timestamp(unix_now())
}
