//! Teams: which one linear-tui shows, switching to another, and each team's
//! workflow states and members.
//!
//! [`pick_initial`] chooses the team at startup and [`switch`] moves to
//! another. [`ensure_context`] fetches a team's states and members — what
//! its issues can be moved to and assigned to — and [`context_arrived`] /
//! [`context_failed`] follow the answer. [`load`] asks for the teams.

use crate::core::entity::{Team, TeamId, User, WorkflowState};
use crate::core::store::{IssueSource, Store, TeamContext};

/// What the team use cases ask of Linear.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Every team the user belongs to.
    Teams,
    /// A team's workflow states and members.
    Context { team_id: TeamId },
}

/// **Know the teams** the user belongs to. Asked for as linear-tui starts;
/// every team page waits on the answer.
pub fn load() -> Request {
    Request::Teams
}

/// **Which team linear-tui opens on.**
///
/// The team the remembered view was on comes first, then the team named in
/// the config (by name or key), then the first team. `None` means the first.
pub fn pick_initial(
    teams: &[Team],
    remembered: Option<&TeamId>,
    configured: Option<&str>,
) -> Option<usize> {
    remembered
        .and_then(|id| teams.iter().position(|t| &t.id == id))
        .or_else(|| {
            let wanted = configured?;
            teams
                .iter()
                .position(|t| t.name == wanted || t.key == wanted)
        })
}

/// **Switch team.**
///
/// Everything scoped to the old team goes: its issues, projects, and cycles
/// are dropped, so none is shown under the new team's name, and the page
/// cursors asked for are forgotten. The lists that hold issues of any team
/// — mine, a view's, a project's, a cycle's — are fetched afresh when next
/// opened. The new team's states and members are asked for.
pub fn switch(store: &mut Store, team_id: TeamId) -> Option<Request> {
    store.issues[IssueSource::Team].reset();
    store.projects.reset();
    store.cycles.reset();
    store.forget_pages();
    for source in [
        IssueSource::My,
        IssueSource::View,
        IssueSource::Project,
        IssueSource::Cycle,
    ] {
        store.issues[source].loaded = false;
    }
    ensure_context(store, team_id)
}

/// **Know a team's workflow states and members.**
///
/// Asked for once: not again once known, and not again while the answer is
/// on its way.
pub fn ensure_context(store: &mut Store, team_id: TeamId) -> Option<Request> {
    if store.team_contexts.contains_key(&team_id)
        || !store.requested.team_contexts.insert(team_id.clone())
    {
        return None;
    }
    Some(Request::Context { team_id })
}

/// **A team's states and members arrive**, and are kept for its issues,
/// whichever team is selected by then.
pub fn context_arrived(
    store: &mut Store,
    team_id: TeamId,
    states: Vec<WorkflowState>,
    members: Vec<User>,
) {
    store.requested.team_contexts.remove(&team_id);
    store
        .team_contexts
        .insert(team_id, TeamContext { states, members });
}

/// **Linear could not answer for a team**; the next ask goes through.
pub fn context_failed(store: &mut Store, team_id: &TeamId) {
    store.requested.team_contexts.remove(team_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::entity::Preset;
    use crate::core::usecase::issue;

    fn team(id: &str, name: &str, key: &str) -> Team {
        serde_json::from_str(&format!(r#"{{"id":"{id}","name":"{name}","key":"{key}"}}"#)).unwrap()
    }

    fn teams() -> Vec<Team> {
        vec![team("t1", "Core", "COR"), team("t2", "Design", "DES")]
    }

    /// The teams are asked of Linear.
    #[test]
    fn the_teams_are_asked_for() {
        assert_eq!(load(), Request::Teams);
    }

    /// The remembered team wins over the configured one.
    #[test]
    fn the_remembered_team_comes_first() {
        let t2 = TeamId::from("t2");
        assert_eq!(pick_initial(&teams(), Some(&t2), Some("Core")), Some(1));
    }

    /// The configured team is found by its name or its key.
    #[test]
    fn the_configured_team_is_found_by_name_or_key() {
        assert_eq!(pick_initial(&teams(), None, Some("Design")), Some(1));
        assert_eq!(pick_initial(&teams(), None, Some("DES")), Some(1));
    }

    /// With neither, or neither found, linear-tui opens on the first team.
    #[test]
    fn otherwise_the_first_team() {
        assert_eq!(pick_initial(&teams(), None, None), None);
        let gone = TeamId::from("gone");
        assert_eq!(pick_initial(&teams(), Some(&gone), Some("Nope")), None);
    }

    /// Switching drops the old team's issues, projects, and cycles, and asks
    /// for the new team's states and members.
    #[test]
    fn switching_drops_the_old_teams_lists() {
        let mut store = Store::default();
        issue::open_team_issues(&mut store, TeamId::from("t1"), Preset::Active);
        store.issues[IssueSource::Team].items =
            vec![serde_json::from_str(r#"{"id":"i","identifier":"COR-1","title":"t"}"#).unwrap()];
        store.requested.cursors.insert("c1".into());
        let request = switch(&mut store, TeamId::from("t2"));
        assert_eq!(
            request,
            Some(Request::Context {
                team_id: "t2".into()
            })
        );
        assert!(store.issues[IssueSource::Team].items.is_empty());
        assert!(store.issues[IssueSource::Team].of.is_none());
        assert!(store.requested.cursors.is_empty());
    }

    /// A team's states and members are asked for once while on their way,
    /// and not at all once known.
    #[test]
    fn a_teams_context_is_asked_for_once() {
        let mut store = Store::default();
        let t = TeamId::from("t");
        assert!(ensure_context(&mut store, t.clone()).is_some());
        assert_eq!(ensure_context(&mut store, t.clone()), None);
        context_arrived(&mut store, t.clone(), Vec::new(), Vec::new());
        assert_eq!(ensure_context(&mut store, t), None);
    }

    /// After a failure the team's context is asked for again.
    #[test]
    fn a_failed_context_is_asked_for_again() {
        let mut store = Store::default();
        let t = TeamId::from("t");
        ensure_context(&mut store, t.clone());
        context_failed(&mut store, &t);
        assert!(ensure_context(&mut store, t).is_some());
    }
}
