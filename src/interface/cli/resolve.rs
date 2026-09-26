//! Turning the names an agent types into what Linear knows: a person, a
//! label, a project, a cycle, a parent issue. Names resolve within one
//! team — the issue's own, or the team an issue is filed in — in any case.
//! A name that matches nothing lists what would; one that matches more than
//! one thing lists those.

use anyhow::{Result, bail};

use super::Linear;
use crate::core::entity::timestamp::parse_timestamp;
use crate::core::entity::{
    CustomView, Cycle, IssueRef, Label, Milestone, Priority, Project, ProjectId, ProjectStatus,
    Team, TeamId, User,
};
use crate::core::message::Message;
use crate::core::store::Store;
use crate::core::usecase;

/// A value given as `none` empties the field.
pub fn is_none(text: &str) -> bool {
    text.trim().eq_ignore_ascii_case("none")
}

/// The one of `items` that `wanted` names, by any of its `names`. `what`
/// names the kind of thing in the error, and `label` each candidate.
pub fn pick<'a, T>(
    what: &str,
    wanted: &str,
    items: &'a [T],
    names: impl Fn(&T) -> Vec<String>,
    label: impl Fn(&T) -> String,
) -> Result<&'a T> {
    let wanted = wanted.trim();
    let found: Vec<&T> = items
        .iter()
        .filter(|item| names(item).iter().any(|n| n.eq_ignore_ascii_case(wanted)))
        .collect();
    match found.as_slice() {
        [one] => Ok(one),
        [] => {
            let mut choices: Vec<String> = items.iter().map(&label).collect();
            choices.dedup();
            if choices.is_empty() {
                bail!("no {what} {wanted}: this team has none")
            }
            bail!("no {what} {wanted} (choices: {})", choices.join(", "))
        }
        many => {
            let candidates: Vec<String> = many.iter().map(|item| label(item)).collect();
            bail!(
                "{what} {wanted} is ambiguous: it could be {}",
                candidates.join(", ")
            )
        }
    }
}

pub fn priority(level: &str) -> Result<Priority> {
    Priority::ALL
        .into_iter()
        .find(|p| p.label().eq_ignore_ascii_case(level.trim()))
        .ok_or_else(|| {
            anyhow::anyhow!("no priority {level} (choices: urgent, high, medium, low, none)")
        })
}

/// `3`, or `none`.
pub fn estimate(text: &str) -> Result<Option<u32>> {
    if is_none(text) {
        return Ok(None);
    }
    text.trim()
        .parse()
        .map(Some)
        .map_err(|_| anyhow::anyhow!("not an estimate: {text} (a whole number, or none)"))
}

fn person_name(user: &User) -> String {
    user.display_name
        .clone()
        .unwrap_or_else(|| user.name.clone())
}

/// What a team offers to fill an issue's fields with, fetched as asked for.
pub struct InTeam<'a, L: Linear> {
    linear: &'a L,
    pub id: TeamId,
    members: Option<Vec<User>>,
}

impl<'a, L: Linear> InTeam<'a, L> {
    pub fn new(linear: &'a L, id: TeamId) -> Self {
        Self {
            linear,
            id,
            members: None,
        }
    }

    async fn members(&mut self) -> Result<&[User]> {
        if self.members.is_none() {
            let request = usecase::team::Request::Context {
                team_id: self.id.clone(),
            };
            let Message::TeamContext { members, .. } = self.linear.run(request).await? else {
                bail!("unexpected answer to a team request");
            };
            self.members = Some(members);
        }
        Ok(self.members.as_deref().unwrap_or_default())
    }

    /// `me`, a member of the team by name, display name, or email, or
    /// `none` for nobody.
    pub async fn assignee(&mut self, text: &str) -> Result<Option<User>> {
        if is_none(text) {
            return Ok(None);
        }
        if text.trim().eq_ignore_ascii_case("me") {
            let Message::Viewer { id, .. } = self.linear.run(usecase::user::load()).await? else {
                bail!("unexpected answer to a viewer request");
            };
            let me = self.members().await?.iter().find(|u| u.id == id).cloned();
            return Ok(Some(me.unwrap_or(User {
                id,
                name: "me".into(),
                email: None,
                display_name: None,
            })));
        }
        let members = self.members().await?;
        let user = pick(
            "member",
            text,
            members,
            |u| {
                let mut names = vec![u.name.clone()];
                names.extend(u.display_name.clone());
                names.extend(u.email.clone());
                names
            },
            |u| match &u.email {
                Some(email) => format!("{} <{email}>", person_name(u)),
                None => person_name(u),
            },
        )?;
        Ok(Some(user.clone()))
    }

