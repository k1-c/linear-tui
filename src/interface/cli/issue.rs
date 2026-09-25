//! `linear-tui issue show / create / comment / status`.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::args::{Args, text_or_stdin};
use super::headless::{self, Session};
use crate::core::entity::IssueId;
use crate::core::entity::{Comment, Issue, Priority, Team, User, WorkflowState};
use crate::core::message::Message;
use crate::core::usecase;

pub async fn run(args: &[String]) -> Result<()> {
    let Some((command, rest)) = args.split_first() else {
        bail!("Usage: linear-tui issue <show|create|comment|status> …");
    };
    let session = headless::client().await?;
    let out = match command.as_str() {
        "show" => show(&session, rest).await?,
        "create" => create(&session, rest).await?,
        "comment" => comment(&session, rest).await?,
        "status" => status(&session, rest).await?,
        other => bail!("unknown command: issue {other}"),
    };
    println!("{}", out.trim_end());
    Ok(())
}

/// What was asked for, as Markdown or as JSON.
fn output(args: &Args, markdown: String, value: Value) -> String {
    if args.flag("json") {
        serde_json::to_string_pretty(&value).unwrap_or_default()
    } else {
        markdown
    }
}

async fn show(session: &Session, args: &[String]) -> Result<String> {
    let args = Args::parse(args, &["json", "md"], &[])?;
    let [key] = args.positionals::<1>("linear-tui issue show <ID> [--json]")?;
    let issue = fetch(session, key).await?;
    Ok(output(&args, render_issue(&issue), issue_json(&issue)))
}

async fn create(session: &Session, args: &[String]) -> Result<String> {
    let args = Args::parse(
        args,
        &["json"],
        &["team", "title", "description", "priority"],
    )?;
    let usage = "linear-tui issue create --team <key> --title <text> [--description <text>] [--priority <level>]";
    args.positionals::<0>(usage)?;
    let (Some(team), Some(title)) = (args.value("team"), args.value("title")) else {
        bail!("Usage: {usage}");
    };
    let priority = match args.value("priority") {
        Some(level) => parse_priority(level)?,
        None => Priority::None,
    };
    let Message::Teams(teams) = session.run(usecase::team::load()).await? else {
        bail!("unexpected answer to a teams request");
    };
    let team = find_team(&teams, team)?;
    let draft = usecase::issue::Draft {
        title: text_or_stdin(title)?,
        description: args
            .value("description")
            .map(text_or_stdin)
            .transpose()?
            .unwrap_or_default(),
        priority,
    };
    let request = usecase::issue::create(team.id.clone(), draft)?;
    let Message::IssueCreated { issue, .. } = session.run(request).await? else {
        bail!("unexpected answer to an issue create");
    };
    let markdown = format!(
        "Created {} {}\n{}",
        issue.identifier,
        issue.title,
        issue.url.as_deref().unwrap_or_default()
    );
    Ok(output(&args, markdown, issue_json(&issue)))
}

async fn comment(session: &Session, args: &[String]) -> Result<String> {
    let args = Args::parse(args, &["json"], &[])?;
    let [key, body] = args.positionals::<2>("linear-tui issue comment <ID> <body|->")?;
    let body = text_or_stdin(body)?;
    if body.trim().is_empty() {
        bail!("The comment is empty");
    }
    // Resolved first, so a mistyped ID says so rather than failing the post.
    let issue = fetch(session, key).await?;
    let mut store = crate::core::store::Store::default();
    if let Some(request) = usecase::issue::comment(&mut store, &issue.id, body) {
        session.run(request).await?;
    }
    Ok(output(
        &args,
        format!("Commented on {}", issue.identifier),
        json!({ "issue": issue.identifier, "commented": true }),
    ))
}

async fn status(session: &Session, args: &[String]) -> Result<String> {
    let args = Args::parse(args, &["json"], &[])?;
    let [key, name] = args.positionals::<2>("linear-tui issue status <ID> <state>")?;
    let issue = fetch(session, key).await?;
    // The state comes from the issue's own team, whichever team it is.
    let team_id = issue
        .team
        .as_ref()
        .map(|t| t.id.clone())
        .with_context(|| format!("Linear did not say which team {} is in", issue.identifier))?;
    let Message::TeamContext { states, .. } = session
        .run(usecase::team::Request::Context { team_id })
        .await?
    else {
        bail!("unexpected answer to a team request");
    };
    let state = resolve_state(&states, name)?.clone();
    let before = issue
        .state
        .as_ref()
        .map_or("—", |s| s.name.as_str())
        .to_string();
    let mut store = crate::core::store::Store::default();
    let request = usecase::issue::set_status(&mut store, &issue.id, state.clone());
    session.run(request).await?;
    Ok(output(
        &args,
        format!("{}: {before} → {}", issue.identifier, state.name),
        json!({ "issue": issue.identifier, "from": before, "to": state.name }),
    ))
}

