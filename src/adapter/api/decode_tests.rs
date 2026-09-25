//! Decoding Linear's answers into the entity types: every field, and every
//! field Linear may leave out, against the fixtures in `tests/fixtures/`.

use serde::Deserialize;

use crate::entity::*;

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read fixture {path}: {e}"))
}

/// Helper: deserialize a fixture and extract the `data` wrapper that
/// the actual GraphQL client strips. Fixtures store the inner `data` object
/// directly (no `{"data": ...}` envelope) to match client.rs response types.

#[test]
fn deserialize_teams() {
    #[derive(Deserialize)]
    struct Resp {
        teams: Connection<Team>,
    }
    let resp: Resp = serde_json::from_str(&fixture("teams.json")).unwrap();
    assert_eq!(resp.teams.nodes.len(), 2);
    assert_eq!(resp.teams.nodes[0].key, "ENG");
}

#[test]
fn deserialize_issues_with_pagination() {
    #[derive(Deserialize)]
    struct Resp {
        issues: Connection<Issue>,
    }
    let resp: Resp = serde_json::from_str(&fixture("issues.json")).unwrap();
    assert_eq!(resp.issues.nodes.len(), 2);
    assert!(resp.issues.page_info.has_next_page);
    assert_eq!(
        resp.issues.page_info.end_cursor.as_deref(),
        Some("cursor-abc123")
    );
}

#[test]
fn deserialize_issue_url_and_branch_name() {
    #[derive(Deserialize)]
    struct Resp {
        issues: Connection<Issue>,
    }
    let resp: Resp = serde_json::from_str(&fixture("issues.json")).unwrap();
    let issue = &resp.issues.nodes[0];
    assert_eq!(
        issue.url.as_deref(),
        Some("https://linear.app/acme/issue/ENG-100")
    );
    assert_eq!(
        issue.branch_name.as_deref(),
        Some("test-user/eng-100-fix-login-bug")
    );
}

/// `url`/`branchName` are absent from older cached payloads; they must not
/// break deserialization.
#[test]
fn deserialize_issue_without_url_fields() {
    let issue: Issue = serde_json::from_str(
        r#"{"id":"i1","identifier":"ENG-1","title":"t","state":null,"assignee":null,
            "description":null,"comments":null,"project":null,"cycle":null}"#,
    )
    .unwrap();
    assert!(issue.url.is_none());
    assert!(issue.branch_name.is_none());
}

#[test]
fn deserialize_issue_with_null_fields() {
    #[derive(Deserialize)]
    struct Resp {
        issues: Connection<Issue>,
    }
    let resp: Resp = serde_json::from_str(&fixture("issues.json")).unwrap();
    let issue = &resp.issues.nodes[1];
    assert!(issue.assignee.is_none());
    assert!(issue.description.is_none());
}

#[test]
fn deserialize_canceled_state_type() {
    #[derive(Deserialize)]
    struct Resp {
        issues: Connection<Issue>,
    }
    let resp: Resp = serde_json::from_str(&fixture("issues.json")).unwrap();
    let state = resp.issues.nodes[1].state.as_ref().unwrap();
    assert_eq!(state.state_type, Some(StateType::Cancelled));
}

#[test]
fn deserialize_issue_detail_with_comments() {
    #[derive(Deserialize)]
    struct Resp {
        issue: Issue,
    }
    let resp: Resp = serde_json::from_str(&fixture("issue_detail.json")).unwrap();
    let comments = resp.issue.comments.as_ref().unwrap();
    assert_eq!(comments.nodes.len(), 1);
    assert_eq!(comments.nodes[0].body, "This is a comment");
    assert!(resp.issue.project.is_some());
    assert!(resp.issue.cycle.is_some());
}

#[test]
fn deserialize_workflow_states_all_types() {
    #[derive(Deserialize)]
    struct Resp {
        #[serde(rename = "workflowStates")]
        workflow_states: Connection<WorkflowState>,
    }
    let resp: Resp = serde_json::from_str(&fixture("workflow_states.json")).unwrap();
    let types: Vec<_> = resp
        .workflow_states
        .nodes
        .iter()
        .filter_map(|s| s.state_type)
        .collect();
    assert!(types.contains(&StateType::Started));
    assert!(types.contains(&StateType::Backlog));
    assert!(types.contains(&StateType::Cancelled));
    assert!(types.contains(&StateType::Unstarted));
    assert!(types.contains(&StateType::Completed));
    assert!(types.contains(&StateType::Triage));
    assert!(types.contains(&StateType::Duplicate));
    // A category this client does not know still parses, and a state
    // without one at all stays None.
    assert!(types.contains(&StateType::Unknown));
    assert_eq!(resp.workflow_states.nodes.len(), 9);
    assert!(
        resp.workflow_states
            .nodes
            .iter()
            .any(|s| s.state_type.is_none())
    );
}

