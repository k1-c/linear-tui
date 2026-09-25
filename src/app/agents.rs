//! herdr agents and the issues they work on, as the plugin reports them.

use super::*;
use crate::herdr::{AgentLink, Handoff};

impl App {
    /// Take the plugin's latest list of agents.
    pub fn set_agents(&mut self, agents: Vec<AgentLink>) {
        self.agents = agents;
    }

    /// The agent working on `identifier`: the one waiting for you first,
    /// then the one at work.
    pub fn agent_for(&self, identifier: &str) -> Option<&AgentLink> {
        self.agents
            .iter()
            .filter(|a| {
                a.issue
                    .as_deref()
                    .is_some_and(|i| i.eq_ignore_ascii_case(identifier))
            })
            .min_by_key(|a| a.status.rank())
    }

    /// `g w`: bring the pane of the agent working on the focused issue to
    /// the front.
    pub fn jump_to_agent(&mut self) {
        if !self.herdr {
            self.set_status("Jumping to an agent needs herdr");
            return;
        }
        let Some(identifier) = self.focused_issue().map(|i| i.identifier.clone()) else {
            return;
        };
        match self.agent_for(&identifier).map(|a| a.pane.clone()) {
            Some(pane) => self.request(Request::Herdr(Handoff::Focus { pane })),
            None => self.set_status(format!("No herdr agent is working on {identifier}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::herdr::parse_agents;

    fn app() -> App {
        let mut app = App::new(&Config::default());
        app.store.teams =
            vec![serde_json::from_str(r#"{"id":"t","name":"Engineering","key":"ENG"}"#).unwrap()];
        app.store.issues[IssueSource::Team].items = vec![
            serde_json::from_str(r#"{"id":"i1","identifier":"ENG-42","title":"t","priority":0}"#)
                .unwrap(),
        ];
        app.outbox.requests.clear();
        app
    }

    #[test]
    fn the_agent_that_needs_you_is_the_one_shown() {
        let mut app = app();
        app.set_agents(
            parse_agents(
                r#"{"version":1,"agents":[
                {"pane":"w1:p1","agent":"claude","status":"working","issue":"ENG-42"},
                {"pane":"w2:p1","agent":"codex","status":"blocked","issue":"eng-42"},
                {"pane":"w3:p1","agent":"claude","status":"idle"}]}"#,
            )
            .unwrap(),
        );
        assert_eq!(app.agent_for("ENG-42").unwrap().pane, "w2:p1");
        assert!(app.agent_for("ENG-7").is_none());
    }

    #[test]
    fn jumping_asks_the_plugin_to_focus_the_agents_pane() {
        let mut app = app();
        app.herdr = true;
        app.jump_to_agent();
        assert!(app.outbox.requests.is_empty());
        assert_eq!(
            app.view.status_message.as_deref(),
            Some("No herdr agent is working on ENG-42")
        );

        app.set_agents(
            parse_agents(
                r#"{"version":1,"agents":[{"pane":"w1:p1","agent":"claude","status":"working","issue":"ENG-42"}]}"#,
            )
            .unwrap(),
        );
        app.jump_to_agent();
        assert_eq!(
            app.outbox.requests.front(),
            Some(&Request::Herdr(Handoff::Focus {
                pane: "w1:p1".into()
            }))
        );
    }
}
