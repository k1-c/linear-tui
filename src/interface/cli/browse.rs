//! `linear-tui team list / show`, `cycle list`, `view list`, and
//! `favorite list`: what the TUI's sidebar and pages show, read afresh.

use anyhow::{Result, bail};
use serde_json::json;

use super::Linear;
use super::args::Args;
use super::issue::{output, user_name};
use super::resolve;
use crate::core::entity::{Cycle, Team};
use crate::core::message::Message;
use crate::core::store::{ListOf, Store};
use crate::core::usecase;
use crate::core::usecase::favorite::{Target, TeamPage};

pub async fn team(args: &[String], linear: &impl Linear) -> Result<String> {
    match args.split_first() {
        Some((command, rest)) if command == "list" => teams(linear, rest).await,
        Some((command, rest)) if command == "show" => show_team(linear, rest).await,
        _ => bail!("Usage: linear-tui team <list|show <key>> [--json]"),
    }
}

/// `team list`.
///
/// Covers: team::load
async fn teams(linear: &impl Linear, args: &[String]) -> Result<String> {
    let args = Args::parse(args, &["json"], &[])?;
    args.positionals::<0>("linear-tui team list [--json]")?;
    let teams = resolve::teams(linear).await?;
    let mut out = format!("# Teams ({})\n\n", teams.len());
    for team in &teams {
        let cycles = if team.cycles_enabled {
            ""
        } else {
            " (no cycles)"
        };
        out.push_str(&format!("- {} {}{cycles}\n", team.key, team.name));
    }
    let json: Vec<_> = teams.iter().map(team_json).collect();
    Ok(output(&args, out, json!(json)))
}

fn team_json(team: &Team) -> serde_json::Value {
    json!({ "id": team.id, "key": team.key, "name": team.name, "cycles": team.cycles_enabled })
}

/// `team show`: a team's workflow states, members, and labels — what its
/// issues can be moved to, assigned to, and labelled with.
///
/// Covers: team::ensure_context, team::context_arrived, team::load_labels
async fn show_team(linear: &impl Linear, args: &[String]) -> Result<String> {
    let args = Args::parse(args, &["json"], &[])?;
    let [key] = args.positionals::<1>("linear-tui team show <key> [--json]")?;
    let team = resolve::one_team(linear, key).await?;
    let mut store = Store::default();
    let Some(request) = usecase::team::ensure_context(&mut store, team.id.clone()) else {
        bail!("the team's states and members could not be asked for");
    };
    let Message::TeamContext {
        team_id,
        states,
        members,
    } = linear.run(request).await?
    else {
        bail!("unexpected answer to a team request");
    };
    usecase::team::context_arrived(&mut store, team_id, states, members);
    let context = store.team_contexts.remove(&team.id).unwrap_or_default();
    let Message::Labels { labels, .. } = linear
        .run(usecase::team::load_labels(team.id.clone()))
        .await?
    else {
        bail!("unexpected answer to a labels request");
    };
    let mut states = context.states.clone();
    states.sort_by(|a, b| {
        let rank = |s: &crate::core::entity::WorkflowState| s.state_type.map_or(9, |t| t.rank());
        rank(a).cmp(&rank(b)).then(
            a.position
                .unwrap_or(0.0)
                .total_cmp(&b.position.unwrap_or(0.0)),
        )
    });

    let mut out = format!(
        "# {} {}\n\n## States ({})\n\n",
        team.key,
        team.name,
        states.len()
    );
    for state in &states {
        let kind = state.state_type.map_or("", |t| t.as_str());
        out.push_str(&format!("- {} ({kind})\n", state.name));
    }
    out.push_str(&format!("\n## Members ({})\n\n", context.members.len()));
    for member in &context.members {
        let email = member
            .email
            .as_ref()
            .map_or(String::new(), |e| format!(" <{e}>"));
        out.push_str(&format!("- {}{email}\n", user_name(member)));
    }
    out.push_str(&format!("\n## Labels ({})\n\n", labels.len()));
    for label in &labels {
        out.push_str(&format!("- {}\n", label.name));
    }
    let json = json!({
        "team": team_json(&team),
        "states": states.iter().map(|s| json!({ "id": s.id, "name": s.name, "type": s.state_type.map(|t| t.as_str()) })).collect::<Vec<_>>(),
        "members": context.members.iter().map(|u| json!({ "id": u.id, "name": user_name(u), "email": u.email })).collect::<Vec<_>>(),
        "labels": labels.iter().map(|l| json!({ "id": l.id, "name": l.name })).collect::<Vec<_>>(),
    });
    Ok(output(&args, out, json))
}

