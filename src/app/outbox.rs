//! Requests waiting for the main loop, and what is already on its way.

use std::collections::{HashSet, VecDeque};

use crate::api::ids::TeamId;
use crate::message::Request;

#[derive(Debug, Default)]
pub struct Outbox {
    /// Requests queued by the UI, drained and spawned by the main loop.
    pub requests: VecDeque<Request>,
    /// Number of requests currently in flight.
    pub inflight: usize,
    /// Page cursors already requested. A next-page request stays in flight
    /// while the user keeps scrolling, and each step near the bottom would
    /// otherwise ask for the same page again — appending it once per step.
    pub prefetched: HashSet<String>,
    /// Teams whose context is in flight, so each one is asked for once.
    pub team_contexts: HashSet<TeamId>,
    /// Text the main loop should push to the system clipboard via OSC 52.
    pub clipboard: Option<String>,
}

impl Outbox {
    pub fn push(&mut self, req: Request) {
        // Collapse duplicates so a held-down key can't pile up identical fetches.
        if !self.requests.contains(&req) {
            self.requests.push_back(req);
        }
    }
}
