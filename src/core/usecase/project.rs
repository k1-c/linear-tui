//! Projects: a team's, and a saved project view's, and each with its
//! milestones.
//!
//! **Lists.** [`open_team_projects`] and [`open_view_projects`] list them;
//! [`next_page`], [`reload`], and [`take_page`] follow the list as pages
//! land. A project's issues are an issue list: `issue::open_project_issues`.
//!
//! **One project.** [`find`] finds one by name, [`open`] reads it with its
//! milestones, and [`open_url`] shows it on linear.app. [`load_statuses`]
//! asks for the statuses a project can be in.
//!
//! **Changes.** [`create`], [`update`], and [`delete`] a project;
//! [`add_milestone`], [`update_milestone`], and [`delete_milestone`] its
//! milestones. Nothing on screen changes before Linear answers, except that
//! a deleted project leaves the lists at once.

use super::{Open, Refusal};
use crate::core::entity::{
    CustomViewId, MilestoneId, Page, Priority, Project, ProjectId, ProjectStatusId, TeamId, UserId,
};
use crate::core::store::{List, ListOf, Store};

/// What the project use cases ask of Linear, and of the desktop.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// A page of a team's projects.
    TeamProjects {
        team_id: TeamId,
        after: Option<String>,
    },
    /// A page of a saved project view's projects. Linear evaluates the view's
    /// filter.
    ViewProjects {
        view_id: CustomViewId,
        after: Option<String>,
    },
    /// Show the project's page on linear.app in the desktop's browser.
    OpenInBrowser(String),
    /// The projects of the workspace with this name, in any case.
    Find {
        name: String,
    },
    /// One project, with its teams and milestones.
    Detail {
        project_id: ProjectId,
    },
    /// The statuses a project can be in.
    Statuses,
    Create {
        draft: Draft,
    },
    Update {
        project_id: ProjectId,
        changes: Changes,
    },
    Delete {
        project_id: ProjectId,
    },
    CreateMilestone {
        project_id: ProjectId,
        draft: MilestoneDraft,
    },
    UpdateMilestone {
        milestone_id: MilestoneId,
        changes: MilestoneChanges,
    },
    DeleteMilestone {
        milestone_id: MilestoneId,
    },
}

impl Request {
    /// The page cursor this request continues from, if it asks for a next
    /// page.
    pub fn cursor(&self) -> Option<&str> {
        match self {
            Self::TeamProjects { after, .. } | Self::ViewProjects { after, .. } => after.as_deref(),
            _ => None,
        }
    }
}

/// What a new project is made with. Dates are `YYYY-MM-DD`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Draft {
    /// Required.
    pub name: String,
    /// At least one.
    pub team_ids: Vec<TeamId>,
    /// Markdown.
    pub description: Option<String>,
    pub lead_id: Option<UserId>,
    pub status_id: Option<ProjectStatusId>,
    pub priority: Option<Priority>,
    pub start_date: Option<String>,
    pub target_date: Option<String>,
}

/// What an update changes on a project. A field left `None` stays as it
/// is; for one that can be emptied, `Some(None)` empties it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Changes {
    pub name: Option<String>,
    pub description: Option<String>,
    pub lead_id: Option<Option<UserId>>,
    pub status_id: Option<ProjectStatusId>,
    pub priority: Option<Priority>,
    pub start_date: Option<Option<String>>,
    pub target_date: Option<Option<String>>,
}

impl Changes {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// What a new milestone is made with.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MilestoneDraft {
    /// Required.
    pub name: String,
    pub description: Option<String>,
    /// `YYYY-MM-DD`.
    pub target_date: Option<String>,
}

/// What an update changes on a milestone, as [`Changes`] does.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MilestoneChanges {
    pub name: Option<String>,
    pub description: Option<String>,
    pub target_date: Option<Option<String>>,
}

/// Refuse a name given, and empty.
fn named(name: Option<&String>) -> Result<(), Refusal> {
    match name {
        Some(name) if name.trim().is_empty() => Err(Refusal::NameRequired),
        _ => Ok(()),
    }
}

/// Refuse a target date before the start date, when both are known.
/// Dates in `YYYY-MM-DD` sort as text.
fn in_order(start: Option<&str>, target: Option<&str>) -> Result<(), Refusal> {
    match (start, target) {
        (Some(start), Some(target)) if target < start => Err(Refusal::EndsBeforeItStarts {
            start: start.to_string(),
            target: target.to_string(),
        }),
        _ => Ok(()),
    }
}

/// Which list of projects: the team's, or a saved view's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Projects {
    Team,
    View,
}