/// `cycle list`: a team's cycles, the one under way marked.
///
/// Covers: cycle::open_team_cycles, cycle::next_page, cycle::reload,
/// cycle::take_page
pub async fn cycles(args: &[String], linear: &impl Linear, now: u64) -> Result<String> {
    let usage = "linear-tui cycle list --team <key> [--json]";
    let args = match args.split_first() {
        Some((command, rest)) if command == "list" => Args::parse(rest, &["json"], &["team"])?,
        _ => bail!("Usage: {usage}"),
    };
    args.positionals::<0>(usage)?;
    let Some(team) = args.value("team") else {
        bail!("Usage: {usage}");
    };
    let team = resolve::one_team(linear, team).await?;
    let mut store = Store::default();
    let open = usecase::cycle::open_team_cycles(&mut store, team.id.clone());
    let mut request = open
        .request()
        .or_else(|| usecase::cycle::reload(&mut store));
    while let Some(asked) = request.take() {
        let Message::Cycles { team_id, page } = linear.run(asked).await? else {
            bail!("unexpected answer to a cycles request");
        };
        usecase::cycle::take_page(&mut store, &ListOf::TeamCycles(team_id), page);
        request = usecase::cycle::next_page(&mut store);
    }
    let cycles = &store.cycles.items;
    let current = resolve::current_cycle(cycles, now).map(|c| c.id.clone());
    let mut out = format!("# {} cycles ({})\n\n", team.name, cycles.len());
    if cycles.is_empty() {
        out.push_str("(none)\n");
    }
    let day = |d: &Option<String>| {
        d.as_deref()
            .and_then(|d| d.get(..10))
            .unwrap_or("?")
            .to_string()
    };
    for cycle in cycles {
        let mark = if current.as_ref() == Some(&cycle.id) {
            " ← current"
        } else {
            ""
        };
        out.push_str(&format!(
            "- {} — {} to {}{}{mark}\n",
            cycle.label(),
            day(&cycle.starts_at),
            day(&cycle.ends_at),
            progress(cycle)
        ));
    }
    let json = json!({
        "team": team.key,
        "cycles": cycles.iter().map(|c| json!({
            "id": c.id, "name": c.label(), "number": c.number,
            "starts_at": c.starts_at, "ends_at": c.ends_at, "progress": c.progress,
            "current": current.as_ref() == Some(&c.id),
        })).collect::<Vec<_>>(),
    });
    Ok(output(&args, out, json))
}

fn progress(cycle: &Cycle) -> String {
    cycle
        .progress
        .map_or(String::new(), |p| format!(" · {:.0}% done", p * 100.0))
}

