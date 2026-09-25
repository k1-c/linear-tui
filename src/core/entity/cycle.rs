//! A cycle: a team's time-boxed iteration.

use serde::Deserialize;

use super::ids::*;
use super::issue::Issue;
use super::page::Connection;

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Cycle {
    pub id: CycleId,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub number: Option<f64>,
    #[serde(default, rename = "startsAt")]
    pub starts_at: Option<String>,
    #[serde(default, rename = "endsAt")]
    pub ends_at: Option<String>,
    #[serde(default)]
    pub progress: Option<f64>,
    pub issues: Option<Connection<Issue>>,
}

impl Cycle {
    /// What a cycle is called: its name, or `Cycle 12` for an unnamed one.
    pub fn label(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| format!("Cycle {}", self.number.unwrap_or(0.0)))
    }
}
