//! Issues: finding and reading them, and changing them.
//!
//! **Lists.** A team's issues in one of Linear's slices, my issues, a saved
//! view's, a project's, a cycle's: [`open_team_issues`], [`open_my_issues`],
//! [`open_view_issues`], [`open_project_issues`], [`open_cycle_issues`];
//! then [`next_page`], [`reload`], and [`take_page`] as pages land. A list
//! is narrowed on screen by [`matches`], and searched across the workspace
//! by [`search`] (results land by [`take_search_results`]) or from the
//! command palette by [`quick_search`].
//!
//! **One issue.** [`open`] reads it with its thread, and [`take_detail`]
//! takes what Linear sends; [`ensure_thread`] keeps the thread in view and
//! [`refresh`] reads it again. [`copy`] and [`open_url`] take its
//! identifier, URL, or branch name elsewhere.
//!
//! **Changes.** [`set_status`], [`set_priority`], [`set_assignee`],
//! [`assign_to_me`], [`comment`], [`create`] (and [`created`] once Linear
//! has it). A change is optimistic: every copy of the issue on screen shows
//! it at once, and the returned request asks Linear to make it real. Should
//! Linear refuse, [`change_refused`] reads the issue back.

use super::{Open, Refusal};
use crate::entity::{CustomViewId, CycleId, IssueId, ProjectId, TeamId, UserId, WorkflowStateId};
use crate::entity::{Issue, IssueFilter, Page, Preset, Priority, User, WorkflowState};
use crate::store::{IssueSource, List, ListOf, Store};

/// What the issue use cases ask of Linear, and of the desktop.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// A page of a team's issues in one slice. Linear filters by workflow
    /// category, so Active means every active issue, not the active ones
    /// among the latest page.
    TeamIssues {
        team_id: TeamId,
        preset: Preset,
        after: Option<String>,
    },
    MyIssues {
        user_id: UserId,
        after: Option<String>,
    },
    /// A page of a saved view's issues. Linear evaluates the view's filter.
    ViewIssues {
        view_id: CustomViewId,
        after: Option<String>,
    },
    ProjectIssues {
        project_id: ProjectId,
        after: Option<String>,
    },
    CycleIssues {
        cycle_id: CycleId,
        after: Option<String>,
    },
    /// One issue with its description, sub-issues, and thread.
    Detail {
        issue_id: IssueId,
    },
    /// Full-text search, in one team or the whole workspace.
    Search {
        term: String,
        team_id: Option<TeamId>,
    },
    /// Search as the user types in the command palette. `seq` tells a stale
    /// answer from the one for the latest query.
    QuickSearch {
        term: String,
        seq: u64,
    },
    SetStatus {
        issue_id: IssueId,
        state_id: WorkflowStateId,
    },
    SetPriority {
        issue_id: IssueId,
        priority: Priority,
    },
    SetAssignee {
        issue_id: IssueId,
        assignee_id: Option<UserId>,
    },
    Comment {
        issue_id: IssueId,
        body: String,
    },
    Create {
        team_id: TeamId,
        title: String,
        description: Option<String>,
        priority: Priority,
    },
    /// Show the issue's page on linear.app in the desktop's browser.
    OpenInBrowser(String),
}

impl Request {
    /// The page cursor this request continues from, if it asks for a next
    /// page.
    pub fn cursor(&self) -> Option<&str> {
        match self {
            Self::TeamIssues { after, .. }
            | Self::MyIssues { after, .. }
            | Self::ViewIssues { after, .. }
            | Self::ProjectIssues { after, .. }
            | Self::CycleIssues { after, .. } => after.as_deref(),
            _ => None,
        }
    }

    /// The issue a change already shows, which must be read back from Linear
    /// if the change is refused.
    pub fn changed_issue(&self) -> Option<&IssueId> {
        match self {
            Self::SetStatus { issue_id, .. }
            | Self::SetPriority { issue_id, .. }
            | Self::SetAssignee { issue_id, .. } => Some(issue_id),
            _ => None,
        }
    }
}

// ------------------------------------------------------------------ lists

/// **Browse a team's issues** in one of Linear's slices: Active, Backlog,
/// or All.
///
/// Linear slices the list, so a team's Active issues are all of them, not
/// the active ones among the latest page. A list already holding that slice
/// is not fetched again. Moving to another slice of the same team keeps the
/// rows on screen until the new slice lands; moving to another team drops
/// them.
pub fn open_team_issues(store: &mut Store, team_id: TeamId, preset: Preset) -> Open {
    let of = ListOf::TeamIssues { team_id, preset };
    store.open_list(List::Issues(IssueSource::Team), of).into()
}

/// **See the issues assigned to me**, across every team.
///
/// Needs to know who "me" is, which Linear answers at startup; until then
/// there is nothing to list.
pub fn open_my_issues(store: &mut Store) -> Result<Open, Refusal> {
    let me = store.viewer_id.clone().ok_or(Refusal::ViewerNotLoaded)?;
    Ok(store
        .open_list(List::Issues(IssueSource::My), ListOf::MyIssues(me))
        .into())
}