impl Projects {
    fn list(self) -> List {
        match self {
            Self::Team => List::Projects,
            Self::View => List::ViewProjects,
        }
    }
}

/// **Browse a team's projects.** A list already holding them is not fetched
/// again; another team's projects are dropped first.
pub fn open_team_projects(store: &mut Store, team_id: TeamId) -> Open {
    store
        .open_list(List::Projects, ListOf::TeamProjects(team_id))
        .into()
}

/// **Open a saved project view.** Linear evaluates its filter; another
/// view's projects are dropped first.
pub fn open_view_projects(store: &mut Store, view_id: CustomViewId) -> Open {
    store
        .open_list(List::ViewProjects, ListOf::ViewProjects(view_id))
        .into()
}

/// **Scroll on to the next page** of projects, asked for once.
pub fn next_page(store: &mut Store, projects: Projects) -> Option<super::Request> {
    store.next_page(projects.list()).map(super::Request::page)
}

/// **Refresh a list of projects**: its first page again.
pub fn reload(store: &mut Store, projects: Projects) -> Option<super::Request> {
    store.reload_list(projects.list()).map(super::Request::page)
}

/// **Find a project by name**, anywhere in the workspace, in any case.
///
/// Linear answers every project so named: several teams can each have one,
/// and telling them apart is for whoever asked. An empty name finds nothing
/// and is refused.
pub fn find(name: &str) -> Result<Request, Refusal> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Refusal::NameRequired);
    }
    Ok(Request::Find {
        name: name.to_string(),
    })
}

/// **Read a project**: its fields, the teams it belongs to, and its
/// milestones.
pub fn open(project_id: ProjectId) -> Request {
    Request::Detail { project_id }
}

/// **Know the statuses a project can be in**: the workspace's, from
/// Backlog to Completed.
pub fn load_statuses() -> Request {
    Request::Statuses
}

/// **Create a project** in one or more teams.
///
/// It needs a name and a team, and a target date on or after its start
/// date. An empty description is sent as none.
pub fn create(mut draft: Draft) -> Result<Request, Refusal> {
    if draft.name.trim().is_empty() {
        return Err(Refusal::NameRequired);
    }
    if draft.team_ids.is_empty() {
        return Err(Refusal::TeamRequired);
    }
    in_order(draft.start_date.as_deref(), draft.target_date.as_deref())?;
    draft.description = draft.description.filter(|d| !d.trim().is_empty());
    Ok(Request::Create { draft })
}

/// **Change a project**: its name, description, lead, status, priority,
/// and dates.
///
/// A change of nothing is refused, and so is an empty name. A target date
/// before the start date is refused when both are given; one given alone is
/// Linear's to check against the date it holds. Every list holding the
/// project shows the new name at once.
pub fn update(
    store: &mut Store,
    project_id: &ProjectId,
    changes: Changes,
) -> Result<Request, Refusal> {
    if changes.is_empty() {
        return Err(Refusal::NothingToChange);
    }
    named(changes.name.as_ref())?;
    in_order(
        changes.start_date.clone().flatten().as_deref(),
        changes.target_date.clone().flatten().as_deref(),
    )?;
    if let Some(name) = &changes.name {
        for rows in [&mut store.projects, &mut store.view_projects] {
            for project in rows.items.iter_mut().filter(|p| &p.id == project_id) {
                project.name = name.clone();
            }
        }
    }
    Ok(Request::Update {
        project_id: project_id.clone(),
        changes,
    })
}

/// **Delete a project.** Linear moves it to its trash, where it can be
/// restored for a while; its issues stay, without a project. It leaves every
/// list at once.
pub fn delete(store: &mut Store, project_id: &ProjectId) -> Request {
    for rows in [&mut store.projects, &mut store.view_projects] {
        rows.items.retain(|p| &p.id != project_id);
    }
    Request::Delete {
        project_id: project_id.clone(),
    }
}

/// **Add a milestone** to a project. It needs a name.
pub fn add_milestone(project_id: ProjectId, mut draft: MilestoneDraft) -> Result<Request, Refusal> {
    if draft.name.trim().is_empty() {
        return Err(Refusal::NameRequired);
    }
    draft.description = draft.description.filter(|d| !d.trim().is_empty());
    Ok(Request::CreateMilestone { project_id, draft })
}

/// **Change a milestone**: its name, description, or target date. A change
/// of nothing is refused, and so is an empty name.
pub fn update_milestone(
    milestone_id: MilestoneId,
    changes: MilestoneChanges,
) -> Result<Request, Refusal> {
    if changes == MilestoneChanges::default() {
        return Err(Refusal::NothingToChange);
    }
    named(changes.name.as_ref())?;
    Ok(Request::UpdateMilestone {
        milestone_id,
        changes,
    })
}

