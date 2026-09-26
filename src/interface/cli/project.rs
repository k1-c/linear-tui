//! `linear-tui project list / show / create / update / delete` and
//! `linear-tui milestone list / create / update / delete`.
//!
//! A project is named by name, in any case, anywhere in the workspace, or
//! by id when two share a name; a milestone by name or id within its
//! project.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::Linear;
use super::args::{Args, text_or_stdin};
use super::issue::{output, user_name};
use super::resolve;
use crate::core::entity::{Milestone, Page, Project};
use crate::core::message::Message;
use crate::core::store::{ListOf, Store};
use crate::core::usecase;
use crate::core::usecase::project::{Changes, Draft, MilestoneChanges, MilestoneDraft, Projects};

pub async fn run(args: &[String], linear: &impl Linear) -> Result<String> {
    let Some((command, rest)) = args.split_first() else {
        bail!("Usage: linear-tui project <list|show|create|update|delete> …");
    };
    match command.as_str() {
        "list" => list(linear, rest).await,
        "show" => show(linear, rest).await,
        "create" => create(linear, rest).await,
        "update" => update(linear, rest).await,
        "delete" => delete(linear, rest).await,
        other => bail!("unknown command: project {other}"),
    }
}

pub async fn run_milestone(args: &[String], linear: &impl Linear) -> Result<String> {
    let Some((command, rest)) = args.split_first() else {
        bail!("Usage: linear-tui milestone <list|create|update|delete> <project> …");
    };
    match command.as_str() {
        "list" => milestones(linear, rest).await,
        "create" => create_milestone(linear, rest).await,
        "update" => update_milestone(linear, rest).await,
        "delete" => delete_milestone(linear, rest).await,
        other => bail!("unknown command: milestone {other}"),
    }
}

/// `project list`: a team's projects, or a saved project view's.
///
/// Covers: project::open_team_projects, project::open_view_projects,
/// project::next_page, project::reload, project::take_page
async fn list(linear: &impl Linear, args: &[String]) -> Result<String> {
    let usage = "linear-tui project list (--team <key> | --view <name>) [--json]";
    let args = Args::parse(args, &["json"], &["team", "view"])?;
    args.positionals::<0>(usage)?;
    let mut store = Store::default();
    let (which, open, heading) = match (args.value("team"), args.value("view")) {
        (Some(team), None) => {
            let team = resolve::one_team(linear, team).await?;
            let open = usecase::project::open_team_projects(&mut store, team.id.clone());
            (Projects::Team, open, format!("{} projects", team.name))
        }
        (None, Some(view)) => {
            let view = resolve::view(linear, view, false).await?;
            let open = usecase::project::open_view_projects(&mut store, view.id.clone());
            (Projects::View, open, format!("View “{}”", view.name))
        }
        _ => bail!("Usage: {usage}"),
    };
    let mut request = open
        .request()
        .or_else(|| usecase::project::reload(&mut store, which));
    while let Some(asked) = request.take() {
        let (of, page) = project_page(linear.run(asked).await?)?;
        usecase::project::take_page(&mut store, which, &of, page);
        request = usecase::project::next_page(&mut store, which);
    }
    let projects = match which {
        Projects::Team => &store.projects.items,
        Projects::View => &store.view_projects.items,
    };
    let mut out = format!("# {heading} ({})\n\n", projects.len());
    if projects.is_empty() {
        out.push_str("(none)\n");
    }
    for project in projects {
        out.push_str(&format!("- {}{}\n", project.name, facts(project)));
    }
    Ok(output(
        &args,
        out,
        json!({ "list": heading, "projects": projects.iter().map(project_json).collect::<Vec<_>>() }),
    ))
}

/// The page of projects in an answer, and what it lists.
fn project_page(message: Message) -> Result<(ListOf, Page<Project>)> {
    Ok(match message {
        Message::Projects { team_id, page } => (ListOf::TeamProjects(team_id), page),
        Message::ViewProjects { view_id, page } => (ListOf::ViewProjects(view_id), page),
        _ => bail!("unexpected answer to a projects request"),
    })
}

/// ` — In Progress · High · target 2026-12-01`, what there is of it.
fn facts(project: &Project) -> String {
    let mut facts = Vec::new();
    facts.extend(project.status.as_ref().map(|s| s.name.clone()));
    facts.extend(
        project
            .priority_label
            .clone()
            .filter(|p| p != "No priority"),
    );
    facts.extend(project.target_date.as_ref().map(|d| format!("target {d}")));
    if facts.is_empty() {
        String::new()
    } else {
        format!(" — {}", facts.join(" · "))
    }
}