    /// Labels by name: the team's own and the workspace's.
    pub async fn labels(&self, names: &[&str]) -> Result<Vec<Label>> {
        if names.is_empty() {
            return Ok(Vec::new());
        }
        let request = usecase::team::load_labels(self.id.clone());
        let Message::Labels { labels, team_id } = self.linear.run(request).await? else {
            bail!("unexpected answer to a labels request");
        };
        if team_id != self.id {
            bail!("Linear answered with another team's labels");
        }
        names
            .iter()
            .map(|name| {
                pick(
                    "label",
                    name,
                    &labels,
                    |l| vec![l.name.clone()],
                    |l| l.name.clone(),
                )
                .cloned()
            })
            .collect()
    }

    /// A project of the team by name, or `none`.
    pub async fn project(&self, text: &str) -> Result<Option<Project>> {
        if is_none(text) {
            return Ok(None);
        }
        let open = usecase::project::open_team_projects(&mut Store::default(), self.id.clone());
        let Some(request) = open.request() else {
            bail!("the team's projects could not be asked for");
        };
        let Message::Projects { page, .. } = self.linear.run(request).await? else {
            bail!("unexpected answer to a projects request");
        };
        let project = pick(
            "project",
            text,
            &page.items,
            |p| vec![p.name.clone()],
            |p| p.name.clone(),
        )?;
        Ok(Some(project.clone()))
    }

    /// A cycle of the team by name or number, `current` for the one under
    /// way at `now` (seconds since the epoch), or `none`.
    pub async fn cycle(&self, text: &str, now: u64) -> Result<Option<Cycle>> {
        if is_none(text) {
            return Ok(None);
        }
        let open = usecase::cycle::open_team_cycles(&mut Store::default(), self.id.clone());
        let Some(request) = open.request() else {
            bail!("the team's cycles could not be asked for");
        };
        let Message::Cycles { page, .. } = self.linear.run(request).await? else {
            bail!("unexpected answer to a cycles request");
        };
        if text.trim().eq_ignore_ascii_case("current") {
            return current_cycle(&page.items, now)
                .cloned()
                .map(Some)
                .ok_or_else(|| anyhow::anyhow!("the team has no cycle under way"));
        }
        let cycle = pick("cycle", text, &page.items, cycle_names, Cycle::label)?;
        Ok(Some(cycle.clone()))
    }
}

fn cycle_names(cycle: &Cycle) -> Vec<String> {
    let mut names = vec![cycle.label()];
    names.extend(cycle.name.clone());
    if let Some(number) = cycle.number {
        names.push(format!("{number}"));
        names.push(format!("Cycle {number}"));
    }
    names
}

/// The cycle whose dates hold `now`.
pub fn current_cycle(cycles: &[Cycle], now: u64) -> Option<&Cycle> {
    // Linear writes `2026-09-26T00:00:00.000Z`; the seconds are enough.
    let at = |text: &Option<String>| {
        let text = text.as_deref()?;
        parse_timestamp(&format!("{}Z", text.get(..19)?))
    };
    cycles.iter().find(|c| {
        matches!((at(&c.starts_at), at(&c.ends_at)), (Some(start), Some(end)) if start <= now && now < end)
    })
}

/// The issue `key` names, as a parent: `none` for no parent.
pub async fn parent(linear: &impl Linear, key: &str) -> Result<Option<IssueRef>> {
    if is_none(key) {
        return Ok(None);
    }
    let issue = super::issue::fetch(linear, key).await?;
    Ok(Some(IssueRef {
        id: issue.id,
        identifier: issue.identifier,
        title: issue.title,
        state: issue.state,
    }))
}