/// **Open a saved issue view.** Linear evaluates the view's filter; the
/// list is what it answers. Another view's rows are dropped first, so they
/// are never shown under this view's name.
pub fn open_view_issues(store: &mut Store, view_id: CustomViewId) -> Open {
    store
        .open_list(List::Issues(IssueSource::View), ListOf::ViewIssues(view_id))
        .into()
}

/// **See a project's issues.**
pub fn open_project_issues(store: &mut Store, project_id: ProjectId) -> Open {
    let of = ListOf::ProjectIssues(project_id);
    store
        .open_list(List::Issues(IssueSource::Project), of)
        .into()
}

/// **See a cycle's issues.**
pub fn open_cycle_issues(store: &mut Store, cycle_id: CycleId) -> Open {
    let of = ListOf::CycleIssues(cycle_id);
    store.open_list(List::Issues(IssueSource::Cycle), of).into()
}

/// **Scroll on to the next page** of an issue list.
///
/// Asked for once: a page on its way is not asked for again while the user
/// keeps scrolling, and after the last page there is nothing to ask for.
pub fn next_page(store: &mut Store, source: IssueSource) -> Option<super::Request> {
    store
        .next_page(List::Issues(source))
        .map(super::Request::page)
}

/// **Refresh an issue list**: its first page again, the rows kept until it
/// lands.
pub fn reload(store: &mut Store, source: IssueSource) -> Option<super::Request> {
    store
        .reload_list(List::Issues(source))
        .map(super::Request::page)
}

/// **A page of issues lands.** It is taken only while the list still
/// belongs to what it was fetched for; a page for a team, view, project,
/// cycle, or slice the user has since left is dropped. An issue already
/// listed is not listed twice. Returns whether the page was taken.
pub fn take_page(store: &mut Store, source: IssueSource, of: &ListOf, page: Page<Issue>) -> bool {
    store.issues[source].accept_issues_for(of, page)
}

/// **Narrow a list as I type**, and by status and priority.
///
/// An issue stays when its preset admits it, its title or identifier
/// contains `query` (already lowercased; any case matches), and it is in the
/// filter's status and priority when those are set. The status is compared
/// by name, since a list can hold issues of several teams.
pub fn matches(issue: &Issue, preset: Preset, filter: &IssueFilter, query: &str) -> bool {
    preset.admits(issue)
        && (query.is_empty()
            || issue.title.to_lowercase().contains(query)
            || issue.identifier.to_lowercase().contains(query))
        && filter.status.as_ref().is_none_or(|status| {
            issue
                .state
                .as_ref()
                .is_some_and(|state| &state.name == status)
        })
        && filter.priority.is_none_or(|p| issue.priority == p)
}

/// **Search all of Linear** for `term`, in one team or everywhere.
///
/// Nothing is searched for an empty term.
pub fn search(term: &str, team_id: Option<TeamId>) -> Option<Request> {
    (!term.is_empty()).then(|| Request::Search {
        term: term.to_string(),
        team_id,
    })
}

/// **Find an issue from the command palette**, searching as I type.
///
/// Each query is numbered, so an answer to an older query that lands late
/// can be told apart from the answer to the latest.
pub fn quick_search(term: &str, seq: u64) -> Request {
    Request::QuickSearch {
        term: term.trim().to_string(),
        seq,
    }
}

/// **Search results land** in a team's list, in place of its issues.
///
/// Search answers "where is it", done or not, so the results are the team's
/// All slice: Active would quietly hide half of them. They have no next
/// page.
pub fn take_search_results(store: &mut Store, team_id: TeamId, issues: Vec<Issue>) {
    let rows = &mut store.issues[IssueSource::Team];
    rows.reset();
    rows.of = Some(ListOf::TeamIssues {
        team_id,
        preset: Preset::All,
    });
    rows.items = issues;
    rows.loaded = true;
}

// -------------------------------------------------------------- one issue

/// **Read an issue**: its fields at once, from the copy at hand, and its
/// description, sub-issues, and comment thread once Linear sends them. A
/// copy that already has its thread is not read again.
pub fn open(store: &mut Store, issue: Issue) -> Option<Request> {
    let request = issue.comments.is_none().then(|| Request::Detail {
        issue_id: issue.id.clone(),
    });
    store.current_issue = Some(issue);
    request
}

/// **An issue's details arrive.** Every copy on screen takes them, each list
/// copy keeping its own thread state. An issue opened by its identifier —
/// `linear-tui open ENG-42` — learns its real id now. Returns whether it did.
pub fn take_detail(store: &mut Store, issue: Issue) -> bool {
    let mut adopted = false;
    if let Some(current) = &mut store.current_issue
        && current.id != issue.id
        && current.identifier == issue.identifier
    {
        *current = issue.clone();
        adopted = true;
    }
    store.refresh_issue(issue);
    adopted
}

