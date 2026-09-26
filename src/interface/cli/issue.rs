//! `linear-tui issue show / create / update / comment / status`.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::args::{Args, text_or_stdin};
use super::resolve;
use super::{Host, Linear};
use crate::core::entity::{Comment, Issue, Priority, Team, User, WorkflowState};
use crate::core::entity::{IssueId, UserId};
use crate::core::message::Message;
use crate::core::store::Store;
use crate::core::usecase;
use crate::core::usecase::issue::Edit;

pub async fn run(args: &[String], host: &impl Host) -> Result<()> {
    let Some((command, rest)) = args.split_first() else {
        bail!("Usage: linear-tui issue <show|create|update|comment|status> …");
    };
    let linear = host.linear().await?;
    let out = match command.as_str() {
        "show" => show(&linear, rest).await?,
        "create" => create(&linear, rest, host.now()).await?,
        "update" => update(&linear, rest, host.now()).await?,
        "comment" => comment(&linear, rest).await?,
        "status" => status(&linear, rest).await?,
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

async fn show(linear: &impl Linear, args: &[String]) -> Result<String> {
    let args = Args::parse(args, &["json", "md"], &[])?;
    let [key] = args.positionals::<1>("linear-tui issue show <ID> [--json]")?;
    let issue = fetch(linear, key).await?;
    Ok(output(&args, render_issue(&issue), issue_json(&issue)))
}

/// The options `create` and `update` share, beyond the title.
const FIELDS: &[&str] = &[
    "description",
    "priority",
    "assignee",
    "estimate",
    "label",
    "project",
    "cycle",
    "parent",
];

async fn create(linear: &impl Linear, args: &[String], now: u64) -> Result<String> {
    let mut valued = vec!["team", "title"];
    valued.extend_from_slice(FIELDS);
    let args = Args::parse(args, &["json"], &valued)?;
    let usage = "linear-tui issue create --team <key> --title <text> [--description <text>|-] \
                 [--priority <level>] [--assignee <me|name|email>] [--estimate <n>] \
                 [--label <name>]... [--project <name>] [--cycle <name|current>] [--parent <ID>]";
    args.positionals::<0>(usage)?;
    let (Some(team), Some(title)) = (args.value("team"), args.value("title")) else {
        bail!("Usage: {usage}");
    };
    let Message::Teams(teams) = linear.run(usecase::team::load()).await? else {
        bail!("unexpected answer to a teams request");
    };
    let team = find_team(&teams, team)?;
    let mut fields = resolve::Team::new(linear, team.id.clone());
    let draft = usecase::issue::Draft {
        title: text_or_stdin(title)?,
        description: args.value("description").map(text_or_stdin).transpose()?,
        priority: args
            .value("priority")
            .map(resolve::priority)
            .transpose()?
            .unwrap_or_default(),
        assignee_id: match args.value("assignee") {
            Some(who) => fields.assignee(who).await?.map(|u| u.id),
            None => None,
        },
        estimate: args
            .value("estimate")
            .map(resolve::estimate)
            .transpose()?
            .flatten(),
        label_ids: fields
            .labels(&args.all("label"))
            .await?
            .into_iter()
            .map(|l| l.id)
            .collect(),
        project_id: match args.value("project") {
            Some(name) => fields.project(name).await?.map(|p| p.id),
            None => None,
        },
        cycle_id: match args.value("cycle") {
            Some(name) => fields.cycle(name, now).await?.map(|c| c.id),
            None => None,
        },
        parent_id: match args.value("parent") {
            Some(key) => resolve::parent(linear, key).await?.map(|p| p.id),
            None => None,
        },
    };
    let request = usecase::issue::create(team.id.clone(), draft)?;
    let Message::IssueCreated { issue, .. } = linear.run(request).await? else {
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

const UPDATE_USAGE: &str = "linear-tui issue update <ID> [--title <text>] [--description <text>|-] \
    [--priority <urgent|high|medium|low|none>] [--assignee <me|name|email|none>] \
    [--estimate <n|none>] [--label <name>]... [--unlabel <name>]... [--project <name|none>] \
    [--cycle <name|current|none>] [--parent <ID|none>] [--json]";

async fn update(linear: &impl Linear, args: &[String], now: u64) -> Result<String> {
    let mut valued = vec!["title", "unlabel"];
    valued.extend_from_slice(FIELDS);
    let args = Args::parse(args, &["json"], &valued)?;
    let [key] = args.positionals::<1>(UPDATE_USAGE)?;
    if !args.any(&valued) {
        bail!("Nothing to change. Usage: {UPDATE_USAGE}");
    }
    let issue = fetch(linear, key).await?;
    // Names resolve in the issue's own team, whichever team it is.
    let team_id = issue
        .team
        .as_ref()
        .map(|t| t.id.clone())
        .with_context(|| format!("Linear did not say which team {} is in", issue.identifier))?;
    let mut fields = resolve::Team::new(linear, team_id);
    let edit = Edit {
        title: args.value("title").map(text_or_stdin).transpose()?,
        description: args.value("description").map(text_or_stdin).transpose()?,
        priority: args.value("priority").map(resolve::priority).transpose()?,
        assignee: match args.value("assignee") {
            Some(who) => Some(fields.assignee(who).await?),
            None => None,
        },
        estimate: args.value("estimate").map(resolve::estimate).transpose()?,
        add_labels: fields.labels(&args.all("label")).await?,
        remove_labels: fields.labels(&args.all("unlabel")).await?,
        project: match args.value("project") {
            Some(name) => Some(fields.project(name).await?),
            None => None,
        },
        cycle: match args.value("cycle") {
            Some(name) => Some(fields.cycle(name, now).await?),
            None => None,
        },
        parent: match args.value("parent") {
            Some(key) => Some(resolve::parent(linear, key).await?),
            None => None,
        },
    };
    let changes = describe_changes(&issue, &edit);
    let mut store = Store::default();
    let request = usecase::issue::update(&mut store, &issue.id, edit)?;
    linear.run(request).await?;
    let markdown = changes
        .iter()
        .map(|c| format!("{}: {}", issue.identifier, c.line()))
        .collect::<Vec<_>>()
        .join("\n");
    let json = json!({
        "issue": issue.identifier,
        "changes": changes.iter().map(Change::json).collect::<Vec<_>>(),
    });
    Ok(output(&args, markdown, json))
}

/// One field an update changes, from what to what.
struct Change {
    field: &'static str,
    from: Value,
    to: Value,
}

impl Change {
    /// `priority High → Low`; a description is too long to repeat.
    fn line(&self) -> String {
        let text = |v: &Value| match v {
            Value::Null => "—".to_string(),
            Value::String(s) => s.clone(),
            Value::Array(items) if items.is_empty() => "—".to_string(),
            Value::Array(items) => items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", "),
            other => other.to_string(),
        };
        if self.field == "description" {
            return "description rewritten".to_string();
        }
        format!("{} {} → {}", self.field, text(&self.from), text(&self.to))
    }

    fn json(&self) -> Value {
        json!({ "field": self.field, "from": self.from, "to": self.to })
    }
}

/// What `edit` changes on `issue`, field by field, in the order
/// `issue show` lists them.
fn describe_changes(issue: &Issue, edit: &Edit) -> Vec<Change> {
    let mut out = Vec::new();
    let mut push = |field, from: Value, to: Value| out.push(Change { field, from, to });
    let person = |u: &Option<User>| json!(u.as_ref().map(|u| user_name(u).to_string()));
    if let Some(title) = &edit.title {
        push("title", json!(issue.title), json!(title));
    }
    if let Some(priority) = edit.priority {
        push(
            "priority",
            json!(issue.priority.label()),
            json!(priority.label()),
        );
    }
    if let Some(assignee) = &edit.assignee {
        push("assignee", person(&issue.assignee), person(assignee));
    }
    if !edit.add_labels.is_empty() || !edit.remove_labels.is_empty() {
        let before: Vec<String> = issue.labels.as_ref().map_or(Vec::new(), |l| {
            l.nodes.iter().map(|l| l.name.clone()).collect()
        });
        let mut after: Vec<String> = before
            .iter()
            .filter(|name| !edit.remove_labels.iter().any(|r| &&r.name == name))
            .cloned()
            .collect();
        for label in &edit.add_labels {
            if !after.contains(&label.name) {
                after.push(label.name.clone());
            }
        }
        push("labels", json!(before), json!(after));
    }
    if let Some(project) = &edit.project {
        push(
            "project",
            json!(issue.project.as_ref().map(|p| &p.name)),
            json!(project.as_ref().map(|p| &p.name)),
        );
    }
    if let Some(cycle) = &edit.cycle {
        push(
            "cycle",
            json!(issue.cycle.as_ref().map(|c| c.label())),
            json!(cycle.as_ref().map(|c| c.label())),
        );
    }
    if let Some(estimate) = edit.estimate {
        push("estimate", json!(issue.estimate), json!(estimate));
    }
    if let Some(parent) = &edit.parent {
        push(
            "parent",
            json!(issue.parent.as_ref().map(|p| &p.identifier)),
            json!(parent.as_ref().map(|p| &p.identifier)),
        );
    }
    if let Some(description) = &edit.description {
        push("description", json!(issue.description), json!(description));
    }
    out
}

const COMMENT_USAGE: &str = "linear-tui issue comment <ID> <body|-> [--reply-to <comment-id>] \
    | comment edit <ID> <comment-id> <body|-> | comment delete <ID> <comment-id>";

async fn comment(linear: &impl Linear, args: &[String]) -> Result<String> {
    match args.first().map(String::as_str) {
        Some("edit") => return edit_comment(linear, &args[1..]).await,
        Some("delete") => return delete_comment(linear, &args[1..]).await,
        _ => {}
    }
    let args = Args::parse(args, &["json"], &["reply-to"])?;
    let [key, body] = args.positionals::<2>(COMMENT_USAGE)?;
    let body = text_or_stdin(body)?;
    if body.trim().is_empty() {
        bail!("The comment is empty");
    }
    // Resolved first, so a mistyped ID says so rather than failing the post.
    let issue = fetch(linear, key).await?;
    let mut store = thread_store(&issue, None);
    let request = match args.value("reply-to") {
        Some(parent) => usecase::issue::reply(&mut store, &issue.id, &parent.into(), body)?,
        None => {
            usecase::issue::comment(&mut store, &issue.id, body).context("The comment is empty")?
        }
    };
    let Message::CommentPosted { comment, .. } = linear.run(request).await? else {
        bail!("unexpected answer to a comment");
    };
    let markdown = match &comment.parent {
        Some(parent) => format!(
            "Replied on {} (comment {}, in the thread of {})",
            issue.identifier, comment.id, parent.id
        ),
        None => format!("Commented on {} (comment {})", issue.identifier, comment.id),
    };
    Ok(output(
        &args,
        markdown,
        json!({
            "issue": issue.identifier,
            "commented": true,
            "id": comment.id,
            "url": comment.url,
            "parent_id": comment.parent.as_ref().map(|p| &p.id),
        }),
    ))
}

async fn edit_comment(linear: &impl Linear, args: &[String]) -> Result<String> {
    let args = Args::parse(args, &["json"], &[])?;
    let [key, id, body] = args.positionals::<3>(COMMENT_USAGE)?;
    let body = text_or_stdin(body)?;
    let issue = fetch(linear, key).await?;
    let mut store = thread_store(&issue, Some(viewer(linear).await?));
    let request = usecase::issue::edit_comment(&mut store, &issue.id, &id.into(), body)?;
    linear.run(request).await?;
    Ok(output(
        &args,
        format!("Edited comment {id} on {}", issue.identifier),
        json!({ "issue": issue.identifier, "comment": id, "edited": true }),
    ))
}

async fn delete_comment(linear: &impl Linear, args: &[String]) -> Result<String> {
    let args = Args::parse(args, &["json"], &[])?;
    let [key, id] = args.positionals::<2>(COMMENT_USAGE)?;
    let issue = fetch(linear, key).await?;
    let mut store = thread_store(&issue, Some(viewer(linear).await?));
    let request = usecase::issue::delete_comment(&mut store, &issue.id, &id.into())?;
    linear.run(request).await?;
    Ok(output(
        &args,
        format!("Deleted comment {id} on {}", issue.identifier),
        json!({ "issue": issue.identifier, "comment": id, "deleted": true }),
    ))
}

/// How many comments `issue show` reads; a thread that long may go on.
const THREAD_PAGE: usize = 100;

/// A store holding `issue` open with its thread, so the comment rules can be
/// checked against it — unless the thread may go on past what was read, when
/// Linear alone can say whether a comment is in it.
fn thread_store(issue: &Issue, viewer_id: Option<UserId>) -> Store {
    let mut open = issue.clone();
    if open
        .comments
        .as_ref()
        .is_some_and(|c| c.nodes.len() >= THREAD_PAGE)
    {
        open.comments = None;
    }
    Store {
        current_issue: Some(open),
        viewer_id,
        ..Store::default()
    }
}

/// Who the signed-in user is.
async fn viewer(linear: &impl Linear) -> Result<UserId> {
    let Message::Viewer { id, .. } = linear.run(usecase::user::load()).await? else {
        bail!("unexpected answer to a viewer request");
    };
    Ok(id)
}

async fn status(linear: &impl Linear, args: &[String]) -> Result<String> {
    let args = Args::parse(args, &["json"], &[])?;
    let [key, name] = args.positionals::<2>("linear-tui issue status <ID> <state>")?;
    let issue = fetch(linear, key).await?;
    // The state comes from the issue's own team, whichever team it is.
    let team_id = issue
        .team
        .as_ref()
        .map(|t| t.id.clone())
        .with_context(|| format!("Linear did not say which team {} is in", issue.identifier))?;
    let Message::TeamContext { states, .. } = linear
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
    let mut store = Store::default();
    let request = usecase::issue::set_status(&mut store, &issue.id, state.clone());
    linear.run(request).await?;
    Ok(output(
        &args,
        format!("{}: {before} → {}", issue.identifier, state.name),
        json!({ "issue": issue.identifier, "from": before, "to": state.name }),
    ))
}

/// An issue by identifier or URL, with its comments.
pub(super) async fn fetch(linear: &impl Linear, key: &str) -> Result<Issue> {
    let issue_id = IssueId::new(issue_key(key)?);
    match linear
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
                "\n{marker}{author} — {at}\n\n- Id: {}\n\n{}\n",
                comment.id,
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
    use std::cell::RefCell;

    use serde_json::json;

    use super::*;
    use crate::core::usecase::Request;

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

    /// Linear as a test answers it: the issue WEB-7, in team `web`, and
    /// that team's states; every request it was sent is kept.
    #[derive(Default)]
    struct FakeLinear {
        sent: RefCell<Vec<Request>>,
    }

    impl Linear for FakeLinear {
        async fn execute(&self, request: Request) -> Message {
            self.sent.borrow_mut().push(request.clone());
            match request {
                Request::Issue(usecase::issue::Request::Detail { .. }) => {
                    Message::IssueDetail(Box::new(
                        serde_json::from_value(json!({
                            "id": "i1", "identifier": "WEB-7", "title": "t", "priority": 0,
                            "team": { "id": "web" },
                            "state": { "id": "web-todo", "name": "Todo", "type": "unstarted" },
                            "priority": 2,
                            "labels": { "nodes": [{ "id": "l-ui", "name": "UI" }] },
                            "comments": { "nodes": [
                                { "id": "c1", "body": "mine", "user": { "id": "me", "name": "Me" } },
                                { "id": "c2", "body": "a reply", "parent": { "id": "c1" },
                                  "user": { "id": "me", "name": "Me" } },
                                { "id": "c3", "body": "hers", "user": { "id": "u2", "name": "Ada", "displayName": "ada" } }
                            ] }
                        }))
                        .unwrap(),
                    ))
                }
                Request::Team(usecase::team::Request::Context { team_id }) if team_id == "web" => {
                    Message::TeamContext {
                        team_id,
                        states: serde_json::from_value(json!([
                            { "id": "web-todo", "name": "Todo", "type": "unstarted" },
                            { "id": "web-done", "name": "Done", "type": "completed" }
                        ]))
                        .unwrap(),
                        members: serde_json::from_value(json!([
                            { "id": "me", "name": "Me", "email": "me@example.com" },
                            { "id": "u2", "name": "Ada Lovelace", "displayName": "ada", "email": "ada@example.com" }
                        ]))
                        .unwrap(),
                    }
                }
                Request::Team(usecase::team::Request::Labels { team_id }) if team_id == "web" => {
                    Message::Labels {
                        team_id,
                        labels: serde_json::from_value(json!([
                            { "id": "l-bug", "name": "Bug" },
                            { "id": "l-ui", "name": "UI" }
                        ]))
                        .unwrap(),
                    }
                }
                Request::User(usecase::user::Request::Viewer) => Message::Viewer {
                    id: "me".into(),
                    organization: None,
                },
                Request::Issue(usecase::issue::Request::Comment {
                    issue_id,
                    parent_id,
                    ..
                }) => Message::CommentPosted {
                    issue_id,
                    comment: Box::new(
                        serde_json::from_value(json!({
                            "id": "c9", "body": "b", "url": "https://linear.app/w/issue/WEB-7#comment-c9",
                            "parent": parent_id.map(|p| json!({ "id": p })),
                        }))
                        .unwrap(),
                    ),
                },
                Request::Issue(
                    usecase::issue::Request::SetStatus { .. }
                    | usecase::issue::Request::Update { .. }
                    | usecase::issue::Request::EditComment { .. }
                    | usecase::issue::Request::DeleteComment { .. },
                ) => Message::Mutated("done"),
                request => Message::Failed {
                    request: Box::new(request),
                    error: "not in this test".into(),
                },
            }
        }
    }

    /// The bug fixed in #31, from the command line: a state named like one of
    /// the current team's must still come from the issue's own team.
    #[tokio::test]
    async fn status_resolves_the_state_against_the_issues_own_team() {
        let linear = FakeLinear::default();
        let out = status(&linear, &["web-7".into(), "done".into()])
            .await
            .unwrap();
        assert_eq!(out, "WEB-7: Todo → Done");
        assert!(linear.sent.borrow().contains(&Request::Issue(
            usecase::issue::Request::SetStatus {
                issue_id: IssueId::new("i1"),
                state_id: "web-done".into(),
            }
        )));
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// The last issue request sent.
    fn last(linear: &FakeLinear) -> usecase::issue::Request {
        linear
            .sent
            .borrow()
            .iter()
            .rev()
            .find_map(|r| match r {
                Request::Issue(r) => Some(r.clone()),
                _ => None,
            })
            .unwrap()
    }

    /// An update resolves names in the issue's own team and prints each
    /// field it changed, from what to what.
    #[tokio::test]
    async fn an_update_resolves_names_and_says_what_changed() {
        let linear = FakeLinear::default();
        let out = update(
            &linear,
            &args(&[
                "web-7",
                "--priority",
                "low",
                "--assignee",
                "ada@example.com",
                "--label",
                "bug",
                "--unlabel",
                "ui",
            ]),
            0,
        )
        .await
        .unwrap();
        assert_eq!(
            out,
            "WEB-7: priority High → Low\nWEB-7: assignee — → ada\nWEB-7: labels UI → Bug"
        );
        let usecase::issue::Request::Update { changes, .. } = last(&linear) else {
            panic!("expected an update");
        };
        assert_eq!(changes.priority, Some(Priority::Low));
        assert_eq!(changes.assignee_id, Some(Some("u2".into())));
        assert_eq!(changes.added_label_ids, ["l-bug"]);
        assert_eq!(changes.removed_label_ids, ["l-ui"]);
    }

    /// With no field to change, the usage is the answer, and nothing is
    /// asked of Linear.
    #[tokio::test]
    async fn an_update_without_a_field_shows_the_usage() {
        let linear = FakeLinear::default();
        let err = update(&linear, &args(&["web-7"]), 0).await.unwrap_err();
        assert!(
            err.to_string().starts_with("Nothing to change. Usage:"),
            "{err}"
        );
        assert!(linear.sent.borrow().is_empty());
    }

    /// An unknown value lists what is valid.
    #[tokio::test]
    async fn an_unknown_value_lists_the_choices() {
        let linear = FakeLinear::default();
        let err = update(&linear, &args(&["web-7", "--assignee", "bob"]), 0)
            .await
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "no member bob (choices: Me <me@example.com>, ada <ada@example.com>)"
        );
        let err = update(&linear, &args(&["web-7", "--priority", "p1"]), 0)
            .await
            .unwrap_err();
        assert!(err.to_string().starts_with("no priority p1"), "{err}");
    }

    /// A comment prints its new id; a reply to a reply goes to the thread's
    /// first comment.
    #[tokio::test]
    async fn a_comment_prints_its_id_and_a_reply_finds_its_thread() {
        let linear = FakeLinear::default();
        let out = comment(&linear, &args(&["web-7", "hello"])).await.unwrap();
        assert_eq!(out, "Commented on WEB-7 (comment c9)");
        let out = comment(&linear, &args(&["web-7", "yes", "--reply-to", "c2"]))
            .await
            .unwrap();
        assert_eq!(out, "Replied on WEB-7 (comment c9, in the thread of c1)");
        let json = comment(&linear, &args(&["web-7", "x", "--json"]))
            .await
            .unwrap();
        let json: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(json["id"], "c9");
        assert!(json["url"].as_str().unwrap().ends_with("#comment-c9"));
    }

    /// My comment is edited and deleted; someone else's is refused with
    /// their name, before anything is sent.
    #[tokio::test]
    async fn only_my_comments_are_edited_or_deleted() {
        let linear = FakeLinear::default();
        let out = comment(&linear, &args(&["edit", "web-7", "c1", "better"]))
            .await
            .unwrap();
        assert_eq!(out, "Edited comment c1 on WEB-7");
        let out = comment(&linear, &args(&["delete", "web-7", "c2"]))
            .await
            .unwrap();
        assert_eq!(out, "Deleted comment c2 on WEB-7");
        let sent = linear.sent.borrow().len();
        let err = comment(&linear, &args(&["delete", "web-7", "c3"]))
            .await
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Only ada, who wrote it, can edit or delete this comment"
        );
        let err = comment(&linear, &args(&["edit", "web-7", "c7", "x"]))
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), "No comment c7 on this issue");
        let after: Vec<_> = linear.sent.borrow()[sent..].to_vec();
        assert!(
            after.iter().all(|r| !matches!(
                r,
                Request::Issue(
                    usecase::issue::Request::EditComment { .. }
                        | usecase::issue::Request::DeleteComment { .. }
                )
            )),
            "{after:?}"
        );
    }

    /// Each comment in the Markdown carries its id, to address an edit at.
    #[test]
    fn each_comment_carries_its_id() {
        let text = render_issue(&fixture_issue());
        assert!(text.contains("- Id: comment-"), "{text}");
    }

    /// A request Linear fails comes back as an error that says what failed.
    #[tokio::test]
    async fn a_failed_request_says_what_failed() {
        let error = FakeLinear::default()
            .run(usecase::team::Request::Teams)
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "Failed to load teams: not in this test");
    }
}
