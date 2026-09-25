//! The test workspaces: checking that a key points at one, emptying it, and
//! filling it with the data the scenarios name.
//!
//! This talks to Linear directly, not through linear-tui, so a bug in
//! linear-tui cannot hide in the setup. It is destructive: it deletes every
//! issue, project, saved view, and favorite it can see — which is why it
//! refuses to run unless the workspace's URL key is the one the environment
//! names for it.

use std::collections::HashMap;

use serde_json::{Value, json};

const API: &str = "https://api.linear.app/graphql";

/// One test workspace, as the environment describes it.
#[derive(Debug, Clone)]
pub struct Account {
    /// `A` or `B`.
    pub name: &'static str,
    pub api_key: String,
    /// The workspace's URL key (`linear.app/<key>/…`). A key for any other
    /// workspace is refused.
    pub url_key: String,
}

impl Account {
    /// `LINEAR_E2E_API_KEY_<name>` and `LINEAR_E2E_WORKSPACE_<name>`, when
    /// both are set.
    pub fn from_env(name: &'static str) -> Option<Self> {
        let var = |v: &str| std::env::var(v).ok().filter(|s| !s.is_empty());
        Some(Self {
            name,
            api_key: var(&format!("LINEAR_E2E_API_KEY_{name}"))?,
            url_key: var(&format!("LINEAR_E2E_WORKSPACE_{name}"))?,
        })
    }
}

/// A small blocking GraphQL client for the setup.
pub struct Linear {
    http: reqwest::Client,
    api_key: String,
    runtime: tokio::runtime::Runtime,
}

impl Linear {
    pub fn new(account: &Account) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: account.api_key.clone(),
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("a tokio runtime for the setup"),
        }
    }

    /// Run a query; a GraphQL error is an `Err` naming the query.
    pub fn gql(&self, query: &str, variables: Value) -> Result<Value, String> {
        let request = self
            .http
            .post(API)
            .header("Authorization", &self.api_key)
            .json(&json!({ "query": query, "variables": variables }));
        let body: Value = self
            .runtime
            .block_on(async { request.send().await?.json().await })
            .map_err(|e| format!("{e}"))?;
        if let Some(errors) = body.get("errors") {
            let first_line = query.lines().next().unwrap_or_default();
            return Err(format!("{first_line}: {errors}"));
        }
        Ok(body["data"].clone())
    }

    /// Run a `<name>(input: …)` mutation and return what it created.
    fn create(
        &self,
        mutation: &str,
        input_type: &str,
        entity: &str,
        input: Value,
    ) -> Result<String, String> {
        let query = format!(
            "mutation($input: {input_type}!) {{ {mutation}(input: $input) {{ success {entity} {{ id }} }} }}"
        );
        let data = self.gql(&query, json!({ "input": input }))?;
        data[mutation][entity]["id"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| format!("{mutation} created nothing"))
    }

    /// Every node of a connection, following its pages.
    fn all(&self, field: &str, selection: &str) -> Result<Vec<Value>, String> {
        let mut nodes = Vec::new();
        let mut after = Value::Null;
        loop {
            let query = format!(
                "query($after: String) {{ {field}(first: 100, after: $after) {{ nodes {{ {selection} }} pageInfo {{ hasNextPage endCursor }} }} }}"
            );
            let data = self.gql(&query, json!({ "after": after }))?;
            nodes.extend(data[field]["nodes"].as_array().cloned().unwrap_or_default());
            if data[field]["pageInfo"]["hasNextPage"] != json!(true) {
                return Ok(nodes);
            }
            after = data[field]["pageInfo"]["endCursor"].clone();
        }
    }
}

/// What the scenarios find in workspace A once it is seeded.
#[derive(Debug, Clone)]
pub struct Seeded {
    pub team_key: String,
    pub team_name: String,
    /// Another team of the workspace, for switching to.
    pub other_team: Option<String>,
    pub viewer_name: String,
    /// Issue identifiers by title.
    pub issues: HashMap<String, String>,
}

impl Seeded {
    /// The identifier of the seeded issue titled `title`.
    pub fn issue(&self, title: &str) -> &str {
        self.issues
            .get(title)
            .unwrap_or_else(|| panic!("no seeded issue titled {title:?}"))
    }
}

/// Titles the scenarios name. Each changing scenario has an issue of its
/// own, so the scenarios can run in any order against one seeding.
pub mod titles {
    pub const CRASH: &str = "Crash when the forecast is empty";
    pub const RETRY: &str = "Retry failed forecast fetches";
    pub const CHART: &str = "Hourly chart overlaps on narrow screens";
    pub const RADAR: &str = "Add a radar layer toggle";
    pub const DOCS: &str = "Document the forecast API";
    pub const WIND: &str = "Wind arrows point the wrong way";
    pub const FOR_STATUS: &str = "E2E status change";
    pub const FOR_PRIORITY: &str = "E2E priority change";
    pub const FOR_ASSIGNEE: &str = "E2E assignee change";
    pub const FOR_COMMENT: &str = "E2E comment target";
    pub const FOR_AGENT: &str = "E2E agent target";
    pub const SEEDED_COMMENT: &str = "Reproduced on Android too.";
    pub const PROJECT: &str = "Forecast v2";
    pub const ISSUE_VIEW: &str = "Open bugs";
    pub const PROJECT_VIEW: &str = "E2E projects";
    pub const IN_WORKSPACE_B: &str = "Only in workspace B";
}