/// Every team the user is in.
pub async fn teams(linear: &impl Linear) -> Result<Vec<Team>> {
    let Message::Teams(teams) = linear.run(usecase::team::load()).await? else {
        bail!("unexpected answer to a teams request");
    };
    Ok(teams)
}

/// A team by key or name.
pub fn team<'a>(teams: &'a [Team], wanted: &str) -> Result<&'a Team> {
    pick(
        "team",
        wanted,
        teams,
        |t| vec![t.key.clone(), t.name.clone()],
        |t| t.key.clone(),
    )
}

/// A team of the user's by key or name, fetched.
pub async fn one_team(linear: &impl Linear, wanted: &str) -> Result<Team> {
    Ok(team(&teams(linear).await?, wanted)?.clone())
}

/// A project anywhere in the workspace, by name in any case or by id. Two
/// projects of one name are told apart by their teams.
pub async fn project(linear: &impl Linear, wanted: &str) -> Result<Project> {
    let request = usecase::project::find(wanted)?;
    let Message::ProjectsFound { projects, name } = linear.run(request).await? else {
        bail!("unexpected answer to a project search");
    };
    let teams_of = |p: &Project| {
        p.teams.as_ref().map_or(String::new(), |t| {
            let keys: Vec<&str> = t.nodes.iter().map(|t| t.key.as_str()).collect();
            format!(" ({})", keys.join(", "))
        })
    };
    match projects.as_slice() {
        [one] => return Ok(one.clone()),
        [] => {}
        many => {
            let candidates: Vec<String> = many
                .iter()
                .map(|p| format!("{}{} — id {}", p.name, teams_of(p), p.id))
                .collect();
            bail!(
                "project {wanted} is ambiguous: it could be {}; name it by id",
                candidates.join(", ")
            )
        }
    }
    // Not a name: perhaps an id.
    let looks_like_id = wanted.len() >= 8
        && wanted
            .trim()
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-');
    if looks_like_id {
        return project_detail(linear, &ProjectId::new(wanted.trim())).await;
    }
    bail!("no project {name} in this workspace")
}

/// A project read with its teams and milestones.
pub async fn project_detail(linear: &impl Linear, id: &ProjectId) -> Result<Project> {
    let Message::ProjectDetail(project) = linear.run(usecase::project::open(id.clone())).await?
    else {
        bail!("unexpected answer to a project request");
    };
    Ok(*project)
}

/// A milestone of `project` (read with its milestones) by name or id, or
/// `none`.
pub fn milestone(project: &Project, wanted: &str) -> Result<Option<Milestone>> {
    if is_none(wanted) {
        return Ok(None);
    }
    let milestones = project
        .milestones
        .as_ref()
        .map_or(&[][..], |m| m.nodes.as_slice());
    let found = pick(
        "milestone",
        wanted,
        milestones,
        |m| vec![m.name.clone(), m.id.to_string()],
        |m| m.name.clone(),
    )
    .map_err(|e| anyhow::anyhow!("{e}, in project {}", project.name))?;
    Ok(Some(found.clone()))
}

/// One of the workspace's project statuses, by name.
pub async fn project_status(linear: &impl Linear, wanted: &str) -> Result<ProjectStatus> {
    let request = usecase::project::load_statuses();
    let Message::ProjectStatuses(statuses) = linear.run(request).await? else {
        bail!("unexpected answer to a project statuses request");
    };
    Ok(pick(
        "project status",
        wanted,
        &statuses,
        |s| vec![s.name.clone()],
        |s| s.name.clone(),
    )?
    .clone())
}

/// A date as Linear takes it, `YYYY-MM-DD`, or `none`.
pub fn date(text: &str) -> Result<Option<String>> {
    if is_none(text) {
        return Ok(None);
    }
    let text = text.trim();
    let digits = |r: std::ops::Range<usize>| {
        text.get(r)
            .is_some_and(|s| s.chars().all(|c| c.is_ascii_digit()))
    };
    let valid = text.len() == 10
        && digits(0..4)
        && &text[4..5] == "-"
        && digits(5..7)
        && &text[7..8] == "-"
        && digits(8..10)
        && parse_timestamp(&format!("{text}T00:00:00Z")).is_some();
    if !valid {
        bail!("not a date: {text} (YYYY-MM-DD, or none)");
    }
    Ok(Some(text.to_string()))
}