/// An issue by identifier or URL, with its comments.
async fn fetch(session: &Session, key: &str) -> Result<Issue> {
    let issue_id = IssueId::new(issue_key(key)?);
    match session
        .run(usecase::issue::Request::Detail { issue_id })
        .await
    {
        Ok(Message::IssueDetail(issue)) => Ok(*issue),
        Ok(_) => bail!("unexpected answer to an issue request"),
        Err(e) => Err(e.context(format!("Could not load {key}"))),
    }
}

/// What Linear's `issue(id:)` takes: `ENG-123` from an identifier in any
/// case or from an issue URL, or a UUID as it is.
pub fn issue_key(arg: &str) -> Result<String> {
    let arg = arg.trim();
    if let Some(path) = arg
        .strip_prefix("https://linear.app/")
        .or_else(|| arg.strip_prefix("http://linear.app/"))
    {
        let mut parts = path.split('/');
        let (_, kind, identifier) = (parts.next(), parts.next(), parts.next());
        return match (kind, identifier) {
            (Some("issue"), Some(identifier)) if is_identifier(identifier) => {
                Ok(identifier.to_ascii_uppercase())
            }
            _ => bail!("not an issue URL: {arg}"),
        };
    }
    if is_identifier(arg) {
        return Ok(arg.to_ascii_uppercase());
    }
    if !arg.is_empty() && arg.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Ok(arg.to_string());
    }
    bail!("not an issue identifier: {arg} (expected something like ENG-123)")
}

/// `ENG-123`: a team key, a dash, a number.
pub fn is_identifier(text: &str) -> bool {
    let Some((team, number)) = text.split_once('-') else {
        return false;
    };
    !team.is_empty()
        && team.chars().all(|c| c.is_ascii_alphanumeric())
        && team.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
}

fn find_team<'a>(teams: &'a [Team], wanted: &str) -> Result<&'a Team> {
    teams
        .iter()
        .find(|t| t.key.eq_ignore_ascii_case(wanted) || t.name.eq_ignore_ascii_case(wanted))
        .with_context(|| {
            let keys: Vec<&str> = teams.iter().map(|t| t.key.as_str()).collect();
            format!("no team {wanted} (teams: {})", keys.join(", "))
        })
}

fn parse_priority(level: &str) -> Result<Priority> {
    Priority::ALL
        .into_iter()
        .find(|p| p.label().eq_ignore_ascii_case(level))
        .with_context(|| format!("no priority {level} (urgent, high, medium, low, none)"))
}

/// The state called `name` in the team's workflow, ignoring case.
pub fn resolve_state<'a>(states: &'a [WorkflowState], name: &str) -> Result<&'a WorkflowState> {
    states
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name.trim()))
        .with_context(|| {
            let names: Vec<&str> = states.iter().map(|s| s.name.as_str()).collect();
            format!(
                "no state {name} in this issue's team (states: {})",
                names.join(", ")
            )
        })
}

fn user_name(user: &User) -> &str {
    user.display_name.as_deref().unwrap_or(&user.name)
}

/// Comments in the order they were written, each reply right after the
/// comment it answers.
fn threaded(comments: &[Comment]) -> Vec<(&Comment, bool)> {
    let mut sorted: Vec<&Comment> = comments.iter().collect();
    sorted.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    let mut out = Vec::new();
    for root in sorted.iter().filter(|c| c.parent.is_none()) {
        out.push((*root, false));
        for reply in sorted
            .iter()
            .filter(|c| c.parent.as_ref().is_some_and(|p| p.id == root.id))
        {
            out.push((*reply, true));
        }
    }
    // A reply whose parent was not fetched is still shown.
    for orphan in sorted.iter().filter(|c| {
        c.parent
            .as_ref()
            .is_some_and(|p| !comments.iter().any(|root| root.id == p.id))
    }) {
        out.push((*orphan, true));
    }
    out
}