#[test]
fn unknown_state_type_does_not_fail_the_query() {
    // Linear types `WorkflowState.type` as a String and has added values
    // over time. Rejecting an unfamiliar one would fail the whole response
    // and empty every list, so it degrades to Unknown instead.
    assert_eq!(
        serde_json::from_str::<StateType>("\"duplicate\"").unwrap(),
        StateType::Duplicate
    );
    assert_eq!(
        serde_json::from_str::<StateType>("\"somethingNewIn2027\"").unwrap(),
        StateType::Unknown
    );
}

#[test]
fn deserialize_team_members() {
    #[derive(Deserialize)]
    struct TeamResp {
        team: TeamWithMembers,
    }
    #[derive(Deserialize)]
    struct TeamWithMembers {
        members: Connection<User>,
    }
    let resp: TeamResp = serde_json::from_str(&fixture("team_members.json")).unwrap();
    assert_eq!(resp.team.members.nodes.len(), 2);
}

#[test]
fn deserialize_viewer() {
    #[derive(Deserialize)]
    struct Resp {
        viewer: Viewer,
    }
    let resp: Resp = serde_json::from_str(&fixture("viewer.json")).unwrap();
    assert_eq!(resp.viewer.name, "Test User");
    let org = resp.viewer.organization.unwrap();
    assert_eq!(org.name, "Acme");
    assert_eq!(org.url_key, "acme");
}