/// Refuse any workspace but the one the environment names.
pub fn check_workspace(linear: &Linear, account: &Account) -> Result<(), String> {
    let data = linear.gql("query { organization { urlKey name } }", json!({}))?;
    let key = data["organization"]["urlKey"].as_str().unwrap_or_default();
    if key != account.url_key {
        return Err(format!(
            "LINEAR_E2E_API_KEY_{} is for workspace {key:?}, not {:?} (LINEAR_E2E_WORKSPACE_{}); refusing to reset it",
            account.name, account.url_key, account.name
        ));
    }
    Ok(())
}

/// Delete every issue, project, saved view, and favorite.
pub fn reset(linear: &Linear) -> Result<(), String> {
    for favorite in linear.all("favorites", "id")? {
        linear.gql(
            "mutation($id: String!) { favoriteDelete(id: $id) { success } }",
            json!({ "id": favorite["id"] }),
        )?;
    }
    for view in linear.all("customViews", "id")? {
        linear.gql(
            "mutation($id: String!) { customViewDelete(id: $id) { success } }",
            json!({ "id": view["id"] }),
        )?;
    }
    // Sub-issues go with their parents; deleting one twice is harmless.
    for issue in linear.all("issues", "id")? {
        let _ = linear.gql(
            "mutation($id: String!) { issueDelete(id: $id) { success } }",
            json!({ "id": issue["id"] }),
        );
    }
    for project in linear.all("projects", "id")? {
        linear.gql(
            "mutation($id: String!) { projectDelete(id: $id) { success } }",
            json!({ "id": project["id"] }),
        )?;
    }
    Ok(())
}