/// **Keep the open issue's thread in view.** When the open issue has no
/// thread — it was just opened, or a comment was just posted — it is read.
pub fn ensure_thread(store: &Store) -> Option<Request> {
    let issue = store.current_issue.as_ref()?;
    issue.comments.is_none().then(|| Request::Detail {
        issue_id: issue.id.clone(),
    })
}

/// **Read the open issue again**, thread and all.
pub fn refresh(store: &mut Store) -> Option<Request> {
    let issue = store.current_issue.as_mut()?;
    issue.comments = None;
    Some(Request::Detail {
        issue_id: issue.id.clone(),
    })
}

/// **Change an issue's status** (`s`).
///
/// The issue moves to `state` in every list and in the open detail at once;
/// Linear is asked to move it too.
pub fn set_status(store: &mut Store, issue_id: &IssueId, state: WorkflowState) -> Request {
    let state_id = state.id.clone();
    store.patch_issue(issue_id, |i| i.state = Some(state.clone()));
    Request::SetStatus {
        issue_id: issue_id.clone(),
        state_id,
    }
}

/// **Change an issue's priority** (`p`, or directly with `!` `@` `#` `$` `)`).
///
/// The priority and its label change everywhere at once; Linear is asked to
/// change it too.
pub fn set_priority(store: &mut Store, issue_id: &IssueId, priority: Priority) -> Request {
    store.patch_issue(issue_id, |i| {
        i.priority = priority;
        i.priority_label = Some(priority.label().to_string());
    });
    Request::SetPriority {
        issue_id: issue_id.clone(),
        priority,
    }
}

/// **Assign an issue to someone, or unassign it** (`a`).
///
/// `None` unassigns. The assignee changes everywhere at once; Linear is asked
/// to change it too.
pub fn set_assignee(store: &mut Store, issue_id: &IssueId, assignee: Option<User>) -> Request {
    let assignee_id = assignee.as_ref().map(|u| u.id.clone());
    store.patch_issue(issue_id, |i| i.assignee = assignee.clone());
    Request::SetAssignee {
        issue_id: issue_id.clone(),
        assignee_id,
    }
}

/// **Assign an issue to myself** (`i`, Linear's `I`).
///
/// Needs to know who "myself" is, which Linear answers at startup; until it
/// has, the user is told so and nothing changes. Before any of the user's
/// teams has loaded only their id is known, so the issue is assigned but no
/// name can be shown yet.
pub fn assign_to_me(store: &mut Store, issue_id: &IssueId) -> Result<Request, Refusal> {
    let viewer_id = store.viewer_id.clone().ok_or(Refusal::ViewerNotLoaded)?;
    let me = store.viewer().cloned();
    store.patch_issue(issue_id, |i| i.assignee = me.clone());
    Ok(Request::SetAssignee {
        issue_id: issue_id.clone(),
        assignee_id: Some(viewer_id),
    })
}

/// **Comment on an issue** (`m`).
///
/// An empty comment is not sent. The open issue's thread is dropped, so it
/// is read again once Linear has the comment and the new comment shows in
/// place.
pub fn comment(store: &mut Store, issue_id: &IssueId, body: String) -> Option<Request> {
    if body.is_empty() {
        return None;
    }
    if let Some(current) = &mut store.current_issue
        && &current.id == issue_id
    {
        current.comments = None;
    }
    Some(Request::Comment {
        issue_id: issue_id.clone(),
        body,
    })
}

/// What a new issue is filed with.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    /// Required.
    pub title: String,
    /// Markdown; may be empty.
    pub description: String,
    pub priority: Priority,
}

/// **Create an issue** in a team (`c`).
///
/// A title is required. An empty description is sent as none rather than
/// as an empty text.
pub fn create(team_id: TeamId, draft: Draft) -> Result<Request, Refusal> {
    if draft.title.is_empty() {
        return Err(Refusal::TitleRequired);
    }
    Ok(Request::Create {
        team_id,
        title: draft.title,
        description: Some(draft.description).filter(|d| !d.is_empty()),
        priority: draft.priority,
    })
}

/// **A new issue is filed.** It appears at the top of its team's list at
/// once, without waiting for the list to be fetched again, when that list
/// is the team's; another team's list picks it up on its next fetch.
/// Returns whether it was listed.
pub fn created(store: &mut Store, team_id: &TeamId, issue: Issue) -> bool {
    let rows = &mut store.issues[IssueSource::Team];
    let ours = matches!(&rows.of, Some(ListOf::TeamIssues { team_id: t, .. }) if t == team_id);
    if ours {
        rows.items.insert(0, issue);
    }
    ours
}

/// **Linear refuses a change.** The change is already on screen, so rather
/// than guess what to undo, the issue is read back from Linear and every
/// copy takes what Linear holds.
pub fn change_refused(issue_id: &IssueId) -> Request {
    Request::Detail {
        issue_id: issue_id.clone(),
    }
}

/// What of an issue can be copied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyField {
    Identifier,
    Url,
    BranchName,
}

