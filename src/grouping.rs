//! How an issue list is sliced and stacked before it reaches the screen.
//!
//! Linear never shows a flat list of issues: a view is a preset (Active,
//! Backlog, All) crossed with a grouping (by status, by assignee, …), and the
//! result is a stack of labelled sections you can collapse. This module is that
//! transformation, kept apart from [`crate::app`] so it can be exercised on
//! plain slices.

use std::collections::{HashMap, HashSet};

use ratatui::style::Color;

use crate::api::ids::IssueId;
use crate::api::types::{Issue, Priority, StateType, hex_color};
use crate::config::{GroupByName, Theme};

/// Which axis a list stacks its sections along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    Status,
    Assignee,
    Priority,
    Project,
    None,
}

impl GroupBy {
    pub fn from_config(name: GroupByName) -> Self {
        match name {
            GroupByName::Status => Self::Status,
            GroupByName::Assignee => Self::Assignee,
            GroupByName::Priority => Self::Priority,
            GroupByName::Project => Self::Project,
            GroupByName::None => Self::None,
        }
    }

    /// Every grouping, in the order the picker lists them.
    pub const ALL: [GroupBy; 5] = [
        Self::Status,
        Self::Assignee,
        Self::Priority,
        Self::Project,
        Self::None,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Status => "Status",
            Self::Assignee => "Assignee",
            Self::Priority => "Priority",
            Self::Project => "Project",
            Self::None => "None",
        }
    }

    /// Cycle order for the key that toggles grouping.
    pub fn next(self) -> Self {
        match self {
            Self::Status => Self::Assignee,
            Self::Assignee => Self::Priority,
            Self::Priority => Self::Project,
            Self::Project => Self::None,
            Self::None => Self::Status,
        }
    }
}

/// Linear's three standing slices of a team's issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Preset {
    #[default]
    Active,
    Backlog,
    All,
}

impl Preset {
    pub fn all() -> &'static [Preset] {
        &[Preset::Active, Preset::Backlog, Preset::All]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Backlog => "Backlog",
            Self::All => "All issues",
        }
    }

    /// The `state` clause of an `IssueFilter` that selects this slice on the
    /// server, or `None` for no restriction.
    ///
    /// Active is written as "none of the inactive categories" rather than "one
    /// of started/unstarted", so an issue in a category Linear adds later still
    /// comes back — the same fallback [`Self::admits`] makes on the client.
    pub fn state_filter(&self) -> Option<serde_json::Value> {
        match self {
            Self::All => None,
            Self::Active => Some(serde_json::json!({
                "type": { "nin": ["backlog", "triage", "completed", "canceled", "duplicate"] }
            })),
            Self::Backlog => Some(serde_json::json!({
                "type": { "in": ["backlog", "triage"] }
            })),
        }
    }

    /// Whether an issue belongs in this slice.
    ///
    /// An issue whose state category this client does not recognise counts as
    /// active: a new Linear category should surface somewhere obvious rather
    /// than vanish from the default view.
    pub fn admits(&self, issue: &Issue) -> bool {
        let state_type = issue.state.as_ref().and_then(|s| s.state_type);
        match self {
            Self::All => true,
            Self::Active => match state_type {
                Some(t) => t.is_active() || t == StateType::Unknown,
                None => true,
            },
            Self::Backlog => state_type.is_some_and(|t| t.is_backlog()),
        }
    }
}

/// One labelled stack of issues.
pub struct Section<'a> {
    /// Stable identity, used as the key for collapse state so that reordering
    /// or a refetch does not silently re-expand what the user folded away.
    pub key: String,
    pub label: String,
    pub color: Color,
    pub glyph: &'static str,
    pub collapsed: bool,
    /// Issues in display order, each with the indent depth of a sub-issue.
    pub issues: Vec<(&'a Issue, u8)>,
}

/// Sort key that puts a section where Linear would put it.
struct Rank(u8, i64, String);