/// The team the scenarios use: `LINEAR_E2E_TEAM_<name>` by key, or the
/// first team.
fn team(linear: &Linear, account: &Account) -> Result<(Value, Option<String>), String> {
    let teams = linear.gql(
        "query { teams { nodes { id key name cyclesEnabled cycles(first: 10) { nodes { id number } } } } }",
        json!({}),
    )?;
    let teams = teams["teams"]["nodes"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let wanted = std::env::var(format!("LINEAR_E2E_TEAM_{}", account.name)).ok();
    let team = teams
        .iter()
        .find(|t| wanted.as_deref().is_none_or(|k| t["key"] == k))
        .cloned()
        .ok_or_else(|| "no team to test in".to_string())?;
    let other = teams
        .iter()
        .find(|t| t["id"] != team["id"])
        .and_then(|t| t["name"].as_str())
        .map(String::from);
    Ok((team, other))
}

/// The id of each of the team's states, by name.
fn states(linear: &Linear, team_id: &str) -> Result<HashMap<String, String>, String> {
    let data = linear.gql(
        "query($id: ID!) { workflowStates(filter: { team: { id: { eq: $id } } }) { nodes { id name } } }",
        json!({ "id": team_id }),
    )?;
    Ok(data["workflowStates"]["nodes"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|s| {
            (
                s["name"].as_str().unwrap_or_default().to_string(),
                s["id"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect())
}

/// A label of the team, created when missing.
fn label(linear: &Linear, team_id: &str, name: &str, color: &str) -> Result<String, String> {
    let data = linear.gql(
        "query($id: String!) { team(id: $id) { labels(first: 100) { nodes { id name } } } }",
        json!({ "id": team_id }),
    )?;
    if let Some(found) = data["team"]["labels"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|l| l["name"] == name)
    {
        return Ok(found["id"].as_str().unwrap_or_default().to_string());
    }
    linear.create(
        "issueLabelCreate",
        "IssueLabelCreateInput",
        "issueLabel",
        json!({ "teamId": team_id, "name": name, "color": color }),
    )
}

/// Empty workspace A and fill it with what the scenarios name.
pub fn seed_a(linear: &Linear, account: &Account) -> Result<Seeded, String> {
    use titles::*;
    check_workspace(linear, account)?;
    reset(linear)?;
    let (team, other_team) = team(linear, account)?;
    let team_id = team["id"].as_str().unwrap_or_default().to_string();
    let viewer = linear.gql("query { viewer { id name } }", json!({}))?["viewer"].clone();
    let viewer_id = viewer["id"].as_str().unwrap_or_default().to_string();
    let states = states(linear, &team_id)?;
    let state = |name: &str| {
        states.get(name).cloned().ok_or_else(|| {
            format!(
                "team {} has no state {name:?}; the scenarios expect Linear's defaults",
                team["key"]
            )
        })
    };
    let bug = label(linear, &team_id, "Bug", "#eb5757")?;
    let feature = label(linear, &team_id, "Feature", "#bb87fc")?;
    let project = linear.create(
        "projectCreate",
        "ProjectCreateInput",
        "project",
        json!({ "name": PROJECT, "teamIds": [team_id], "description": "Hourly forecasts, rebuilt on the new API" }),
    )?;
    let cycle = team["cycles"]["nodes"][0]["id"]
        .as_str()
        .map(String::from)
        .ok_or(
            "the team has no cycle: turn cycles on for it in Linear (Team settings › Cycles)"
                .to_string(),
        )?;

    let mut issues = HashMap::new();
    let mut create = |title: &str, input: Value| -> Result<String, String> {
        let mut input = input;
        input["teamId"] = json!(team_id);
        input["title"] = json!(title);
        let query = "mutation($input: IssueCreateInput!) { issueCreate(input: $input) { success issue { id identifier } } }";
        let data = linear.gql(query, json!({ "input": input }))?;
        let issue = &data["issueCreate"]["issue"];
        issues.insert(
            title.to_string(),
            issue["identifier"].as_str().unwrap_or_default().to_string(),
        );
        Ok(issue["id"].as_str().unwrap_or_default().to_string())
    };
    // The oldest first, so the list (newest activity first) reads as below.
    create(
        DOCS,
        json!({ "stateId": state("Done")?, "priority": 4, "projectId": project }),
    )?;
    create(
        WIND,
        json!({ "stateId": state("Backlog")?, "priority": 2, "labelIds": [bug] }),
    )?;
    create(
        RADAR,
        json!({ "stateId": state("Backlog")?, "priority": 3, "labelIds": [feature] }),
    )?;
    create(FOR_AGENT, json!({ "stateId": state("Backlog")? }))?;
    create(FOR_ASSIGNEE, json!({ "stateId": state("Todo")? }))?;
    create(FOR_PRIORITY, json!({ "stateId": state("Todo")? }))?;
    create(FOR_STATUS, json!({ "stateId": state("Todo")? }))?;
    let commented = create(FOR_COMMENT, json!({ "stateId": state("Todo")? }))?;
    let chart = create(
        CHART,
        json!({ "stateId": state("Todo")?, "priority": 2, "labelIds": [bug], "cycleId": cycle,
                "description": "Below 360px the labels collide." }),
    )?;
    let crash = create(
        CRASH,
        json!({
            "stateId": state("In Progress")?, "priority": 1, "labelIds": [bug],
            "assigneeId": viewer_id, "projectId": project, "cycleId": cycle,
            "description": "The app **crashes** when the API returns no hours.\n\n## Steps\n\n1. Pick a city with no data\n2. Open the hourly chart",
        }),
    )?;
    create(
        RETRY,
        json!({ "stateId": state("In Progress")?, "priority": 2, "parentId": crash, "projectId": project }),
    )?;
    linear.create(
        "commentCreate",
        "CommentCreateInput",
        "comment",
        json!({ "issueId": commented, "body": SEEDED_COMMENT }),
    )?;

    let issue_view = linear.create(
        "customViewCreate",
        "CustomViewCreateInput",
        "customView",
        json!({ "name": ISSUE_VIEW, "shared": true, "filterData": { "labels": { "id": { "eq": bug } } } }),
    )?;
    linear.create(
        "customViewCreate",
        "CustomViewCreateInput",
        "customView",
        json!({ "name": PROJECT_VIEW, "shared": true,
                "projectFilterData": { "name": { "eq": PROJECT } } }),
    )?;
    for input in [
        json!({ "projectId": project, "sortOrder": 1 }),
        json!({ "customViewId": issue_view, "sortOrder": 2 }),
        json!({ "issueId": chart, "sortOrder": 3 }),
    ] {
        linear.create("favoriteCreate", "FavoriteCreateInput", "favorite", input)?;
    }

    Ok(Seeded {
        team_key: team["key"].as_str().unwrap_or_default().to_string(),
        team_name: team["name"].as_str().unwrap_or_default().to_string(),
        other_team,
        viewer_name: viewer["name"].as_str().unwrap_or_default().to_string(),
        issues,
    })
}

/// Empty workspace B and give it one issue workspace A does not have.
pub fn seed_b(linear: &Linear, account: &Account) -> Result<Seeded, String> {
    check_workspace(linear, account)?;
    reset(linear)?;
    let (team, other_team) = team(linear, account)?;
    let team_id = team["id"].as_str().unwrap_or_default();
    let data = linear.gql(
        "mutation($input: IssueCreateInput!) { issueCreate(input: $input) { issue { identifier } } }",
        json!({ "input": { "teamId": team_id, "title": titles::IN_WORKSPACE_B } }),
    )?;
    let viewer = linear.gql("query { viewer { name } }", json!({}))?;
    Ok(Seeded {
        team_key: team["key"].as_str().unwrap_or_default().to_string(),
        team_name: team["name"].as_str().unwrap_or_default().to_string(),
        other_team,
        viewer_name: viewer["viewer"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        issues: HashMap::from([(
            titles::IN_WORKSPACE_B.to_string(),
            data["issueCreate"]["issue"]["identifier"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        )]),
    })
}
