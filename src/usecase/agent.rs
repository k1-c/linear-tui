//! Coding agents working on issues, as herdr's plugin reports them.
//!
//! [`agent_for`] says which agent works on an issue; [`jump`] brings its
//! pane to the front.

use super::Refusal;
use crate::entity::AgentLink;

/// What the agent use cases ask of herdr.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Bring an agent's pane to the front.
    Focus { pane: String },
}

/// **See which agent works on an issue.**
///
/// An agent works on the issue its checkout's branch names, whatever case
/// the branch spells the identifier in. When several agents work on one
/// issue, the one shown is the one that needs the user most: an agent
/// waiting for an answer comes before one at work, which comes before one
/// that is done or idle.
pub fn agent_for<'a>(agents: &'a [AgentLink], identifier: &str) -> Option<&'a AgentLink> {
    agents
        .iter()
        .filter(|a| {
            a.issue
                .as_deref()
                .is_some_and(|i| i.eq_ignore_ascii_case(identifier))
        })
        .min_by_key(|a| a.status.rank())
}

/// **Jump to the agent working on an issue** (`g w`).
///
/// herdr brings that agent's pane to the front. This only works inside
/// herdr, and only when an agent works on the issue; otherwise the user is
/// told which of the two is missing.
pub fn jump(agents: &[AgentLink], identifier: &str, herdr: bool) -> Result<Request, Refusal> {
    if !herdr {
        return Err(Refusal::NeedsHerdr);
    }
    let agent =
        agent_for(agents, identifier).ok_or_else(|| Refusal::NoAgentOn(identifier.into()))?;
    Ok(Request::Focus {
        pane: agent.pane.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three agents: one at work on ENG-42, one waiting for an answer on it
    /// (with the identifier in lower case), and one on no issue.
    fn agents() -> Vec<AgentLink> {
        serde_json::from_str(
            r#"[
            {"pane":"w1:p1","agent":"claude","status":"working","issue":"ENG-42"},
            {"pane":"w2:p1","agent":"codex","status":"blocked","issue":"eng-42"},
            {"pane":"w3:p1","agent":"claude","status":"idle"}]"#,
        )
        .unwrap()
    }

    /// Of two agents on one issue, the one waiting for the user is shown.
    #[test]
    fn the_agent_waiting_for_you_is_shown_before_the_one_at_work() {
        assert_eq!(agent_for(&agents(), "ENG-42").unwrap().pane, "w2:p1");
    }

    /// An issue no agent names has no agent.
    #[test]
    fn an_issue_nobody_works_on_has_no_agent() {
        assert!(agent_for(&agents(), "ENG-7").is_none());
    }

    /// Jumping asks herdr to focus the pane of the agent shown.
    #[test]
    fn jumping_focuses_that_agents_pane() {
        assert_eq!(
            jump(&agents(), "ENG-42", true),
            Ok(Request::Focus {
                pane: "w2:p1".into()
            })
        );
    }

    /// With no agent on the issue, the user is told so and nothing is sent.
    #[test]
    fn jumping_to_an_issue_nobody_works_on_says_so() {
        assert_eq!(
            jump(&agents(), "ENG-7", true),
            Err(Refusal::NoAgentOn("ENG-7".into()))
        );
    }

    /// Outside herdr there are no panes to jump to.
    #[test]
    fn jumping_needs_herdr() {
        assert_eq!(jump(&agents(), "ENG-42", false), Err(Refusal::NeedsHerdr));
    }
}