/// Every saved view the user can open, in Linear's order.
pub async fn views(linear: &impl Linear) -> Result<Vec<CustomView>> {
    let mut store = Store::default();
    let request = usecase::view::reload_views(&mut store);
    let Message::CustomViews(views) = linear.run(request).await? else {
        bail!("unexpected answer to a views request");
    };
    usecase::view::take_views(&mut store, views);
    Ok(store.custom_views)
}

/// A saved view by name: one listing issues, or one listing projects.
pub async fn view(linear: &impl Linear, wanted: &str, issues: bool) -> Result<CustomView> {
    let views: Vec<CustomView> = views(linear)
        .await?
        .into_iter()
        .filter(|v| v.lists_issues() == issues)
        .collect();
    let what = if issues { "issue view" } else { "project view" };
    Ok(pick(
        what,
        wanted,
        &views,
        |v| vec![v.name.clone()],
        |v| v.name.clone(),
    )?
    .clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_calendar_days_or_none() {
        assert_eq!(date("2026-12-01").unwrap().as_deref(), Some("2026-12-01"));
        assert_eq!(date("none").unwrap(), None);
        assert!(date("2026-13-01").is_err());
        assert!(date("12/01/2026").is_err());
    }

    fn labels() -> Vec<Label> {
        serde_json::from_value(serde_json::json!([
            { "id": "l1", "name": "Bug" },
            { "id": "l2", "name": "bug" },
            { "id": "l3", "name": "Feature" },
        ]))
        .unwrap()
    }

    fn by_name(l: &Label) -> Vec<String> {
        vec![l.name.clone()]
    }

    #[test]
    fn a_name_matches_in_any_case() {
        let labels = labels();
        let found = pick("label", "feature", &labels, by_name, |l| l.name.clone()).unwrap();
        assert_eq!(found.id, "l3");
    }

    #[test]
    fn an_unknown_name_lists_the_choices() {
        let labels = labels();
        let err = pick("label", "Chore", &labels, by_name, |l| l.name.clone()).unwrap_err();
        assert_eq!(
            err.to_string(),
            "no label Chore (choices: Bug, bug, Feature)"
        );
    }

    #[test]
    fn an_ambiguous_name_lists_the_candidates() {
        let labels = labels();
        let err = pick("label", "BUG", &labels, by_name, |l| l.name.clone()).unwrap_err();
        assert_eq!(
            err.to_string(),
            "label BUG is ambiguous: it could be Bug, bug"
        );
    }

    #[test]
    fn priorities_and_estimates_parse_or_say_what_is_valid() {
        assert_eq!(priority("LOW").unwrap(), Priority::Low);
        assert!(
            priority("huge")
                .unwrap_err()
                .to_string()
                .contains("urgent, high")
        );
        assert_eq!(estimate("3").unwrap(), Some(3));
        assert_eq!(estimate("none").unwrap(), None);
        assert!(estimate("two").is_err());
    }

    #[test]
    fn the_current_cycle_is_the_one_under_way() {
        let cycles: Vec<Cycle> = serde_json::from_value(serde_json::json!([
            { "id": "c1", "number": 1, "startsAt": "2026-09-01T00:00:00.000Z", "endsAt": "2026-09-15T00:00:00.000Z" },
            { "id": "c2", "number": 2, "startsAt": "2026-09-15T00:00:00.000Z", "endsAt": "2026-09-29T00:00:00.000Z" },
        ]))
        .unwrap();
        let now = parse_timestamp("2026-09-26T12:00:00Z").unwrap();
        assert_eq!(current_cycle(&cycles, now).unwrap().id, "c2");
        let later = parse_timestamp("2026-10-26T12:00:00Z").unwrap();
        assert!(current_cycle(&cycles, later).is_none());
        let cycle = pick("cycle", "cycle 1", &cycles, cycle_names, Cycle::label).unwrap();
        assert_eq!(cycle.id, "c1");
    }
}