fn assignee_name(user: &crate::api::types::User) -> String {
    user.display_name
        .clone()
        .unwrap_or_else(|| user.name.clone())
}

/// The key of the section `issue` falls into. Keys are what folding
/// remembers, so they name the group by id rather than by its label: two
/// people who share a name are two groups, and a rename keeps the fold.
fn section_key(issue: &Issue, by: GroupBy) -> String {
    match by {
        GroupBy::None => String::new(),
        GroupBy::Status => match &issue.state {
            Some(state) => format!("status:{}", state.id),
            None => "status:none".to_string(),
        },
        GroupBy::Priority => format!("priority:{}", issue.priority.as_u8()),
        GroupBy::Assignee => match &issue.assignee {
            Some(user) => format!("assignee:{}", user.id),
            None => "assignee:none".to_string(),
        },
        GroupBy::Project => match &issue.project {
            Some(project) => format!("project:{}", project.id),
            None => "project:none".to_string(),
        },
    }
}

/// How the section `issue` opens is drawn, and where it sorts. Only built
/// once per section, for its first issue.
fn section_head(issue: &Issue, by: GroupBy, theme: &Theme) -> (Rank, String, Color, &'static str) {
    match by {
        GroupBy::None => (Rank(0, 0, String::new()), String::new(), theme.muted, ""),
        GroupBy::Status => match &issue.state {
            Some(state) => {
                let category = state.state_type.unwrap_or(StateType::Unknown);
                (
                    // Within a category Linear lists the state furthest
                    // along the workflow first — In Review above In
                    // Progress — so position sorts descending. It is a
                    // float; scaling keeps the order in an integer key.
                    Rank(
                        category.rank(),
                        -(state.position.unwrap_or(0.0) * 1000.0) as i64,
                        state.name.clone(),
                    ),
                    state.name.clone(),
                    state
                        .color
                        .as_deref()
                        .and_then(hex_color)
                        .unwrap_or_else(|| category.color()),
                    category.glyph(),
                )
            }
            None => (
                Rank(u8::MAX, 0, String::new()),
                "No status".to_string(),
                theme.muted,
                StateType::Unknown.glyph(),
            ),
        },
        GroupBy::Priority => {
            let p = issue.priority;
            // Urgent first, None last — the priority enum's own order.
            let rank = match p {
                Priority::Urgent => 0,
                Priority::High => 1,
                Priority::Medium => 2,
                Priority::Low => 3,
                Priority::None => 4,
            };
            (
                Rank(rank, 0, String::new()),
                p.label().to_string(),
                p.color(theme),
                p.glyph(),
            )
        }
        GroupBy::Assignee => match &issue.assignee {
            Some(user) => {
                let name = assignee_name(user);
                (
                    Rank(0, 0, name.to_lowercase()),
                    name,
                    theme.accent,
                    "\u{25c6}",
                )
            }
            None => (
                Rank(1, 0, String::new()),
                "Unassigned".to_string(),
                theme.muted,
                "\u{25c7}",
            ),
        },
        GroupBy::Project => match &issue.project {
            Some(project) => (
                Rank(0, 0, project.name.to_lowercase()),
                project.name.clone(),
                project
                    .color
                    .as_deref()
                    .and_then(hex_color)
                    .unwrap_or(theme.secondary),
                "\u{25a3}",
            ),
            None => (
                Rank(1, 0, String::new()),
                "No project".to_string(),
                theme.muted,
                "\u{25a1}",
            ),
        },
    }
}

