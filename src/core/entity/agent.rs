//! Coding agents beside linear-tui, as herdr's plugin reports them, and
//! what linear-tui hands to one. Talking to herdr is `crate::infra::herdr`'s job.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A herdr agent, and the issue it works on.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AgentLink {
    pub pane: String,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub workspace_label: Option<String>,
    /// What herdr detected: `claude`, `codex`, …
    pub agent: String,
    pub status: AgentStatus,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    /// `ENG-42`, read by the plugin from the checkout's branch name.
    #[serde(default)]
    pub issue: Option<String>,
}

/// herdr's agent states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Working,
    Blocked,
    Idle,
    Done,
    #[serde(other)]
    Unknown,
}

impl AgentStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Blocked => "waiting for you",
            Self::Idle => "idle",
            Self::Done => "done",
            Self::Unknown => "unknown",
        }
    }

    /// Which of several agents on one issue to show: the one that needs you,
    /// then the one at work.
    pub fn rank(self) -> u8 {
        match self {
            Self::Blocked => 0,
            Self::Working => 1,
            Self::Done => 2,
            Self::Idle => 3,
            Self::Unknown => 4,
        }
    }
}

/// What linear-tui asks the plugin to do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Handoff {
    /// Send the user's notes to the agent working next to linear-tui.
    Prompt {
        /// The whole prompt, ready to send as it is.
        text: String,
        /// Its parts, for a prompt template: the notes as a Markdown list,
        /// where the user is, and how to read more.
        notes: String,
        view: String,
        hint: String,
    },
    /// Bring an agent's pane to the front.
    Focus { pane: String },
}

impl Handoff {
    pub fn done(&self) -> &'static str {
        match self {
            Self::Prompt { .. } => "Notes handed to herdr for your agent",
            Self::Focus { .. } => "Switched to the agent",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Of several agents on one issue, the one waiting for the user shows
    /// first, then the one at work; an unknown status shows last.
    #[test]
    fn an_agent_waiting_for_you_shows_before_one_at_work() {
        let order = [
            AgentStatus::Blocked,
            AgentStatus::Working,
            AgentStatus::Done,
            AgentStatus::Idle,
            AgentStatus::Unknown,
        ];
        assert!(order.windows(2).all(|w| w[0].rank() < w[1].rank()));
    }

    /// Each status reads as a person would say it: a blocked agent is
    /// waiting for you.
    #[test]
    fn each_status_reads_as_a_person_would_say_it() {
        let labels = [
            AgentStatus::Working,
            AgentStatus::Blocked,
            AgentStatus::Idle,
            AgentStatus::Done,
            AgentStatus::Unknown,
        ]
        .map(AgentStatus::label);
        assert_eq!(
            labels,
            ["working", "waiting for you", "idle", "done", "unknown"]
        );
    }

    /// A hand-off says what it did once herdr took it.
    #[test]
    fn a_handoff_says_what_it_did() {
        let prompt = Handoff::Prompt {
            text: String::new(),
            notes: String::new(),
            view: String::new(),
            hint: String::new(),
        };
        assert_eq!(prompt.done(), "Notes handed to herdr for your agent");
        let focus = Handoff::Focus { pane: "p".into() };
        assert_eq!(focus.done(), "Switched to the agent");
    }
}