/// **Delete a milestone.** Its issues stay in the project, under no
/// milestone.
pub fn delete_milestone(milestone_id: MilestoneId) -> Request {
    Request::DeleteMilestone { milestone_id }
}

/// **Open a project on linear.app** (`o`). Without a URL there is nothing
/// to open.
pub fn open_url(url: Option<String>) -> Result<Request, Refusal> {
    url.map(Request::OpenInBrowser)
        .ok_or(Refusal::NothingToOpen)
}

/// **A page of projects lands**, and is taken only while the list still
/// belongs to what it was fetched for. Returns whether it was taken.
pub fn take_page(store: &mut Store, projects: Projects, of: &ListOf, page: Page<Project>) -> bool {
    match projects {
        Projects::Team => store.projects.accept_for(of, page),
        Projects::View => store.view_projects.accept_for(of, page),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(id: &str) -> Project {
        serde_json::from_str(&format!(r#"{{"id":"{id}","name":"{id}"}}"#)).unwrap()
    }

    fn draft(name: &str) -> Draft {
        Draft {
            name: name.into(),
            team_ids: vec![TeamId::from("t")],
            ..Draft::default()
        }
    }

    /// A project is found by name, workspace-wide; an empty name is refused.
    #[test]
    fn a_project_is_found_by_its_name() {
        assert_eq!(
            find("  Forecast v2 "),
            Ok(Request::Find {
                name: "Forecast v2".into()
            })
        );
        assert_eq!(find(" "), Err(Refusal::NameRequired));
    }

    /// Reading a project asks for it, milestones and all; its statuses are
    /// the workspace's.
    #[test]
    fn a_project_is_read_with_its_milestones() {
        assert_eq!(
            open(ProjectId::from("p")),
            Request::Detail {
                project_id: "p".into()
            }
        );
        assert_eq!(load_statuses(), Request::Statuses);
    }

    /// A new project needs a name and a team.
    #[test]
    fn a_new_project_needs_a_name_and_a_team() {
        assert_eq!(create(draft(" ")), Err(Refusal::NameRequired));
        let teamless = Draft {
            team_ids: Vec::new(),
            ..draft("Launch")
        };
        assert_eq!(create(teamless), Err(Refusal::TeamRequired));
    }

    /// A project cannot be due before it starts.
    #[test]
    fn a_project_cannot_end_before_it_starts() {
        let backwards = Draft {
            start_date: Some("2026-10-01".into()),
            target_date: Some("2026-09-01".into()),
            ..draft("Launch")
        };
        assert_eq!(
            create(backwards),
            Err(Refusal::EndsBeforeItStarts {
                start: "2026-10-01".into(),
                target: "2026-09-01".into()
            })
        );
    }

    /// A project is made as drafted, an empty description sent as none.
    #[test]
    fn a_new_project_is_made_as_drafted() {
        let described = Draft {
            description: Some("".into()),
            target_date: Some("2026-12-01".into()),
            ..draft("Launch")
        };
        let Ok(Request::Create { draft }) = create(described) else {
            panic!("expected a create");
        };
        assert_eq!(draft.description, None);
        assert_eq!(draft.target_date.as_deref(), Some("2026-12-01"));
    }

    /// A change of nothing, or to an empty name, is refused.
    #[test]
    fn an_empty_project_change_is_refused() {
        let mut store = Store::default();
        let p = ProjectId::from("p");
        assert_eq!(
            update(&mut store, &p, Changes::default()),
            Err(Refusal::NothingToChange)
        );
        let unnamed = Changes {
            name: Some("".into()),
            ..Changes::default()
        };
        assert_eq!(update(&mut store, &p, unnamed), Err(Refusal::NameRequired));
    }

    /// A renamed project shows its new name in the lists at once, and an
    /// emptied date is asked of Linear as none.
    #[test]
    fn a_renamed_project_shows_at_once() {
        let mut store = Store::default();
        store.projects.items = vec![project("p")];
        let changes = Changes {
            name: Some("Renamed".into()),
            target_date: Some(None),
            ..Changes::default()
        };
        let request = update(&mut store, &ProjectId::from("p"), changes.clone());
        assert_eq!(
            request,
            Ok(Request::Update {
                project_id: "p".into(),
                changes
            })
        );
        assert_eq!(store.projects.items[0].name, "Renamed");
    }

    /// A deleted project leaves the lists at once.
    #[test]
    fn a_deleted_project_leaves_the_lists() {
        let mut store = Store::default();
        store.projects.items = vec![project("p"), project("q")];
        assert_eq!(
            delete(&mut store, &ProjectId::from("p")),
            Request::Delete {
                project_id: "p".into()
            }
        );
        assert_eq!(store.projects.items.len(), 1);
    }

    /// A milestone needs a name; one is added as drafted.
    #[test]
    fn a_milestone_needs_a_name() {
        let p = || ProjectId::from("p");
        assert_eq!(
            add_milestone(p(), MilestoneDraft::default()),
            Err(Refusal::NameRequired)
        );
        let beta = MilestoneDraft {
            name: "Beta".into(),
            ..MilestoneDraft::default()
        };
        assert_eq!(
            add_milestone(p(), beta.clone()),
            Ok(Request::CreateMilestone {
                project_id: p(),
                draft: beta
            })
        );
    }

    /// A milestone change of nothing, or to an empty name, is refused; a
    /// milestone is deleted by its id.
    #[test]
    fn milestones_are_changed_and_deleted() {
        let m = || MilestoneId::from("m");
        assert_eq!(
            update_milestone(m(), MilestoneChanges::default()),
            Err(Refusal::NothingToChange)
        );
        let unnamed = MilestoneChanges {
            name: Some(" ".into()),
            ..MilestoneChanges::default()
        };
        assert_eq!(update_milestone(m(), unnamed), Err(Refusal::NameRequired));
        let due = MilestoneChanges {
            target_date: Some(Some("2026-11-01".into())),
            ..MilestoneChanges::default()
        };
        assert!(update_milestone(m(), due).is_ok());
        assert_eq!(
            delete_milestone(m()),
            Request::DeleteMilestone { milestone_id: m() }
        );
    }

    /// A project opens on linear.app by its URL; without one there is
    /// nothing to open.
    #[test]
    fn a_project_opens_by_its_url() {
        assert_eq!(
            open_url(Some("https://linear.app/p".into())),
            Ok(Request::OpenInBrowser("https://linear.app/p".into()))
        );
        assert_eq!(open_url(None), Err(Refusal::NothingToOpen));
    }

    /// A team's projects are asked for once, then shown from what is held.
    #[test]
    fn a_teams_projects_are_fetched_once() {
        let mut store = Store::default();
        assert_eq!(
            open_team_projects(&mut store, TeamId::from("t")).request(),
            Some(crate::core::usecase::Request::Project(
                Request::TeamProjects {
                    team_id: "t".into(),
                    after: None
                }
            ))
        );
        let of = ListOf::TeamProjects(TeamId::from("t"));
        take_page(
            &mut store,
            Projects::Team,
            &of,
            Page::new(vec![project("p")], Default::default(), false),
        );
        assert_eq!(
            open_team_projects(&mut store, TeamId::from("t")),
            Open::Cached
        );
    }

    /// Projects fetched for a team the user has left are dropped.
    #[test]
    fn projects_for_a_team_left_behind_are_dropped() {
        let mut store = Store::default();
        open_team_projects(&mut store, TeamId::from("u"));
        let of = ListOf::TeamProjects(TeamId::from("t"));
        let page = Page::new(vec![project("p")], Default::default(), false);
        assert!(!take_page(&mut store, Projects::Team, &of, page));
    }

    /// A project view asks for its own projects, apart from the team's.
    #[test]
    fn a_project_view_lists_its_own_projects() {
        let mut store = Store::default();
        assert_eq!(
            open_view_projects(&mut store, CustomViewId::from("v")).request(),
            Some(crate::core::usecase::Request::Project(
                Request::ViewProjects {
                    view_id: "v".into(),
                    after: None
                }
            ))
        );
        assert!(store.projects.of.is_none());
        assert_eq!(
            reload(&mut store, Projects::View),
            Some(crate::core::usecase::Request::Project(
                Request::ViewProjects {
                    view_id: "v".into(),
                    after: None
                }
            ))
        );
        assert_eq!(next_page(&mut store, Projects::View), None);
    }

    /// A team's projects reload from their first page, and ask for no next
    /// page before one has landed.
    #[test]
    fn a_teams_projects_reload_from_the_first_page() {
        let mut store = Store::default();
        open_team_projects(&mut store, TeamId::from("t"));
        assert_eq!(
            reload(&mut store, Projects::Team),
            Some(crate::core::usecase::Request::Project(
                Request::TeamProjects {
                    team_id: "t".into(),
                    after: None
                }
            ))
        );
        assert_eq!(next_page(&mut store, Projects::Team), None);
    }
}