/// Split `issues` into sections along `by`, marking those in `collapsed` folded.
pub fn group<'a, I>(
    issues: I,
    by: GroupBy,
    collapsed: &HashSet<String>,
    theme: &Theme,
) -> Vec<Section<'a>>
where
    I: IntoIterator<Item = &'a Issue>,
{
    let mut sections: Vec<(Rank, Section<'a>)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();

    for issue in issues {
        let key = section_key(issue, by);
        if let Some(&i) = index.get(&key) {
            sections[i].1.issues.push((issue, 0));
            continue;
        }
        let (rank, label, color, glyph) = section_head(issue, by, theme);
        index.insert(key.clone(), sections.len());
        sections.push((
            rank,
            Section {
                collapsed: collapsed.contains(&key),
                key,
                label,
                color,
                glyph,
                issues: vec![(issue, 0)],
            },
        ));
    }

    sections.sort_by(|(a, _), (b, _)| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut sections: Vec<Section<'a>> = sections.into_iter().map(|(_, s)| s).collect();
    for section in &mut sections {
        nest_sub_issues(&mut section.issues);
    }
    sections
}

/// Pull each sub-issue up to sit directly under its parent, indented.
///
/// Linear shows a child beneath the issue it belongs to rather than wherever
/// its own update time happens to land it, which is the difference between a
/// list you can read as a tree and one you have to cross-reference.
fn nest_sub_issues(issues: &mut Vec<(&Issue, u8)>) {
    let ids: HashSet<&IssueId> = issues.iter().map(|(i, _)| &i.id).collect();
    // Children in their original order, by the parent they sit under.
    let mut children: HashMap<&IssueId, Vec<usize>> = HashMap::new();
    for (idx, (issue, _)) in issues.iter().enumerate() {
        if let Some(parent) = &issue.parent
            && ids.contains(&parent.id)
        {
            children.entry(&parent.id).or_default().push(idx);
        }
    }

    let mut ordered: Vec<(&Issue, u8)> = Vec::with_capacity(issues.len());
    let mut placed = vec![false; issues.len()];
    for idx in 0..issues.len() {
        let issue = issues[idx].0;
        // A child whose parent is also in this section waits for the parent.
        if placed[idx] || issue.parent.as_ref().is_some_and(|p| ids.contains(&p.id)) {
            continue;
        }
        placed[idx] = true;
        ordered.push((issue, 0));
        for &child in children.get(&issue.id).into_iter().flatten() {
            if !placed[child] {
                placed[child] = true;
                ordered.push((issues[child].0, 1));
            }
        }
    }

    // Anything left over is a child of a child, or of an issue that only got
    // filtered out; show it rather than drop it.
    for (idx, entry) in issues.iter().enumerate() {
        if !placed[idx] {
            ordered.push((entry.0, 1));
        }
    }

    *issues = ordered;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Theme, ThemeName};

    fn issue(json: &str) -> Issue {
        serde_json::from_str(json).expect("fixture issue")
    }

    fn base(id: &str, identifier: &str) -> String {
        format!(
            r#"{{"id":"{id}","identifier":"{identifier}","title":"t","priority":0,
                "state":null,"assignee":null,"description":null,"comments":null,
                "project":null,"cycle":null}}"#
        )
    }

    fn with_state(id: &str, state_id: &str, name: &str, kind: &str, position: f64) -> Issue {
        issue(&format!(
            r#"{{"id":"{id}","identifier":"X-{id}","title":"t","priority":0,
                "state":{{"id":"{state_id}","name":"{name}","type":"{kind}","position":{position}}},
                "assignee":null,"description":null,"comments":null,"project":null,"cycle":null}}"#
        ))
    }

    fn theme() -> Theme {
        Theme::from_name(ThemeName::Default)
    }

    #[test]
    fn status_groups_follow_the_workflow_order() {
        let issues = vec![
            with_state("1", "s-done", "Done", "completed", 3.0),
            with_state("2", "s-prog", "In Progress", "started", 1.0),
            with_state("3", "s-todo", "Todo", "unstarted", 2.0),
        ];
        let sections = group(&issues, GroupBy::Status, &HashSet::new(), &theme());
        let labels: Vec<_> = sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["In Progress", "Todo", "Done"]);
    }

    /// Two states in the same category order by workflow position — the one
    /// further along first, as Linear lists them — not by whichever issue the
    /// API happened to return first.
    #[test]
    fn states_in_one_category_list_the_furthest_along_first() {
        let issues = vec![
            with_state("1", "s-a", "In Progress", "started", 1.0),
            with_state("2", "s-b", "In Review", "started", 2.0),
        ];
        let sections = group(&issues, GroupBy::Status, &HashSet::new(), &theme());
        let labels: Vec<_> = sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["In Review", "In Progress"]);
    }

    #[test]
    fn an_issue_without_a_state_lands_in_its_own_last_group() {
        let issues = vec![
            issue(&base("1", "X-1")),
            with_state("2", "s", "Todo", "unstarted", 1.0),
        ];
        let sections = group(&issues, GroupBy::Status, &HashSet::new(), &theme());
        assert_eq!(sections.last().unwrap().label, "No status");
    }

    #[test]
    fn collapsing_marks_the_section_without_dropping_it() {
        let issues = vec![with_state("1", "s", "Todo", "unstarted", 1.0)];
        let collapsed = HashSet::from(["status:s".to_string()]);
        let sections = group(&issues, GroupBy::Status, &collapsed, &theme());
        assert_eq!(sections.len(), 1);
        assert!(sections[0].collapsed);
        assert_eq!(sections[0].issues.len(), 1, "the count still reads right");
    }

    #[test]
    fn grouping_by_none_yields_a_single_unlabelled_section() {
        let issues = vec![
            with_state("1", "a", "Todo", "unstarted", 1.0),
            with_state("2", "b", "Done", "completed", 2.0),
        ];
        let sections = group(&issues, GroupBy::None, &HashSet::new(), &theme());
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].issues.len(), 2);
    }

    #[test]
    fn priority_groups_run_urgent_to_none() {
        let mut low = issue(&base("1", "X-1"));
        low.priority = Priority::Low;
        let mut urgent = issue(&base("2", "X-2"));
        urgent.priority = Priority::Urgent;
        let none = issue(&base("3", "X-3"));
        let issues = vec![low, urgent, none];
        let sections = group(&issues, GroupBy::Priority, &HashSet::new(), &theme());
        let labels: Vec<_> = sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["Urgent", "Low", "None"]);
    }

    #[test]
    fn a_sub_issue_sits_under_its_parent() {
        let parent = with_state("p", "s", "Todo", "unstarted", 1.0);
        let other = with_state("o", "s", "Todo", "unstarted", 1.0);
        let child: Issue = issue(
            r#"{"id":"c","identifier":"X-c","title":"child","priority":0,
                "state":{"id":"s","name":"Todo","type":"unstarted","position":1.0},
                "assignee":null,"description":null,"comments":null,"project":null,
                "cycle":null,"parent":{"id":"p","identifier":"X-p","title":"t"}}"#,
        );
        // The child arrives last, as a newer update time would put it.
        let issues = vec![parent, other, child];
        let sections = group(&issues, GroupBy::Status, &HashSet::new(), &theme());
        let order: Vec<_> = sections[0]
            .issues
            .iter()
            .map(|(i, depth)| (i.id.as_str(), *depth))
            .collect();
        assert_eq!(order, [("p", 0), ("c", 1), ("o", 0)]);
    }

    // ------------------------------------------------------------- presets

    #[test]
    fn active_admits_started_and_unstarted_only() {
        assert!(Preset::Active.admits(&with_state("1", "s", "Doing", "started", 1.0)));
        assert!(Preset::Active.admits(&with_state("2", "s", "Todo", "unstarted", 1.0)));
        assert!(!Preset::Active.admits(&with_state("3", "s", "Done", "completed", 1.0)));
        assert!(!Preset::Active.admits(&with_state("4", "s", "Later", "backlog", 1.0)));
    }

    #[test]
    fn backlog_admits_backlog_and_triage() {
        assert!(Preset::Backlog.admits(&with_state("1", "s", "Backlog", "backlog", 1.0)));
        assert!(Preset::Backlog.admits(&with_state("2", "s", "Triage", "triage", 1.0)));
        assert!(!Preset::Backlog.admits(&with_state("3", "s", "Doing", "started", 1.0)));
    }

    /// A category added to Linear after this client shipped must not make
    /// issues disappear from the default view.
    #[test]
    fn an_unknown_category_stays_visible_under_active() {
        let odd = with_state("1", "s", "Something", "inventedIn2027", 1.0);
        assert!(Preset::Active.admits(&odd));
        assert!(Preset::All.admits(&odd));
    }

    #[test]
    fn only_all_is_unfiltered_on_the_server() {
        assert!(Preset::All.state_filter().is_none());
        assert!(Preset::Active.state_filter().is_some());
        assert!(Preset::Backlog.state_filter().is_some());
    }

    #[test]
    fn an_issue_without_a_state_stays_visible_under_active() {
        assert!(Preset::Active.admits(&issue(&base("1", "X-1"))));
    }

    fn assigned(id: &str, user_id: &str, name: &str) -> Issue {
        issue(&format!(
            r#"{{"id":"{id}","identifier":"X-{id}","title":"t","priority":0,"state":null,
                "assignee":{{"id":"{user_id}","name":"{name}"}},
                "description":null,"comments":null,"project":null,"cycle":null}}"#
        ))
    }

    fn child(id: &str, parent: &str) -> Issue {
        issue(&format!(
            r#"{{"id":"{id}","identifier":"X-{id}","title":"t","priority":0,"state":null,
                "assignee":null,"description":null,"comments":null,"project":null,"cycle":null,
                "parent":{{"id":"{parent}","identifier":"X-{parent}","title":"p"}}}}"#
        ))
    }

    /// Regression: assignee groups were keyed by display name, so two people
    /// who share one were merged into a single group.
    #[test]
    fn namesakes_get_a_group_each() {
        let issues = [
            assigned("1", "u1", "Alex"),
            assigned("2", "u2", "Alex"),
            assigned("3", "u1", "Alex"),
        ];
        let sections = group(issues.iter(), GroupBy::Assignee, &HashSet::new(), &theme());
        assert_eq!(sections.len(), 2);
        assert_eq!(sections.iter().map(|s| s.issues.len()).sum::<usize>(), 3);
        assert!(sections.iter().all(|s| s.label == "Alex"));
    }

    #[test]
    fn a_folded_assignee_group_is_remembered_by_user() {
        let issues = [assigned("1", "u1", "Alex")];
        let collapsed = HashSet::from(["assignee:u1".to_string()]);
        let sections = group(issues.iter(), GroupBy::Assignee, &collapsed, &theme());
        assert!(sections[0].collapsed);
    }

    #[test]
    fn a_grandchild_is_kept_and_indented() {
        let issues = [child("3", "2"), issue(&base("1", "X-1")), child("2", "1")];
        let sections = group(issues.iter(), GroupBy::None, &HashSet::new(), &theme());
        let rows: Vec<(&str, u8)> = sections[0]
            .issues
            .iter()
            .map(|(i, depth)| (i.id.as_str(), *depth))
            .collect();
        assert_eq!(rows, [("1", 0), ("2", 1), ("3", 1)]);
    }

    #[test]
    fn issues_group_by_project_with_the_projectless_last() {
        let in_project = issue(
            r#"{"id":"1","identifier":"X-1","title":"t","priority":0,"state":null,"assignee":null,
                "description":null,"comments":null,"cycle":null,
                "project":{"id":"p1","name":"Launch","lead":null}}"#,
        );
        let issues = [issue(&base("2", "X-2")), in_project];
        let sections = group(issues.iter(), GroupBy::Project, &HashSet::new(), &theme());
        let labels: Vec<&str> = sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["Launch", "No project"]);
    }
}