/// `view list`: the saved views, as Linear's Views page lists them — the
/// workspace's, or a team's with `--team`.
///
/// Covers: view::ensure_views, view::reload_views, view::take_views,
/// view::listed
pub async fn views(args: &[String], linear: &impl Linear) -> Result<String> {
    let usage = "linear-tui view list [--team <key>] [--json]";
    let args = match args.split_first() {
        Some((command, rest)) if command == "list" => Args::parse(rest, &["json"], &["team"])?,
        _ => bail!("Usage: {usage}"),
    };
    args.positionals::<0>(usage)?;
    let team = match args.value("team") {
        Some(key) => Some(resolve::one_team(linear, key).await?),
        None => None,
    };
    let mut store = Store::default();
    if let Some(request) = usecase::view::ensure_views(&store) {
        let Message::CustomViews(views) = linear.run(request).await? else {
            bail!("unexpected answer to a views request");
        };
        usecase::view::take_views(&mut store, views);
    }
    let views = &store.custom_views;
    let team_id = team.as_ref().map(|t| &t.id);
    let heading = team
        .as_ref()
        .map_or("Views".to_string(), |t| format!("{} views", t.name));
    let mut out = format!("# {heading}\n");
    let mut json = Vec::new();
    for (kind, issues) in [("Issue views", true), ("Project views", false)] {
        let listed = usecase::view::listed(views, team_id, issues);
        out.push_str(&format!("\n## {kind} ({})\n\n", listed.len()));
        if listed.is_empty() {
            out.push_str("(none)\n");
        }
        for view in listed.iter().filter_map(|&i| views.get(i)) {
            let shared = if view.shared { "shared" } else { "personal" };
            out.push_str(&format!("- {} ({shared})\n", view.name));
            json.push(json!({
                "id": view.id, "name": view.name, "lists": if issues { "issues" } else { "projects" },
                "shared": view.shared, "team": view.team.as_ref().map(|t| &t.key),
            }));
        }
    }
    Ok(output(&args, out, json!(json)))
}

/// `favorite list`: Linear's Favorites, in its order, each with where it
/// leads and the command that reads it.
///
/// Covers: favorite::load, favorite::take_favorites, favorite::target
pub async fn favorites(args: &[String], linear: &impl Linear) -> Result<String> {
    let usage = "linear-tui favorite list [--json]";
    let args = match args.split_first() {
        Some((command, rest)) if command == "list" => Args::parse(rest, &["json"], &[])?,
        _ => bail!("Usage: {usage}"),
    };
    args.positionals::<0>(usage)?;
    let mut store = Store::default();
    let Message::Favorites(favorites) = linear.run(usecase::favorite::load()).await? else {
        bail!("unexpected answer to a favorites request");
    };
    usecase::favorite::take_favorites(&mut store, favorites);
    let views = resolve::views(linear).await?;
    let teams = resolve::teams(linear).await?;
    let team_key = |id: &crate::core::entity::TeamId| {
        teams
            .iter()
            .find(|t| &t.id == id)
            .map_or_else(|| id.to_string(), |t| t.key.clone())
    };
    let mut out = format!("# Favorites ({})\n\n", store.favorites.len());
    let mut json = Vec::new();
    for favorite in &store.favorites {
        let target = usecase::favorite::target(favorite, &views, &teams);
        let (kind, read) = match &target {
            Target::Folder => ("folder", None),
            Target::View(id) => (
                "view",
                views
                    .iter()
                    .find(|v| &v.id == id)
                    .map(|v| format!("linear-tui issue list --view {:?}", v.name)),
            ),
            Target::MyIssues => ("my issues", Some("linear-tui issue list --mine".into())),
            Target::TeamPage(id, page) => (
                "team page",
                Some(match page {
                    TeamPage::Issues => format!("linear-tui issue list --team {}", team_key(id)),
                    TeamPage::Cycles => format!("linear-tui cycle list --team {}", team_key(id)),
                    TeamPage::Projects => {
                        format!("linear-tui project list --team {}", team_key(id))
                    }
                }),
            ),
            Target::Project(p) => (
                "project",
                Some(format!("linear-tui project show {:?}", p.name)),
            ),
            Target::Cycle(_) => ("cycle", None),
            Target::Issue(i) => (
                "issue",
                Some(format!("linear-tui issue show {}", i.identifier)),
            ),
            Target::Browser(_) => ("page on linear.app", None),
            Target::Nowhere => ("nothing", None),
        };
        let folder = if favorite.parent.is_some() { "  " } else { "" };
        let read_it = read.as_ref().map_or(String::new(), |r| format!(" — `{r}`"));
        out.push_str(&format!(
            "{folder}- {} ({kind}){read_it}\n",
            favorite.label()
        ));
        json.push(json!({
            "id": favorite.id, "title": favorite.label(), "kind": kind,
            "url": favorite.url, "in_folder": favorite.parent.is_some(), "command": read,
        }));
    }
    if store.favorites.is_empty() {
        out.push_str("(none)\n");
    }
    Ok(output(&args, out, json!(json)))
}