#[test]
fn deserialize_viewer_without_organization() {
    let viewer: Viewer =
        serde_json::from_str(r#"{ "id": "u1", "name": "Ada", "displayName": null }"#).unwrap();
    assert!(viewer.organization.is_none());
}

#[test]
fn deserialize_my_issues_with_float_priority() {
    #[derive(Deserialize)]
    struct Resp {
        issues: Connection<Issue>,
    }
    let resp: Resp = serde_json::from_str(&fixture("my_issues.json")).unwrap();
    // priority: 3.0 should deserialize to Medium
    assert_eq!(resp.issues.nodes[0].priority, Priority::Medium);
    // priority: 0.0 should deserialize to None
    assert_eq!(resp.issues.nodes[1].priority, Priority::None);
    // "canceled" state type
    let state = resp.issues.nodes[0].state.as_ref().unwrap();
    assert_eq!(state.state_type, Some(StateType::Cancelled));
    // Unicode description
    assert!(
        resp.issues.nodes[1]
            .description
            .as_ref()
            .unwrap()
            .contains("日本語")
    );
}

/// My Issues spans teams, so each row says which team it belongs to. A
/// payload without `team` (an older cache, a test helper) still loads.
#[test]
fn deserialize_issue_team() {
    #[derive(Deserialize)]
    struct Resp {
        issues: Connection<Issue>,
    }
    let resp: Resp = serde_json::from_str(&fixture("my_issues.json")).unwrap();
    let team = resp.issues.nodes[0].team.as_ref().unwrap();
    assert_eq!(team.id, "team-001");
    assert!(resp.issues.nodes[1].team.is_none());
}

#[test]
fn deserialize_projects() {
    #[derive(Deserialize)]
    struct TeamResp {
        team: TeamWithProjects,
    }
    #[derive(Deserialize)]
    struct TeamWithProjects {
        projects: Connection<Project>,
    }
    let resp: TeamResp = serde_json::from_str(&fixture("projects.json")).unwrap();
    assert_eq!(resp.team.projects.nodes.len(), 2);
    assert!(resp.team.projects.nodes[0].lead.is_some());
    assert!(resp.team.projects.nodes[1].lead.is_none());
    assert!(resp.team.projects.nodes[1].start_date.is_none());
}

#[test]
fn deserialize_cycles() {
    #[derive(Deserialize)]
    struct TeamResp {
        team: TeamWithCycles,
    }
    #[derive(Deserialize)]
    struct TeamWithCycles {
        cycles: Connection<Cycle>,
    }
    let resp: TeamResp = serde_json::from_str(&fixture("cycles.json")).unwrap();
    assert_eq!(resp.team.cycles.nodes.len(), 2);
    assert_eq!(resp.team.cycles.nodes[0].name.as_deref(), Some("Sprint 1"));
    assert!(resp.team.cycles.nodes[1].name.is_none());
}

#[test]
fn deserialize_priority_edge_cases() {
    // Integer values (Linear sometimes returns int instead of float)
    assert_eq!(
        serde_json::from_str::<Priority>("0").unwrap(),
        Priority::None
    );
    assert_eq!(
        serde_json::from_str::<Priority>("1").unwrap(),
        Priority::Urgent
    );
    assert_eq!(
        serde_json::from_str::<Priority>("4").unwrap(),
        Priority::Low
    );
    // Float values
    assert_eq!(
        serde_json::from_str::<Priority>("2.0").unwrap(),
        Priority::High
    );
    // Out of range
    assert_eq!(
        serde_json::from_str::<Priority>("99").unwrap(),
        Priority::None
    );
    assert_eq!(
        serde_json::from_str::<Priority>("2.5").unwrap(),
        Priority::None,
        "a fraction is not rounded into a level"
    );
}

#[test]
fn deserialize_state_type_both_spellings() {
    assert_eq!(
        serde_json::from_str::<StateType>("\"canceled\"").unwrap(),
        StateType::Cancelled
    );
    assert_eq!(
        serde_json::from_str::<StateType>("\"cancelled\"").unwrap(),
        StateType::Cancelled
    );
}

#[test]
fn deserialize_issue_detail_metadata() {
    #[derive(Deserialize)]
    struct Resp {
        issue: Issue,
    }
    let resp: Resp = serde_json::from_str(&fixture("issue_detail_full.json")).unwrap();
    let issue = resp.issue;
    assert_eq!(issue.creator.as_ref().unwrap().name, "Other Person");
    assert_eq!(issue.estimate, Some(3.0));
    assert_eq!(issue.due_date.as_deref(), Some("2026-10-01"));
    assert_eq!(issue.parent.as_ref().unwrap().identifier, "ENG-151");
    assert_eq!(issue.children.as_ref().unwrap().nodes.len(), 1);
    assert_eq!(issue.project_milestone.as_ref().unwrap().name, "Beta");
    assert_eq!(issue.state.as_ref().unwrap().position, Some(2.0));
    let comments = &issue.comments.as_ref().unwrap().nodes;
    assert!(comments[0].parent.is_none());
    assert_eq!(comments[1].parent.as_ref().unwrap().id, "comment-001");
    assert!(comments[1].edited_at.is_some());
}

/// Every field added for the richer detail view is optional: older
/// fixtures and trimmed queries must still parse.
#[test]
fn deserialize_issue_without_the_new_metadata() {
    #[derive(Deserialize)]
    struct Resp {
        issue: Issue,
    }
    let resp: Resp = serde_json::from_str(&fixture("issue_detail.json")).unwrap();
    let issue = resp.issue;
    assert!(issue.creator.is_none());
    assert!(issue.estimate.is_none());
    assert!(issue.due_date.is_none());
    assert!(issue.parent.is_none());
    assert!(issue.children.is_none());
    assert!(issue.project_milestone.is_none());
    assert!(issue.comments.unwrap().nodes[0].parent.is_none());
}

#[test]
fn deserialize_custom_views() {
    #[derive(Deserialize)]
    struct Resp {
        #[serde(rename = "customViews")]
        custom_views: Connection<CustomView>,
    }
    let resp: Resp = serde_json::from_str(&fixture("custom_views.json")).unwrap();
    let views = resp.custom_views.nodes;
    assert_eq!(views.len(), 3);
    assert!(!views[0].shared);
    assert!(views[1].shared);
    assert!(views[0].team.is_none());
    assert!(!views[1].team.as_ref().unwrap().cycles_enabled);
    assert!(views[1].description.is_none());
    assert!(views[0].lists_issues());
    assert!(!views[1].lists_issues(), "a project view");
    assert!(views[2].lists_issues(), "no modelName: an old issue view");
}

#[test]
fn deserialize_view_issues_with_pagination() {
    #[derive(Deserialize)]
    struct Resp {
        #[serde(rename = "customView")]
        custom_view: ViewIssues,
    }
    #[derive(Deserialize)]
    struct ViewIssues {
        issues: Connection<Issue>,
    }
    let resp: Resp = serde_json::from_str(&fixture("view_issues.json")).unwrap();
    assert_eq!(resp.custom_view.issues.nodes.len(), 1);
    assert_eq!(
        resp.custom_view.issues.page_info.end_cursor.as_deref(),
        Some("view-cursor-1")
    );
}

/// A team payload without `color`/`cyclesEnabled` still parses, and
/// assumes cycles are on so the sidebar does not hide a real page.
#[test]
fn a_team_without_the_new_fields_defaults_sensibly() {
    #[derive(Deserialize)]
    struct Resp {
        teams: Connection<Team>,
    }
    let resp: Resp = serde_json::from_str(&fixture("teams.json")).unwrap();
    assert!(resp.teams.nodes[0].color.is_none());
    assert!(resp.teams.nodes[0].cycles_enabled);
}

#[test]
fn deserialize_favorites() {
    #[derive(Deserialize)]
    struct Resp {
        favorites: Connection<Favorite>,
    }
    let resp: Resp = serde_json::from_str(&fixture("favorites.json")).unwrap();
    let favs = resp.favorites.nodes;
    assert_eq!(favs.len(), 6);
    assert_eq!(favs[0].kind, "project");
    assert_eq!(favs[0].project.as_ref().unwrap().name, "Q1 Release");
    assert_eq!(favs[1].custom_view.as_ref().unwrap().id, "view-001");
    assert!(favs[2].is_folder());
    assert_eq!(favs[2].label(), "Ops");
    assert_eq!(favs[3].parent.as_ref().unwrap().id, "fav-3");
    assert_eq!(favs[3].issue.as_ref().unwrap().identifier, "ENG-100");
    assert_eq!(favs[4].predefined_view_type.as_deref(), Some("projects"));
    // A kind this client knows nothing about still parses and has a name.
    assert_eq!(favs[5].label(), "Onboarding doc");
    assert!(favs[5].title.is_some());
}