/// `project show`: its fields, teams, description, and milestones; its URL
/// among its fields.
///
/// Covers: project::find, project::open, project::open_url
async fn show(linear: &impl Linear, args: &[String]) -> Result<String> {
    let args = Args::parse(args, &["json"], &[])?;
    let [name] = args.positionals::<1>("linear-tui project show <project> [--json]")?;
    let found = resolve::project(linear, name).await?;
    let project = resolve::project_detail(linear, &found.id).await?;
    Ok(output(&args, render(&project), project_json(&project)))
}

pub fn render(project: &Project) -> String {
    let mut out = format!("# {}\n\n", project.name);
    let mut field = |name: &str, value: Option<String>| {
        if let Some(value) = value.filter(|v| !v.is_empty()) {
            out.push_str(&format!("- {name}: {value}\n"));
        }
    };
    field("Id", Some(project.id.to_string()));
    field("Status", project.status.as_ref().map(|s| s.name.clone()));
    field(
        "Priority",
        project
            .priority_label
            .clone()
            .filter(|p| p != "No priority"),
    );
    field(
        "Lead",
        project.lead.as_ref().map(|u| user_name(u).to_string()),
    );
    field(
        "Teams",
        project.teams.as_ref().map(|t| {
            t.nodes
                .iter()
                .map(|t| t.key.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }),
    );
    field("Start", project.start_date.clone());
    field("Target", project.target_date.clone());
    field(
        "Progress",
        project.progress.map(|p| format!("{:.0}%", p * 100.0)),
    );
    field("URL", project.url.clone());

    let description = project.description.as_deref().unwrap_or("").trim();
    out.push_str("\n## Description\n\n");
    out.push_str(if description.is_empty() {
        "(none)"
    } else {
        description
    });
    out.push('\n');

    if let Some(milestones) = &project.milestones {
        out.push_str(&format!("\n## Milestones ({})\n\n", milestones.nodes.len()));
        for milestone in &milestones.nodes {
            out.push_str(&format!("- {}\n", milestone_line(milestone)));
        }
    }
    out
}

/// `Beta — target 2026-11-01 (id 3f2c…)`.
fn milestone_line(milestone: &Milestone) -> String {
    let target = milestone
        .target_date
        .as_ref()
        .map_or(String::new(), |d| format!(" — target {d}"));
    format!("{}{target} (id {})", milestone.name, milestone.id)
}

pub fn project_json(project: &Project) -> Value {
    json!({
        "id": project.id,
        "name": project.name,
        "url": project.url,
        "status": project.status.as_ref().map(|s| json!({ "name": s.name, "type": s.kind })),
        "priority": project.priority_label,
        "lead": project.lead.as_ref().map(|u| user_name(u).to_string()),
        "teams": project.teams.as_ref().map(|t| t.nodes.iter().map(|t| t.key.clone()).collect::<Vec<_>>()),
        "start_date": project.start_date,
        "target_date": project.target_date,
        "progress": project.progress,
        "description": project.description,
        "milestones": project.milestones.as_ref().map(|m| m.nodes.iter().map(milestone_json).collect::<Vec<_>>()),
    })
}

fn milestone_json(milestone: &Milestone) -> Value {
    json!({
        "id": milestone.id,
        "name": milestone.name,
        "target_date": milestone.target_date,
        "description": milestone.description,
    })
}

/// The options `project create` and `update` share, beyond the name.
const FIELDS: &[&str] = &[
    "description",
    "lead",
    "status",
    "priority",
    "start",
    "target",
];

/// `project create`.
///
/// Covers: project::create, project::load_statuses
async fn create(linear: &impl Linear, args: &[String]) -> Result<String> {
    let usage = "linear-tui project create --team <key>... --name <text> [--description <text>|-] \
                 [--lead <me|name|email>] [--status <name>] [--priority <level>] \
                 [--start <YYYY-MM-DD>] [--target <YYYY-MM-DD>] [--json]";
    let mut valued = vec!["team", "name"];
    valued.extend_from_slice(FIELDS);
    let args = Args::parse(args, &["json"], &valued)?;
    args.positionals::<0>(usage)?;
    let Some(name) = args.value("name") else {
        bail!("Usage: {usage}");
    };
    let teams = resolve::teams(linear).await?;
    let teams: Vec<_> = args
        .all("team")
        .iter()
        .map(|t| resolve::team(&teams, t).cloned())
        .collect::<Result<_>>()?;
    let lead_id = match (args.value("lead"), teams.first()) {
        (Some(who), Some(team)) => resolve::InTeam::new(linear, team.id.clone())
            .assignee(who)
            .await?
            .map(|u| u.id),
        _ => None,
    };
    let draft = Draft {
        name: text_or_stdin(name)?,
        team_ids: teams.iter().map(|t| t.id.clone()).collect(),
        description: args.value("description").map(text_or_stdin).transpose()?,
        lead_id,
        status_id: match args.value("status") {
            Some(name) => Some(resolve::project_status(linear, name).await?.id),
            None => None,
        },
        priority: args.value("priority").map(resolve::priority).transpose()?,
        start_date: args
            .value("start")
            .map(resolve::date)
            .transpose()?
            .flatten(),
        target_date: args
            .value("target")
            .map(resolve::date)
            .transpose()?
            .flatten(),
    };
    let request = usecase::project::create(draft)?;
    let Message::ProjectCreated(project) = linear.run(request).await? else {
        bail!("unexpected answer to a project create");
    };
    let markdown = format!(
        "Created project {} (id {})\n{}",
        project.name,
        project.id,
        project.url.as_deref().unwrap_or_default()
    );
    Ok(output(&args, markdown, project_json(&project)))
}

/// `project update`: any of its fields at once.
///
/// Covers: project::update
async fn update(linear: &impl Linear, args: &[String]) -> Result<String> {
    let usage = "linear-tui project update <project> [--name <text>] [--description <text>|-] \
                 [--lead <me|name|email|none>] [--status <name>] [--priority <level>] \
                 [--start <YYYY-MM-DD|none>] [--target <YYYY-MM-DD|none>] [--json]";
    let mut valued = vec!["name"];
    valued.extend_from_slice(FIELDS);
    let args = Args::parse(args, &["json"], &valued)?;
    let [wanted] = args.positionals::<1>(usage)?;
    if !args.any(&valued) {
        bail!("Nothing to change. Usage: {usage}");
    }
    let found = resolve::project(linear, wanted).await?;
    let project = resolve::project_detail(linear, &found.id).await?;
    let lead = match args.value("lead") {
        Some(who) => {
            // A lead is a member of one of the project's teams.
            let team = project
                .teams
                .as_ref()
                .and_then(|t| t.nodes.first())
                .context("Linear did not say which teams the project is in")?;
            Some(
                resolve::InTeam::new(linear, team.id.clone())
                    .assignee(who)
                    .await?,
            )
        }
        None => None,
    };
    let status = match args.value("status") {
        Some(name) => Some(resolve::project_status(linear, name).await?),
        None => None,
    };
    let changes = Changes {
        name: args.value("name").map(text_or_stdin).transpose()?,
        description: args.value("description").map(text_or_stdin).transpose()?,
        lead_id: lead.as_ref().map(|l| l.as_ref().map(|u| u.id.clone())),
        status_id: status.as_ref().map(|s| s.id.clone()),
        priority: args.value("priority").map(resolve::priority).transpose()?,
        start_date: args.value("start").map(resolve::date).transpose()?,
        target_date: args.value("target").map(resolve::date).transpose()?,
    };
    let mut lines = Vec::new();
    let mut changed = |field: &str, from: Option<String>, to: Option<String>| {
        let text = |v: &Option<String>| v.clone().unwrap_or_else(|| "—".into());
        let line = if field == "description" {
            "description rewritten".to_string()
        } else {
            format!("{field} {} → {}", text(&from), text(&to))
        };
        lines.push((line, json!({ "field": field, "from": from, "to": to })));
    };
    if let Some(name) = &changes.name {
        changed("name", Some(project.name.clone()), Some(name.clone()));
    }
    if let Some(status) = &status {
        changed(
            "status",
            project.status.as_ref().map(|s| s.name.clone()),
            Some(status.name.clone()),
        );
    }
    if let Some(priority) = changes.priority {
        changed(
            "priority",
            project.priority_label.clone(),
            Some(priority.label().to_string()),
        );
    }
    if let Some(lead) = &lead {
        changed(
            "lead",
            project.lead.as_ref().map(|u| user_name(u).to_string()),
            lead.as_ref().map(|u| user_name(u).to_string()),
        );
    }
    if let Some(start) = &changes.start_date {
        changed("start", project.start_date.clone(), start.clone());
    }
    if let Some(target) = &changes.target_date {
        changed("target", project.target_date.clone(), target.clone());
    }
    if let Some(description) = &changes.description {
        changed(
            "description",
            project.description.clone(),
            Some(description.clone()),
        );
    }
    let request = usecase::project::update(&mut Store::default(), &project.id, changes)?;
    linear.run(request).await?;
    let markdown = lines
        .iter()
        .map(|(line, _)| format!("{}: {line}", project.name))
        .collect::<Vec<_>>()
        .join("\n");
    let json = json!({
        "project": project.name,
        "id": project.id,
        "changes": lines.into_iter().map(|(_, v)| v).collect::<Vec<_>>(),
    });
    Ok(output(&args, markdown, json))
}

/// `project delete`: to Linear's trash.
///
/// Covers: project::delete
async fn delete(linear: &impl Linear, args: &[String]) -> Result<String> {
    let args = Args::parse(args, &["json"], &[])?;
    let [wanted] = args.positionals::<1>("linear-tui project delete <project> [--json]")?;
    let project = resolve::project(linear, wanted).await?;
    let request = usecase::project::delete(&mut Store::default(), &project.id);
    linear.run(request).await?;
    Ok(output(
        &args,
        format!(
            "Deleted project {} (id {}); it is in Linear's trash",
            project.name, project.id
        ),
        json!({ "project": project.name, "id": project.id, "deleted": true }),
    ))
}

/// A project named on the command line, read with its milestones.
async fn project_with_milestones(linear: &impl Linear, wanted: &str) -> Result<Project> {
    let found = resolve::project(linear, wanted).await?;
    resolve::project_detail(linear, &found.id).await
}

/// `milestone list`: a project's milestones.
///
/// Covers: project::open
async fn milestones(linear: &impl Linear, args: &[String]) -> Result<String> {
    let args = Args::parse(args, &["json"], &[])?;
    let [wanted] = args.positionals::<1>("linear-tui milestone list <project> [--json]")?;
    let project = project_with_milestones(linear, wanted).await?;
    let milestones = project.milestones.as_ref().map_or(&[][..], |m| &m.nodes);
    let mut out = format!(
        "# Milestones of {} ({})\n\n",
        project.name,
        milestones.len()
    );
    if milestones.is_empty() {
        out.push_str("(none)\n");
    }
    for milestone in milestones {
        out.push_str(&format!("- {}\n", milestone_line(milestone)));
    }
    Ok(output(
        &args,
        out,
        json!({ "project": project.name, "milestones": milestones.iter().map(milestone_json).collect::<Vec<_>>() }),
    ))
}

/// `milestone create`.
///
/// Covers: project::add_milestone
async fn create_milestone(linear: &impl Linear, args: &[String]) -> Result<String> {
    let usage = "linear-tui milestone create <project> --name <text> [--description <text>|-] \
                 [--target <YYYY-MM-DD>] [--json]";
    let args = Args::parse(args, &["json"], &["name", "description", "target"])?;
    let [wanted] = args.positionals::<1>(usage)?;
    let Some(name) = args.value("name") else {
        bail!("Usage: {usage}");
    };
    let project = resolve::project(linear, wanted).await?;
    let draft = MilestoneDraft {
        name: text_or_stdin(name)?,
        description: args.value("description").map(text_or_stdin).transpose()?,
        target_date: args
            .value("target")
            .map(resolve::date)
            .transpose()?
            .flatten(),
    };
    let request = usecase::project::add_milestone(project.id.clone(), draft)?;
    let Message::MilestoneCreated {
        project_id,
        milestone,
    } = linear.run(request).await?
    else {
        bail!("unexpected answer to a milestone create");
    };
    if project_id != project.id {
        bail!("Linear answered for another project");
    }
    Ok(output(
        &args,
        format!(
            "Created milestone {} in {}",
            milestone_line(&milestone),
            project.name
        ),
        milestone_json(&milestone),
    ))
}

/// `milestone update`.
///
/// Covers: project::update_milestone
async fn update_milestone(linear: &impl Linear, args: &[String]) -> Result<String> {
    let usage = "linear-tui milestone update <project> <milestone> [--name <text>] \
                 [--description <text>|-] [--target <YYYY-MM-DD|none>] [--json]";
    let args = Args::parse(args, &["json"], &["name", "description", "target"])?;
    let [wanted, which] = args.positionals::<2>(usage)?;
    if !args.any(&["name", "description", "target"]) {
        bail!("Nothing to change. Usage: {usage}");
    }
    let project = project_with_milestones(linear, wanted).await?;
    let milestone = resolve::milestone(&project, which)?
        .context("name the milestone to change; none is none")?;
    let changes = MilestoneChanges {
        name: args.value("name").map(text_or_stdin).transpose()?,
        description: args.value("description").map(text_or_stdin).transpose()?,
        target_date: args.value("target").map(resolve::date).transpose()?,
    };
    let mut lines = Vec::new();
    if let Some(name) = &changes.name {
        lines.push(format!("name {} → {name}", milestone.name));
    }
    if let Some(target) = &changes.target_date {
        let text = |d: Option<&String>| d.cloned().unwrap_or_else(|| "—".into());
        lines.push(format!(
            "target {} → {}",
            text(milestone.target_date.as_ref()),
            text(target.as_ref())
        ));
    }
    if changes.description.is_some() {
        lines.push("description rewritten".into());
    }
    let request = usecase::project::update_milestone(milestone.id.clone(), changes)?;
    linear.run(request).await?;
    let markdown = lines
        .iter()
        .map(|l| format!("{} › {}: {l}", project.name, milestone.name))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(output(
        &args,
        markdown,
        json!({ "project": project.name, "milestone": milestone.id, "changes": lines }),
    ))
}

/// `milestone delete`.
///
/// Covers: project::delete_milestone
async fn delete_milestone(linear: &impl Linear, args: &[String]) -> Result<String> {
    let args = Args::parse(args, &["json"], &[])?;
    let [wanted, which] =
        args.positionals::<2>("linear-tui milestone delete <project> <milestone> [--json]")?;
    let project = project_with_milestones(linear, wanted).await?;
    let milestone = resolve::milestone(&project, which)?
        .context("name the milestone to delete; none is none")?;
    linear
        .run(usecase::project::delete_milestone(milestone.id.clone()))
        .await?;
    Ok(output(
        &args,
        format!("Deleted milestone {} of {}", milestone.name, project.name),
        json!({ "project": project.name, "milestone": milestone.id, "deleted": true }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> Project {
        serde_json::from_value(json!({
            "id": "p1", "name": "Forecast v2", "url": "https://linear.app/w/project/forecast",
            "status": { "id": "s", "name": "In Progress", "type": "started" },
            "priorityLabel": "High", "targetDate": "2026-12-01", "progress": 0.4,
            "teams": { "nodes": [{ "id": "t", "name": "Weather", "key": "WX" }] },
            "projectMilestones": { "nodes": [
                { "id": "m1", "name": "Beta", "targetDate": "2026-11-01" },
                { "id": "m2", "name": "Launch" }
            ] }
        }))
        .unwrap()
    }

    #[test]
    fn a_project_reads_as_its_fields_and_milestones() {
        let text = render(&project());
        assert!(text.starts_with("# Forecast v2\n"), "{text}");
        for line in [
            "- Status: In Progress\n",
            "- Priority: High\n",
            "- Teams: WX\n",
            "- Target: 2026-12-01\n",
            "- Progress: 40%\n",
            "## Milestones (2)",
            "- Beta — target 2026-11-01 (id m1)\n",
            "- Launch (id m2)\n",
        ] {
            assert!(text.contains(line), "{line:?} in {text}");
        }
        assert_eq!(
            facts(&project()),
            " — In Progress · High · target 2026-12-01"
        );
    }

    #[test]
    fn a_milestone_is_named_by_name_or_id_within_its_project() {
        let project = project();
        let named = |w: &str| resolve::milestone(&project, w).unwrap();
        assert_eq!(named("beta").unwrap().id, "m1");
        assert_eq!(named("m2").unwrap().name, "Launch");
        assert!(named("none").is_none());
        let err = resolve::milestone(&project, "GA").unwrap_err().to_string();
        assert_eq!(
            err,
            "no milestone GA (choices: Beta, Launch), in project Forecast v2"
        );
    }
}
