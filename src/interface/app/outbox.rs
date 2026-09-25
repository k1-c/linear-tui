//! Requests waiting for the main loop, and what is already on its way.

use std::collections::VecDeque;

use crate::usecase::Request;

#[derive(Debug, Default)]
pub struct Outbox {
    /// Requests queued by the UI, drained and spawned by the main loop.
    pub requests: VecDeque<Request>,
    /// Number of requests currently in flight.
    pub inflight: usize,
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