impl CopyField {
    /// How the status line names it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Identifier => "identifier",
            Self::Url => "URL",
            Self::BranchName => "branch name",
        }
    }
}

/// **Copy an issue's identifier, URL, or git branch name** (`y`, `Y`, `b`).
///
/// Returns the text for the clipboard. The URL and branch name are the ones
/// Linear gives the issue. When Linear gave none, or no issue is at hand,
/// there is nothing to copy, and the user is told what was missing.
pub fn copy(issue: Option<&Issue>, field: CopyField) -> Result<String, Refusal> {
    let text = issue.and_then(|issue| match field {
        CopyField::Identifier => Some(issue.identifier.clone()),
        CopyField::Url => issue.url.clone(),
        CopyField::BranchName => issue.branch_name.clone(),
    });
    text.ok_or(Refusal::NothingToCopy(field.label()))
}

/// **Open a page on linear.app** (`o`): an issue, or a project.
///
/// The page opens in the desktop's browser. Without a URL there is nothing
/// to open.
pub fn open_url(url: Option<String>) -> Result<Request, Refusal> {
    url.map(Request::OpenInBrowser)
        .ok_or(Refusal::NothingToOpen)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::UserId;
    use crate::store::{IssueSource, TeamContext};

    /// A store holding issue `i1` (ENG-1) in the team list, in My Issues,
    /// and open in the detail view.
    fn store_with_issue() -> Store {
        let issue: Issue = serde_json::from_str(
            r#"{"id":"i1","identifier":"ENG-1","title":"t","priority":0,
                "url":"https://linear.app/acme/issue/ENG-1/t","branchName":"me/eng-1-t"}"#,
        )
        .unwrap();
        let mut store = Store::default();
        store.issues[IssueSource::Team].items = vec![issue.clone()];
        store.issues[IssueSource::My].items = vec![issue.clone()];
        store.current_issue = Some(issue);
        store
    }

    /// Every copy of `i1`, so a test can check they agree.
    fn copies(store: &Store) -> [&Issue; 3] {
        [
            &store.issues[IssueSource::Team].items[0],
            &store.issues[IssueSource::My].items[0],
            store.current_issue.as_ref().unwrap(),
        ]
    }

    fn user(id: &str) -> User {
        serde_json::from_str(&format!(r#"{{"id":"{id}","name":"{id}"}}"#)).unwrap()
    }

    fn done() -> WorkflowState {
        serde_json::from_str(r#"{"id":"s","name":"Done","type":"completed","position":1}"#).unwrap()
    }

    fn id() -> IssueId {
        IssueId::from("i1")
    }

    // ---------------------------------------------------------- status

    /// Linear is asked to move the issue to the chosen state.
    #[test]
    fn a_status_change_asks_linear_for_the_new_state() {
        let request = set_status(&mut store_with_issue(), &id(), done());
        assert_eq!(
            request,
            Request::SetStatus {
                issue_id: id(),
                state_id: "s".into()
            }
        );
    }

    /// Every list and the open detail show the new state before Linear
    /// answers.
    #[test]
    fn a_status_change_shows_everywhere_at_once() {
        let mut store = store_with_issue();
        set_status(&mut store, &id(), done());
        for issue in copies(&store) {
            assert_eq!(issue.state.as_ref().unwrap().name, "Done");
        }
    }

    // -------------------------------------------------------- priority

    /// The priority and the label shown for it change together, everywhere.
    #[test]
    fn a_priority_change_updates_the_label_too() {
        let mut store = store_with_issue();
        let request = set_priority(&mut store, &id(), Priority::High);
        assert_eq!(
            request,
            Request::SetPriority {
                issue_id: id(),
                priority: Priority::High
            }
        );
        for issue in copies(&store) {
            assert_eq!(issue.priority, Priority::High);
            assert_eq!(issue.priority_label.as_deref(), Some("High"));
        }
    }

    // -------------------------------------------------------- assignee

    /// Assigning shows the assignee everywhere and asks Linear for them.
    #[test]
    fn assigning_shows_the_assignee_and_asks_linear() {
        let mut store = store_with_issue();
        let request = set_assignee(&mut store, &id(), Some(user("u")));
        assert_eq!(
            request,
            Request::SetAssignee {
                issue_id: id(),
                assignee_id: Some("u".into())
            }
        );
        for issue in copies(&store) {
            assert_eq!(issue.assignee.as_ref().unwrap().name, "u");
        }
    }

    /// Unassigning clears the assignee and asks Linear for nobody.
    #[test]
    fn unassigning_clears_the_assignee() {
        let mut store = store_with_issue();
        set_assignee(&mut store, &id(), Some(user("u")));
        let request = set_assignee(&mut store, &id(), None);
        assert_eq!(
            request,
            Request::SetAssignee {
                issue_id: id(),
                assignee_id: None
            }
        );
        assert!(store.issue(&id()).unwrap().assignee.is_none());
    }

    /// Until Linear has said who the user is, "assign to me" is refused and
    /// nothing changes.
    #[test]
    fn assign_to_me_waits_until_the_user_is_known() {
        let mut store = store_with_issue();
        assert_eq!(
            assign_to_me(&mut store, &id()),
            Err(Refusal::ViewerNotLoaded)
        );
        assert!(store.issue(&id()).unwrap().assignee.is_none());
    }

    /// Once the user and one of their teams are known, the issue shows them
    /// as its assignee.
    #[test]
    fn assign_to_me_shows_me_as_the_assignee() {
        let mut store = store_with_issue();
        store.viewer_id = Some(UserId::from("me"));
        store.team_contexts.insert(
            TeamId::from("t"),
            TeamContext {
                states: Vec::new(),
                members: vec![user("me")],
            },
        );
        assert_eq!(
            assign_to_me(&mut store, &id()),
            Ok(Request::SetAssignee {
                issue_id: id(),
                assignee_id: Some("me".into())
            })
        );
        assert_eq!(
            store.issue(&id()).unwrap().assignee.as_ref().unwrap().name,
            "me"
        );
    }

    /// Known only by id, the user is still assigned in Linear; the issue
    /// shows no name until a team tells it.
    #[test]
    fn assign_to_me_before_my_teams_load_still_asks_linear() {
        let mut store = store_with_issue();
        store.viewer_id = Some(UserId::from("me"));
        assert!(assign_to_me(&mut store, &id()).is_ok());
        assert!(store.issue(&id()).unwrap().assignee.is_none());
    }

    // --------------------------------------------------------- comment

    /// An empty comment is not sent.
    #[test]
    fn an_empty_comment_is_not_sent() {
        assert_eq!(comment(&mut store_with_issue(), &id(), String::new()), None);
    }

    /// A comment is sent, and the open thread is dropped so it is read again
    /// with the new comment in it.
    #[test]
    fn a_comment_is_sent_and_the_thread_read_again() {
        let mut store = store_with_issue();
        store.current_issue.as_mut().unwrap().comments =
            Some(serde_json::from_str(r#"{"nodes":[]}"#).unwrap());
        let request = comment(&mut store, &id(), "hi".into());
        assert_eq!(
            request,
            Some(Request::Comment {
                issue_id: id(),
                body: "hi".into()
            })
        );
        assert!(store.current_issue.as_ref().unwrap().comments.is_none());
    }

    // ---------------------------------------------------------- create

    fn draft(title: &str, description: &str) -> Draft {
        Draft {
            title: title.into(),
            description: description.into(),
            priority: Priority::Medium,
        }
    }

    /// An issue without a title is refused.
    #[test]
    fn a_new_issue_needs_a_title() {
        assert_eq!(
            create(TeamId::from("t"), draft("", "")),
            Err(Refusal::TitleRequired)
        );
    }

    /// A new issue is filed in the team with its title, description, and
    /// priority.
    #[test]
    fn a_new_issue_is_filed_as_drafted() {
        assert_eq!(
            create(TeamId::from("t"), draft("Crash", "On start")),
            Ok(Request::Create {
                team_id: TeamId::from("t"),
                title: "Crash".into(),
                description: Some("On start".into()),
                priority: Priority::Medium,
            })
        );
    }

    /// An empty description is sent as none.
    #[test]
    fn an_empty_description_is_sent_as_none() {
        let Ok(Request::Create { description, .. }) = create(TeamId::from("t"), draft("x", ""))
        else {
            panic!("expected a create request");
        };
        assert_eq!(description, None);
    }

    // ------------------------------------------------------ copy, open

    /// The identifier, URL, and branch name copied are the issue's own.
    #[test]
    fn the_identifier_url_and_branch_name_are_copied() {
        let store = store_with_issue();
        let issue = store.issue(&id());
        assert_eq!(copy(issue, CopyField::Identifier).unwrap(), "ENG-1");
        assert_eq!(
            copy(issue, CopyField::Url).unwrap(),
            "https://linear.app/acme/issue/ENG-1/t"
        );
        assert_eq!(copy(issue, CopyField::BranchName).unwrap(), "me/eng-1-t");
    }

    /// With no issue at hand, or nothing Linear gave for the field, the user
    /// is told what there was none of.
    #[test]
    fn copying_what_is_not_there_says_what_was_missing() {
        assert_eq!(
            copy(None, CopyField::Identifier),
            Err(Refusal::NothingToCopy("identifier"))
        );
        let bare: Issue =
            serde_json::from_str(r#"{"id":"i","identifier":"ENG-2","title":"t","priority":0}"#)
                .unwrap();
        assert_eq!(
            copy(Some(&bare), CopyField::BranchName),
            Err(Refusal::NothingToCopy("branch name"))
        );
    }

    /// A URL opens in the browser; without one there is nothing to open.
    #[test]
    fn a_page_opens_by_its_url() {
        assert_eq!(
            open_url(Some("https://linear.app/x".into())),
            Ok(Request::OpenInBrowser("https://linear.app/x".into()))
        );
        assert_eq!(open_url(None), Err(Refusal::NothingToOpen));
    }

    // ----------------------------------------------------------- lists

    fn team(id: &str) -> TeamId {
        TeamId::from(id)
    }

    fn row(id: &str, title: &str, state: (&str, &str), priority: u8) -> Issue {
        serde_json::from_str(&format!(
            r#"{{"id":"{id}","identifier":"ENG-{id}","title":"{title}","priority":{priority},
                "state":{{"id":"s-{0}","name":"{0}","type":"{1}","position":1}}}}"#,
            state.0, state.1
        ))
        .unwrap()
    }

    fn page_of(issues: Vec<Issue>, append: bool) -> Page<Issue> {
        Page::new(issues, Default::default(), append)
    }

    /// A team's Active issues are asked of Linear, which does the slicing.
    #[test]
    fn a_teams_slice_is_asked_of_linear() {
        let mut store = Store::default();
        assert_eq!(
            open_team_issues(&mut store, team("t"), Preset::Active),
            Open::Replace(crate::usecase::Request::Issue(Request::TeamIssues {
                team_id: team("t"),
                after: None,
                preset: Preset::Active
            }))
        );
    }

    /// A slice already listed is not fetched again.
    #[test]
    fn a_slice_already_listed_is_not_fetched_again() {
        let mut store = Store::default();
        open_team_issues(&mut store, team("t"), Preset::Active);
        let of = ListOf::TeamIssues {
            team_id: team("t"),
            preset: Preset::Active,
        };
        take_page(&mut store, IssueSource::Team, &of, page_of(vec![], false));
        assert_eq!(
            open_team_issues(&mut store, team("t"), Preset::Active),
            Open::Cached
        );
    }

    /// Another slice of the team is fetched, and the rows on screen stay
    /// until it lands.
    #[test]
    fn another_slice_is_fetched_over_the_rows_on_screen() {
        let mut store = Store::default();
        open_team_issues(&mut store, team("t"), Preset::Active);
        store.issues[IssueSource::Team].items = vec![row("1", "a", ("Todo", "unstarted"), 0)];
        assert!(matches!(
            open_team_issues(&mut store, team("t"), Preset::Backlog),
            Open::Fetch(crate::usecase::Request::Issue(Request::TeamIssues {
                preset: Preset::Backlog,
                ..
            }))
        ));
        assert_eq!(store.issues[IssueSource::Team].items.len(), 1);
    }

    /// Another team's issues replace the list; none of the old team's is
    /// shown under the new name.
    #[test]
    fn another_teams_issues_replace_the_list() {
        let mut store = Store::default();
        open_team_issues(&mut store, team("t"), Preset::Active);
        store.issues[IssueSource::Team].items = vec![row("1", "a", ("Todo", "unstarted"), 0)];
        assert!(matches!(
            open_team_issues(&mut store, team("u"), Preset::Active),
            Open::Replace(_)
        ));
        assert!(store.issues[IssueSource::Team].items.is_empty());
    }

    /// A page for a slice the user has since left is dropped.
    #[test]
    fn a_page_for_a_slice_left_behind_is_dropped() {
        let mut store = Store::default();
        open_team_issues(&mut store, team("t"), Preset::Active);
        open_team_issues(&mut store, team("t"), Preset::Backlog);
        let active = ListOf::TeamIssues {
            team_id: team("t"),
            preset: Preset::Active,
        };
        let page = page_of(vec![row("1", "a", ("Todo", "unstarted"), 0)], false);
        assert!(!take_page(&mut store, IssueSource::Team, &active, page));
        assert!(store.issues[IssueSource::Team].items.is_empty());
    }

    /// An issue that moved between two pages is listed once.
    #[test]
    fn an_issue_on_two_pages_is_listed_once() {
        let mut store = Store::default();
        open_project_issues(&mut store, ProjectId::from("p"));
        let of = ListOf::ProjectIssues(ProjectId::from("p"));
        let issue = || row("1", "a", ("Todo", "unstarted"), 0);
        take_page(
            &mut store,
            IssueSource::Project,
            &of,
            page_of(vec![issue()], false),
        );
        take_page(
            &mut store,
            IssueSource::Project,
            &of,
            page_of(vec![issue()], true),
        );
        assert_eq!(store.issues[IssueSource::Project].items.len(), 1);
    }

    /// My issues wait until Linear has said who I am, then are asked for.
    #[test]
    fn my_issues_need_to_know_who_i_am() {
        let mut store = Store::default();
        assert_eq!(open_my_issues(&mut store), Err(Refusal::ViewerNotLoaded));
        store.viewer_id = Some(crate::entity::UserId::from("me"));
        assert_eq!(
            open_my_issues(&mut store),
            Ok(Open::Replace(crate::usecase::Request::Issue(
                Request::MyIssues {
                    user_id: "me".into(),
                    after: None
                }
            )))
        );
    }

    /// A view, a project, and a cycle each ask for their own issues.
    #[test]
    fn a_view_project_and_cycle_each_ask_for_their_issues() {
        let mut store = Store::default();
        assert_eq!(
            open_view_issues(&mut store, CustomViewId::from("v")).request(),
            Some(crate::usecase::Request::Issue(Request::ViewIssues {
                view_id: "v".into(),
                after: None
            }))
        );
        assert_eq!(
            open_project_issues(&mut store, ProjectId::from("p")).request(),
            Some(crate::usecase::Request::Issue(Request::ProjectIssues {
                project_id: "p".into(),
                after: None
            }))
        );
        assert_eq!(
            open_cycle_issues(&mut store, CycleId::from("c")).request(),
            Some(crate::usecase::Request::Issue(Request::CycleIssues {
                cycle_id: "c".into(),
                after: None
            }))
        );
    }

    /// The next page is asked for once, with the cursor Linear gave.
    #[test]
    fn the_next_page_is_asked_for_once() {
        let mut store = Store::default();
        open_team_issues(&mut store, team("t"), Preset::All);
        let of = ListOf::TeamIssues {
            team_id: team("t"),
            preset: Preset::All,
        };
        let info = crate::entity::PageInfo {
            has_next_page: true,
            end_cursor: Some("c1".into()),
        };
        take_page(
            &mut store,
            IssueSource::Team,
            &of,
            Page::new(vec![], info, false),
        );
        assert_eq!(
            next_page(&mut store, IssueSource::Team),
            Some(crate::usecase::Request::Issue(Request::TeamIssues {
                team_id: team("t"),
                after: Some("c1".into()),
                preset: Preset::All
            }))
        );
        assert_eq!(next_page(&mut store, IssueSource::Team), None);
    }

    /// Refreshing asks for the first page again.
    #[test]
    fn refreshing_a_list_asks_for_its_first_page() {
        let mut store = Store::default();
        open_cycle_issues(&mut store, CycleId::from("c"));
        assert_eq!(
            reload(&mut store, IssueSource::Cycle),
            Some(crate::usecase::Request::Issue(Request::CycleIssues {
                cycle_id: "c".into(),
                after: None
            }))
        );
        assert_eq!(reload(&mut store, IssueSource::My), None);
    }

    // --------------------------------------------------------- narrowing

    /// The search text matches the title or the identifier, in any case.
    #[test]
    fn typing_matches_the_title_or_identifier_in_any_case() {
        let issue = row("7", "Fix Login", ("Todo", "unstarted"), 0);
        let none = IssueFilter::default();
        assert!(matches(&issue, Preset::All, &none, "login"));
        assert!(matches(&issue, Preset::All, &none, "eng-7"));
        assert!(!matches(&issue, Preset::All, &none, "search"));
    }

    /// A status filter matches the state by name; a priority filter, the
    /// priority.
    #[test]
    fn filters_match_status_by_name_and_priority() {
        let issue = row("1", "a", ("Todo", "unstarted"), 2);
        let status = |name: &str| IssueFilter {
            status: Some(name.into()),
            priority: None,
        };
        assert!(matches(&issue, Preset::All, &status("Todo"), ""));
        assert!(!matches(&issue, Preset::All, &status("Done"), ""));
        let high = IssueFilter {
            status: None,
            priority: Some(Priority::High),
        };
        assert!(matches(&issue, Preset::All, &high, ""));
        let low = IssueFilter {
            status: None,
            priority: Some(Priority::Low),
        };
        assert!(!matches(&issue, Preset::All, &low, ""));
    }

    /// The list's preset narrows it too: Active leaves done work out.
    #[test]
    fn the_preset_narrows_the_list() {
        let done = row("1", "a", ("Done", "completed"), 0);
        assert!(!matches(&done, Preset::Active, &IssueFilter::default(), ""));
        assert!(matches(&done, Preset::All, &IssueFilter::default(), ""));
    }

    // ------------------------------------------------------------ search

    /// An empty term searches for nothing.
    #[test]
    fn an_empty_search_is_not_sent() {
        assert_eq!(search("", Some(team("t"))), None);
        assert_eq!(
            search("login", Some(team("t"))),
            Some(Request::Search {
                term: "login".into(),
                team_id: Some(team("t"))
            })
        );
    }

    /// Results fill the team's list as its All slice, with no next page.
    #[test]
    fn search_results_are_the_teams_whole_list() {
        let mut store = Store::default();
        take_search_results(
            &mut store,
            team("t"),
            vec![row("1", "a", ("Done", "completed"), 0)],
        );
        let rows = &store.issues[IssueSource::Team];
        assert_eq!(rows.items.len(), 1);
        assert_eq!(
            rows.of,
            Some(ListOf::TeamIssues {
                team_id: team("t"),
                preset: Preset::All
            })
        );
        assert_eq!(next_page(&mut store, IssueSource::Team), None);
    }

    // --------------------------------------------------------- one issue

    /// Opening an issue shows the copy at hand and asks for its thread.
    #[test]
    fn opening_an_issue_asks_for_its_thread() {
        let mut store = Store::default();
        let issue = row("1", "a", ("Todo", "unstarted"), 0);
        assert_eq!(
            open(&mut store, issue),
            Some(Request::Detail {
                issue_id: IssueId::from("1")
            })
        );
        assert_eq!(store.current_issue.as_ref().unwrap().title, "a");
    }

    /// A copy that has its thread is shown without asking again.
    #[test]
    fn an_issue_with_its_thread_is_not_read_again() {
        let mut store = Store::default();
        let mut issue = row("1", "a", ("Todo", "unstarted"), 0);
        issue.comments = Some(serde_json::from_str(r#"{"nodes":[]}"#).unwrap());
        assert_eq!(open(&mut store, issue), None);
    }

    /// Refreshing the open issue drops its thread and reads it again.
    #[test]
    fn refreshing_an_issue_reads_it_again() {
        let mut store = store_with_issue();
        store.current_issue.as_mut().unwrap().comments =
            Some(serde_json::from_str(r#"{"nodes":[]}"#).unwrap());
        assert_eq!(
            refresh(&mut store),
            Some(Request::Detail { issue_id: id() })
        );
        assert!(store.current_issue.as_ref().unwrap().comments.is_none());
        assert_eq!(refresh(&mut Store::default()), None);
    }

    // ------------------------------------------------- created, refused

    /// A new issue goes to the top of its team's list at once.
    #[test]
    fn a_new_issue_goes_to_the_top_of_its_teams_list() {
        let mut store = Store::default();
        open_team_issues(&mut store, team("t"), Preset::Active);
        store.issues[IssueSource::Team].items = vec![row("1", "old", ("Todo", "unstarted"), 0)];
        assert!(created(
            &mut store,
            &team("t"),
            row("2", "new", ("Todo", "unstarted"), 0)
        ));
        assert_eq!(store.issues[IssueSource::Team].items[0].title, "new");
    }

    /// Another team's new issue waits for that team's next fetch.
    #[test]
    fn another_teams_new_issue_is_not_listed_here() {
        let mut store = Store::default();
        open_team_issues(&mut store, team("t"), Preset::Active);
        assert!(!created(
            &mut store,
            &team("u"),
            row("2", "new", ("Todo", "unstarted"), 0)
        ));
        assert!(store.issues[IssueSource::Team].items.is_empty());
    }

    /// A refused change reads the issue back from Linear.
    #[test]
    fn a_refused_change_reads_the_issue_back() {
        assert_eq!(change_refused(&id()), Request::Detail { issue_id: id() });
    }

    // ------------------------------------------------- details, palette

    /// Details from Linear reach every copy; a list copy keeps its own
    /// thread state.
    #[test]
    fn details_reach_every_copy() {
        let mut store = store_with_issue();
        let mut fresh = row("i1", "renamed", ("Done", "completed"), 0);
        fresh.identifier = "ENG-1".into();
        fresh.comments = Some(serde_json::from_str(r#"{"nodes":[]}"#).unwrap());
        assert!(!take_detail(&mut store, fresh));
        assert_eq!(store.current_issue.as_ref().unwrap().title, "renamed");
        let listed = &store.issues[IssueSource::Team].items[0];
        assert_eq!(listed.title, "renamed");
        assert!(listed.comments.is_none());
    }

    /// An issue opened by its identifier takes its real id when Linear
    /// answers.
    #[test]
    fn an_issue_opened_by_identifier_learns_its_id() {
        // Opened by identifier, which stands in for the id until Linear answers.
        let mut store = Store {
            current_issue: Some(
                serde_json::from_str(r#"{"id":"ENG-7","identifier":"ENG-7","title":""}"#).unwrap(),
            ),
            ..Store::default()
        };
        let mut real = row("uuid-7", "Real", ("Todo", "unstarted"), 0);
        real.identifier = "ENG-7".into();
        assert!(take_detail(&mut store, real));
        assert_eq!(store.current_issue.as_ref().unwrap().id, "uuid-7");
    }

    /// An open issue without its thread asks for it; one with it asks for
    /// nothing, and so does no open issue.
    #[test]
    fn a_missing_thread_is_read() {
        let mut store = store_with_issue();
        assert_eq!(
            ensure_thread(&store),
            Some(Request::Detail { issue_id: id() })
        );
        store.current_issue.as_mut().unwrap().comments =
            Some(serde_json::from_str(r#"{"nodes":[]}"#).unwrap());
        assert_eq!(ensure_thread(&store), None);
        assert_eq!(ensure_thread(&Store::default()), None);
    }

    /// A palette search is numbered and sent without surrounding blanks.
    #[test]
    fn a_palette_search_is_numbered_and_trimmed() {
        assert_eq!(
            quick_search("  sunrise ", 3),
            Request::QuickSearch {
                term: "sunrise".into(),
                seq: 3
            }
        );
    }
}