pub fn render_issue(issue: &Issue) -> String {
    let mut out = format!("# {} {}\n\n", issue.identifier, issue.title);
    let mut field = |name: &str, value: Option<String>| {
        if let Some(value) = value.filter(|v| !v.is_empty()) {
            out.push_str(&format!("- {name}: {value}\n"));
        }
    };
    field("State", issue.state.as_ref().map(|s| s.name.clone()));
    field(
        "Priority",
        (issue.priority != Priority::None).then(|| issue.priority.label().to_string()),
    );
    field(
        "Assignee",
        issue.assignee.as_ref().map(|u| user_name(u).to_string()),
    );
    field(
        "Creator",
        issue.creator.as_ref().map(|u| user_name(u).to_string()),
    );
    field(
        "Labels",
        issue.labels.as_ref().map(|l| {
            l.nodes
                .iter()
                .map(|l| l.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }),
    );
    field("Project", issue.project.as_ref().map(|p| p.name.clone()));
    field(
        "Milestone",
        issue.project_milestone.as_ref().map(|m| m.name.clone()),
    );
    field("Cycle", issue.cycle.as_ref().map(|c| c.label()));
    field("Estimate", issue.estimate.map(|e| e.to_string()));
    field("Due", issue.due_date.clone());
    field(
        "Parent",
        issue
            .parent
            .as_ref()
            .map(|p| format!("{} {}", p.identifier, p.title)),
    );
    field("Branch", issue.branch_name.clone());
    field("URL", issue.url.clone());

    let description = issue.description.as_deref().unwrap_or("").trim();
    out.push_str("\n## Description\n\n");
    out.push_str(if description.is_empty() {
        "(none)"
    } else {
        description
    });
    out.push('\n');

    if let Some(children) = issue.children.as_ref().filter(|c| !c.nodes.is_empty()) {
        out.push_str("\n## Sub-issues\n\n");
        for child in &children.nodes {
            let state = child.state.as_ref().map_or("", |s| s.name.as_str());
            out.push_str(&format!(
                "- {} {} ({state})\n",
                child.identifier, child.title
            ));
        }
    }

    if let Some(comments) = &issue.comments {
        out.push_str(&format!("\n## Comments ({})\n", comments.nodes.len()));
        for (comment, reply) in threaded(&comments.nodes) {
            let author = comment.user.as_ref().map_or("someone", user_name);
            let at = comment.created_at.as_deref().unwrap_or("");
            let marker = if reply { "#### ↳ " } else { "### " };
            out.push_str(&format!(
                "\n{marker}{author} — {at}\n\n{}\n",
                comment.body.trim()
            ));
        }
    }
    out
}

pub fn issue_json(issue: &Issue) -> Value {
    let user = |u: &Option<User>| u.as_ref().map(|u| user_name(u).to_string());
    json!({
        "id": issue.id,
        "identifier": issue.identifier,
        "title": issue.title,
        "url": issue.url,
        "state": issue.state.as_ref().map(|s| json!({
            "name": s.name,
            "type": s.state_type.map(|t| t.as_str()),
        })),
        "priority": issue.priority.label(),
        "assignee": user(&issue.assignee),
        "creator": user(&issue.creator),
        "team_id": issue.team.as_ref().map(|t| &t.id),
        "labels": issue.labels.as_ref().map_or(Vec::new(), |l| l.nodes.iter().map(|l| l.name.clone()).collect()),
        "project": issue.project.as_ref().map(|p| json!({ "id": p.id, "name": p.name })),
        "cycle": issue.cycle.as_ref().map(|c| json!({ "id": c.id, "name": c.label() })),
        "parent": issue.parent.as_ref().map(|p| json!({ "identifier": p.identifier, "title": p.title })),
        "children": issue.children.as_ref().map_or(Vec::new(), |c| c.nodes.iter().map(|c| json!({
            "identifier": c.identifier,
            "title": c.title,
            "state": c.state.as_ref().map(|s| &s.name),
        })).collect()),
        "estimate": issue.estimate,
        "due_date": issue.due_date,
        "branch_name": issue.branch_name,
        "created_at": issue.created_at,
        "updated_at": issue.updated_at,
        "description": issue.description,
        "comments": issue.comments.as_ref().map(|c| threaded(&c.nodes).into_iter().map(|(c, _)| json!({
            "id": c.id,
            "author": user(&c.user),
            "created_at": c.created_at,
            "parent_id": c.parent.as_ref().map(|p| &p.id),
            "body": c.body,
        })).collect::<Vec<_>>()),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use wiremock::matchers::{body_string_contains, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::infra::linear::client::{LinearClient, StaticCredentials};

    fn fixture_issue() -> Issue {
        #[derive(serde::Deserialize)]
        struct Resp {
            issue: Issue,
        }
        let text = std::fs::read_to_string("tests/fixtures/issue_detail_full.json").unwrap();
        serde_json::from_str::<Resp>(&text).unwrap().issue
    }

    #[test]
    fn an_issue_is_named_by_identifier_url_or_uuid() {
        assert_eq!(issue_key("eng-42").unwrap(), "ENG-42");
        assert_eq!(
            issue_key("https://linear.app/shop/issue/ENG-42/checkout-fails").unwrap(),
            "ENG-42"
        );
        assert_eq!(
            issue_key("9a0e1c2d-0000-4000-8000-00000000abcd").unwrap(),
            "9a0e1c2d-0000-4000-8000-00000000abcd"
        );
        assert!(issue_key("https://linear.app/shop/project/x").is_err());
        assert!(issue_key("fix the bug").is_err());
    }

    #[test]
    fn the_markdown_carries_the_fields_description_and_thread() {
        let text = render_issue(&fixture_issue());
        assert!(
            text.starts_with("# ENG-157 Merge the dashboard and the report\n"),
            "{text}"
        );
        assert!(text.contains("- State: In Progress\n"), "{text}");
        assert!(text.contains("- Priority: High\n"), "{text}");
        assert!(text.contains("## Description"), "{text}");
        assert!(text.contains("## Comments ("), "{text}");
    }

    #[test]
    fn the_json_names_the_state_type_and_keeps_every_comment() {
        let issue = fixture_issue();
        let value = issue_json(&issue);
        assert_eq!(value["identifier"], "ENG-157");
        assert_eq!(value["state"]["type"], "started");
        assert_eq!(
            value["comments"].as_array().unwrap().len(),
            issue.comments.as_ref().unwrap().nodes.len()
        );
    }

    #[test]
    fn a_state_is_found_by_name_in_any_case_or_the_choices_are_listed() {
        let states: Vec<WorkflowState> = serde_json::from_value(json!([
            { "id": "s1", "name": "Todo", "type": "unstarted" },
            { "id": "s2", "name": "In Review", "type": "started" },
        ]))
        .unwrap();
        assert_eq!(resolve_state(&states, "in review").unwrap().id, "s2");
        let err = resolve_state(&states, "Doing").unwrap_err().to_string();
        assert_eq!(
            err,
            "no state Doing in this issue's team (states: Todo, In Review)"
        );
    }

    /// The bug fixed in #31, from the command line: a state named like one of
    /// the current team's must still come from the issue's own team.
    #[tokio::test]
    async fn status_resolves_the_state_against_the_issues_own_team() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_string_contains("IssueDetail"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({ "data": { "issue": {
                "id": "i1", "identifier": "WEB-7", "title": "t", "priority": 0,
                "team": { "id": "web" },
                "state": { "id": "web-todo", "name": "Todo", "type": "unstarted" },
                "comments": { "nodes": [] }
            } } })),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(body_string_contains("WorkflowStates"))
            .and(body_string_contains("\"teamId\":\"web\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": {
                "workflowStates": { "nodes": [
                    { "id": "web-todo", "name": "Todo", "type": "unstarted" },
                    { "id": "web-done", "name": "Done", "type": "completed" }
                ] }
            } })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(body_string_contains("members"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({ "data": { "team": {
                "members": { "nodes": [] }
            } } })),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(body_string_contains("issueUpdate"))
            .and(body_string_contains("web-done"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": {
                "issueUpdate": { "success": true }
            } })))
            .expect(1)
            .mount(&server)
            .await;

        let client =
            LinearClient::with_endpoint(server.uri(), Arc::new(StaticCredentials("key".into())));
        let session = Session::new(client);
        let out = status(&session, &["web-7".into(), "done".into()])
            .await
            .unwrap();
        assert_eq!(out, "WEB-7: Todo → Done");
    }
}
