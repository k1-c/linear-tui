//! Changing issues.

use super::Refusal;
use crate::api::ids::{IssueId, TeamId};
use crate::api::types::{Priority, User, WorkflowState};
use crate::message::Request;
use crate::store::Store;

/// Move an issue to `state`.
pub fn set_status(store: &mut Store, issue_id: &IssueId, state: WorkflowState) -> Request {
    let state_id = state.id.clone();
    store.patch_issue(issue_id, |i| i.state = Some(state.clone()));
    Request::UpdateStatus {
        issue_id: issue_id.clone(),
        state_id,
    }
}

pub fn set_priority(store: &mut Store, issue_id: &IssueId, priority: Priority) -> Request {
    store.patch_issue(issue_id, |i| {
        i.priority = priority;
        i.priority_label = Some(priority.label().to_string());
    });
    Request::UpdatePriority {
        issue_id: issue_id.clone(),
        priority,
    }
}

/// Assign an issue to `assignee`, or unassign it.
pub fn set_assignee(store: &mut Store, issue_id: &IssueId, assignee: Option<User>) -> Request {
    let assignee_id = assignee.as_ref().map(|u| u.id.clone());
    store.patch_issue(issue_id, |i| i.assignee = assignee.clone());
    Request::UpdateAssignee {
        issue_id: issue_id.clone(),
        assignee_id,
    }
}

/// Linear's `I`: assign an issue to the current user.
pub fn assign_to_me(store: &mut Store, issue_id: &IssueId) -> Result<Request, Refusal> {
    let viewer_id = store.viewer_id.clone().ok_or(Refusal::ViewerNotLoaded)?;
    // Until one of the viewer's teams has loaded, only their id is known and
    // the copies show no assignee.
    let me = store.viewer().cloned();
    store.patch_issue(issue_id, |i| i.assignee = me.clone());
    Ok(Request::UpdateAssignee {
        issue_id: issue_id.clone(),
        assignee_id: Some(viewer_id),
    })
}

/// Post a comment; an empty one is not sent.
pub fn comment(store: &mut Store, issue_id: &IssueId, body: String) -> Option<Request> {
    if body.is_empty() {
        return None;
    }
    // Drop the cached thread so the detail view refetches it once the
    // mutation lands.
    if let Some(current) = &mut store.current_issue
        && &current.id == issue_id
    {
        current.comments = None;
    }
    Some(Request::CreateComment {
        issue_id: issue_id.clone(),
        body,
    })
}

/// What a new issue is filed with.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    pub title: String,
    pub description: String,
    pub priority: Priority,
}

pub fn create(team_id: TeamId, draft: Draft) -> Result<Request, Refusal> {
    if draft.title.is_empty() {
        return Err(Refusal::TitleRequired);
    }
    Ok(Request::CreateIssue {
        team_id,
        title: draft.title,
        description: Some(draft.description).filter(|d| !d.is_empty()),
        priority: draft.priority,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ids::UserId;
    use crate::store::{IssueSource, TeamContext};

    fn store_with_issue() -> Store {
        let mut store = Store::default();
        store.issues[IssueSource::Team].items = vec![
            serde_json::from_str(r#"{"id":"i1","identifier":"ENG-1","title":"t","priority":0}"#)
                .unwrap(),
        ];
        store
    }

    fn user(id: &str) -> User {
        serde_json::from_str(&format!(r#"{{"id":"{id}","name":"{id}"}}"#)).unwrap()
    }

    fn id() -> IssueId {
        IssueId::from("i1")
    }

    #[test]
    fn a_status_change_patches_the_issue_and_asks_linear() {
        let mut store = store_with_issue();
        let state: WorkflowState =
            serde_json::from_str(r#"{"id":"s","name":"Done","type":"completed","position":1}"#)
                .unwrap();
        let request = set_status(&mut store, &id(), state);
        assert_eq!(
            request,
            Request::UpdateStatus {
                issue_id: id(),
                state_id: "s".into()
            }
        );
        assert_eq!(
            store.issue(&id()).unwrap().state.as_ref().unwrap().name,
            "Done"
        );
    }

    #[test]
    fn a_priority_change_updates_the_label_too() {
        let mut store = store_with_issue();
        set_priority(&mut store, &id(), Priority::High);
        let issue = store.issue(&id()).unwrap();
        assert_eq!(issue.priority, Priority::High);
        assert_eq!(
            issue.priority_label.as_deref(),
            Some(Priority::High.label())
        );
    }

    #[test]
    fn unassigning_clears_the_assignee() {
        let mut store = store_with_issue();
        set_assignee(&mut store, &id(), Some(user("u")));
        let request = set_assignee(&mut store, &id(), None);
        assert_eq!(
            request,
            Request::UpdateAssignee {
                issue_id: id(),
                assignee_id: None
            }
        );
        assert!(store.issue(&id()).unwrap().assignee.is_none());
    }

    #[test]
    fn assign_to_me_needs_the_viewer() {
        let mut store = store_with_issue();
        assert_eq!(
            assign_to_me(&mut store, &id()),
            Err(Refusal::ViewerNotLoaded)
        );

        store.viewer_id = Some(UserId::from("me"));
        store.team_contexts.insert(
            TeamId::from("t"),
            TeamContext {
                states: Vec::new(),
                members: vec![user("me")],
            },
        );
        assert!(assign_to_me(&mut store, &id()).is_ok());
        assert_eq!(
            store.issue(&id()).unwrap().assignee.as_ref().unwrap().name,
            "me"
        );
    }

    #[test]
    fn an_empty_comment_is_not_sent() {
        let mut store = store_with_issue();
        assert_eq!(comment(&mut store, &id(), String::new()), None);
        assert!(comment(&mut store, &id(), "hi".into()).is_some());
    }

    #[test]
    fn a_new_issue_needs_a_title_and_drops_an_empty_description() {
        let draft = |title: &str| Draft {
            title: title.into(),
            description: String::new(),
            priority: Priority::None,
        };
        assert_eq!(
            create(TeamId::from("t"), draft("")),
            Err(Refusal::TitleRequired)
        );
        let Ok(Request::CreateIssue { description, .. }) = create(TeamId::from("t"), draft("x"))
        else {
            panic!("expected a create request");
        };
        assert_eq!(description, None);
    }
}
