//! herdr agents and the issues they work on, as the plugin reports them.
//! The rules are `usecase::agent`; this resolves which issue is meant.

use super::*;
use crate::entity::AgentLink;

impl App {
    /// Take the plugin's latest list of agents.
    pub fn set_agents(&mut self, agents: Vec<AgentLink>) {
        self.agents = agents;
    }

    /// The agent shown on `identifier`'s row and detail.
    pub fn agent_for(&self, identifier: &str) -> Option<&AgentLink> {
        usecase::agent::agent_for(&self.agents, identifier)
    }

    /// `g w`: bring the pane of the agent working on the focused issue to
    /// the front.
    pub fn jump_to_agent(&mut self) {
        let Some(identifier) = self.focused_issue().map(|i| i.identifier.clone()) else {
            return;
        };
        let outcome = usecase::agent::jump(&self.agents, &identifier, self.herdr);
        self.run(outcome);
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
    fn g_w_jumps_to_the_agent_on_the_issue_under_the_cursor() {
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
            Some(&Request::Agent(crate::usecase::agent::Request::Focus {
                pane: "w1:p1".into()
            }))
        );
    }
}
