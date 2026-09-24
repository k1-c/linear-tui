use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::types::*;

const API_URL: &str = "https://api.linear.app/graphql";

/// Fields selected for every issue, list row or detail alike, so that an issue
/// coming from any query is equally usable everywhere in the UI.
const ISSUE_FIELDS: &str = r#"
    id
    identifier
    title
    priority
    priorityLabel
    estimate
    dueDate
    url
    branchName
    state { id name color type position }
    assignee { id name displayName }
    creator { id name displayName }
    labels { nodes { id name color } }
    project { id name color state }
    parent { id identifier title state { id name color type } }
    description
    createdAt
    updatedAt
"#;

/// Fields selected for every project row, whichever list it comes from.
const PROJECT_FIELDS: &str = r#"
    id
    name
    description
    state
    color
    health
    progress
    startDate
    targetDate
    url
    lead { id name displayName }
"#;

/// Default page size for the sub-lists that hang off a project or cycle.
const SUBLIST_PAGE_SIZE: u32 = 100;

pub struct LinearClient {
    http: reqwest::Client,
    token: String,
}

#[derive(Serialize)]
struct GraphQLRequest<'a> {
    query: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    variables: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct GraphQLResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Deserialize)]
struct GraphQLError {
    message: String,
}

impl LinearClient {
    pub fn new(token: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            token,
        }
    }

    async fn query<T: for<'de> Deserialize<'de>>(
        &self,
        query: &str,
        variables: Option<serde_json::Value>,
    ) -> Result<T> {
        let query_name = query
            .split_whitespace()
            .find(|s| !matches!(*s, "query" | "mutation"))
            .unwrap_or("unknown")
            .split(['(', '{', '$'])
            .next()
            .unwrap_or("unknown");

        tracing::debug!(query_name, "sending GraphQL request");
        if let Some(vars) = &variables {
            tracing::trace!(query_name, variables = %vars, "request variables");
        }

        let resp = self
            .http
            .post(API_URL)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json")
            .json(&GraphQLRequest { query, variables })
            .send()
            .await
            .context("GraphQL request failed")?;

        let status = resp.status();
        tracing::debug!(query_name, status = %status, "received response");

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            tracing::error!(query_name, status = %status, body = %body, "API error");
            anyhow::bail!("API error ({status}): {body}");
        }

        let text = resp.text().await.context("Failed to read response body")?;
        tracing::trace!(query_name, body_len = text.len(), "response body received");

        let gql_resp: GraphQLResponse<T> = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(
                    query_name,
                    error = %e,
                    response_body = %text,
                    "failed to parse GraphQL response"
                );
                anyhow::bail!("Failed to parse response: {e}");
            }
        };

        if let Some(errors) = gql_resp.errors {
            let msgs: Vec<_> = errors.iter().map(|e| e.message.as_str()).collect();
            tracing::error!(query_name, errors = %msgs.join(", "), "GraphQL errors");
            anyhow::bail!("GraphQL errors: {}", msgs.join(", "));
        }

        gql_resp.data.context("No data in response")
    }

    pub async fn teams(&self) -> Result<Vec<Team>> {
        #[derive(Deserialize)]
        struct Resp {
            teams: Connection<Team>,
        }
        let resp: Resp = self
            .query(
                "query { teams(first: 100) { nodes { id name key color cyclesEnabled } } }",
                None,
            )
            .await?;
        Ok(resp.teams.nodes)
    }

    /// A team's issues, newest activity first, optionally narrowed to some
    /// workflow categories (`state` is an `IssueFilter.state` clause).
    pub async fn issues(
        &self,
        team_id: &str,
        state: Option<serde_json::Value>,
        after: Option<&str>,
        first: u32,
    ) -> Result<(Vec<Issue>, PageInfo)> {
        #[derive(Deserialize)]
        struct Resp {
            issues: Connection<Issue>,
        }
        let mut filter = serde_json::json!({ "team": { "id": { "eq": team_id } } });
        if let Some(state) = state {
            filter["state"] = state;
        }
        let variables = serde_json::json!({
            "filter": filter,
            "after": after,
            "first": first,
        });
        let query = format!(
            r#"query($filter: IssueFilter, $after: String, $first: Int!) {{
                issues(
                    filter: $filter
                    first: $first
                    after: $after
                    orderBy: updatedAt
                ) {{
                    nodes {{ {ISSUE_FIELDS} }}
                    pageInfo {{ hasNextPage endCursor }}
                }}
            }}"#
        );
        let resp: Resp = self.query(&query, Some(variables)).await?;
        Ok((resp.issues.nodes, resp.issues.page_info))
    }

    pub async fn issue_detail(&self, issue_id: &str) -> Result<Issue> {
        #[derive(Deserialize)]
        struct Resp {
            issue: Issue,
        }
        let variables = serde_json::json!({ "id": issue_id });
        let query = format!(
            r#"query($id: String!) {{
                issue(id: $id) {{
                    {ISSUE_FIELDS}
                    comments(first: 100) {{
                        nodes {{
                            id
                            body
                            createdAt
                            editedAt
                            user {{ id name displayName }}
                            parent {{ id }}
                        }}
                    }}
                    children(first: 50) {{
                        nodes {{ id identifier title state {{ id name color type }} }}
                    }}
                    project {{ id name url color state }}
                    projectMilestone {{ id name }}
                    cycle {{ id name number }}
                }}
            }}"#
        );
        let resp: Resp = self.query(&query, Some(variables)).await?;
        Ok(resp.issue)
    }

    pub async fn workflow_states(&self, team_id: &str) -> Result<Vec<WorkflowState>> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "workflowStates")]
            workflow_states: Connection<WorkflowState>,
        }
        let variables = serde_json::json!({
            "teamId": team_id,
        });
        let resp: Resp = self
            .query(
                r#"query($teamId: ID!) {
                    workflowStates(filter: { team: { id: { eq: $teamId } } }) {
                        nodes { id name color type position }
                    }
                }"#,
                Some(variables),
            )
            .await?;
        Ok(resp.workflow_states.nodes)
    }

    pub async fn team_members(&self, team_id: &str) -> Result<Vec<User>> {
        #[derive(Deserialize)]
        struct TeamResp {
            team: TeamWithMembers,
        }
        #[derive(Deserialize)]
        struct TeamWithMembers {
            members: Connection<User>,
        }
        let variables = serde_json::json!({ "id": team_id });
        let resp: TeamResp = self
            .query(
                r#"query($id: String!) {
                    team(id: $id) {
                        members { nodes { id name displayName } }
                    }
                }"#,
                Some(variables),
            )
            .await?;
        Ok(resp.team.members.nodes)
    }

    pub async fn viewer(&self) -> Result<Viewer> {
        #[derive(Deserialize)]
        struct Resp {
            viewer: Viewer,
        }
        let resp: Resp = self
            .query("query { viewer { id name displayName } }", None)
            .await?;
        Ok(resp.viewer)
    }

    pub async fn my_issues(
        &self,
        user_id: &str,
        after: Option<&str>,
        first: u32,
    ) -> Result<(Vec<Issue>, PageInfo)> {
        #[derive(Deserialize)]
        struct Resp {
            issues: Connection<Issue>,
        }
        let variables = serde_json::json!({
            "userId": user_id,
            "after": after,
            "first": first,
        });
        let query = format!(
            r#"query($userId: ID!, $after: String, $first: Int!) {{
                issues(
                    filter: {{ assignee: {{ id: {{ eq: $userId }} }} }}
                    first: $first
                    after: $after
                    orderBy: updatedAt
                ) {{
                    nodes {{ {ISSUE_FIELDS} }}
                    pageInfo {{ hasNextPage endCursor }}
                }}
            }}"#
        );
        let resp: Resp = self.query(&query, Some(variables)).await?;
        Ok((resp.issues.nodes, resp.issues.page_info))
    }

    /// Workspace-wide full-text search, optionally scoped to one team.
    pub async fn search_issues(
        &self,
        term: &str,
        team_id: Option<&str>,
        first: u32,
    ) -> Result<(Vec<Issue>, PageInfo)> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "searchIssues")]
            search_issues: Connection<Issue>,
        }
        let variables = serde_json::json!({
            "term": term,
            "teamId": team_id,
            "first": first,
        });
        let query = format!(
            r#"query($term: String!, $teamId: String, $first: Int!) {{
                searchIssues(term: $term, teamId: $teamId, first: $first) {{
                    nodes {{ {ISSUE_FIELDS} }}
                    pageInfo {{ hasNextPage endCursor }}
                }}
            }}"#
        );
        let resp: Resp = self.query(&query, Some(variables)).await?;
        Ok((resp.search_issues.nodes, resp.search_issues.page_info))
    }

    /// Every saved view the user can open. The list is small and rarely
    /// changes, so it is fetched once at startup and lives in the sidebar.
    pub async fn custom_views(&self) -> Result<Vec<CustomView>> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "customViews")]
            custom_views: Connection<CustomView>,
        }
        let resp: Resp = self
            .query(
                r#"query {
                    customViews(first: 100) {
                        nodes {
                            id
                            name
                            description
                            color
                            shared
                            modelName
                            team { id name key color cyclesEnabled }
                            owner { id name displayName }
                        }
                    }
                }"#,
                None,
            )
            .await?;
        Ok(resp.custom_views.nodes)
    }

    /// The user's Favorites, in no particular order (the caller sorts them by
    /// `sortOrder`, as Linear's sidebar does).
    pub async fn favorites(&self) -> Result<Vec<Favorite>> {
        #[derive(Deserialize)]
        struct Resp {
            favorites: Connection<Favorite>,
        }
        let resp: Resp = self
            .query(
                r#"query {
                    favorites(first: 250) {
                        nodes {
                            id
                            type
                            title
                            url
                            color
                            sortOrder
                            folderName
                            predefinedViewType
                            parent { id }
                            predefinedViewTeam { id }
                            customView { id }
                            issue { id identifier title state { id name color type } }
                            project {
                                id name description state color health progress
                                startDate targetDate url
                                lead { id name displayName }
                            }
                            cycle { id name number startsAt endsAt progress }
                        }
                    }
                }"#,
                None,
            )
            .await?;
        Ok(resp.favorites.nodes)
    }

    /// Issues belonging to a saved view.
    ///
    /// The filter is evaluated by Linear, not here: `filterData` is an opaque
    /// JSON blob whose semantics are Linear's to define, and reimplementing it
    /// would drift the moment a user adds a condition this client has not seen.
    pub async fn custom_view_issues(
        &self,
        view_id: &str,
        after: Option<&str>,
        first: u32,
    ) -> Result<(Vec<Issue>, PageInfo)> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "customView")]
            custom_view: ViewWithIssues,
        }
        #[derive(Deserialize)]
        struct ViewWithIssues {
            issues: Connection<Issue>,
        }
        let variables = serde_json::json!({
            "id": view_id,
            "after": after,
            "first": first,
        });
        let query = format!(
            r#"query($id: String!, $after: String, $first: Int!) {{
                customView(id: $id) {{
                    issues(first: $first, after: $after) {{
                        nodes {{ {ISSUE_FIELDS} }}
                        pageInfo {{ hasNextPage endCursor }}
                    }}
                }}
            }}"#
        );
        let resp: Resp = self.query(&query, Some(variables)).await?;
        Ok((
            resp.custom_view.issues.nodes,
            resp.custom_view.issues.page_info,
        ))
    }

    /// Projects belonging to a saved project view — filtered by Linear, as
    /// with issue views.
    pub async fn custom_view_projects(
        &self,
        view_id: &str,
        after: Option<&str>,
    ) -> Result<(Vec<Project>, PageInfo)> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "customView")]
            custom_view: ViewWithProjects,
        }
        #[derive(Deserialize)]
        struct ViewWithProjects {
            projects: Connection<Project>,
        }
        let variables = serde_json::json!({
            "id": view_id,
            "after": after,
            "first": SUBLIST_PAGE_SIZE,
        });
        let query = format!(
            r#"query($id: String!, $after: String, $first: Int!) {{
                customView(id: $id) {{
                    projects(first: $first, after: $after) {{
                        nodes {{ {PROJECT_FIELDS} }}
                        pageInfo {{ hasNextPage endCursor }}
                    }}
                }}
            }}"#
        );
        let resp: Resp = self.query(&query, Some(variables)).await?;
        Ok((
            resp.custom_view.projects.nodes,
            resp.custom_view.projects.page_info,
        ))
    }

    pub async fn projects(
        &self,
        team_id: &str,
        after: Option<&str>,
    ) -> Result<(Vec<Project>, PageInfo)> {
        #[derive(Deserialize)]
        struct TeamResp {
            team: TeamWithProjects,
        }
        #[derive(Deserialize)]
        struct TeamWithProjects {
            projects: Connection<Project>,
        }
        let variables = serde_json::json!({
            "id": team_id,
            "after": after,
            "first": SUBLIST_PAGE_SIZE,
        });
        let query = format!(
            r#"query($id: String!, $after: String, $first: Int!) {{
                team(id: $id) {{
                    projects(first: $first, after: $after) {{
                        nodes {{ {PROJECT_FIELDS} }}
                        pageInfo {{ hasNextPage endCursor }}
                    }}
                }}
            }}"#
        );
        let resp: TeamResp = self.query(&query, Some(variables)).await?;
        Ok((resp.team.projects.nodes, resp.team.projects.page_info))
    }

    pub async fn project_issues(
        &self,
        project_id: &str,
        after: Option<&str>,
    ) -> Result<(Vec<Issue>, PageInfo)> {
        #[derive(Deserialize)]
        struct Resp {
            project: ProjectWithIssues,
        }
        #[derive(Deserialize)]
        struct ProjectWithIssues {
            issues: Connection<Issue>,
        }
        let variables = serde_json::json!({
            "id": project_id,
            "after": after,
            "first": SUBLIST_PAGE_SIZE,
        });
        let query = format!(
            r#"query($id: String!, $after: String, $first: Int!) {{
                project(id: $id) {{
                    issues(first: $first, after: $after) {{
                        nodes {{ {ISSUE_FIELDS} }}
                        pageInfo {{ hasNextPage endCursor }}
                    }}
                }}
            }}"#
        );
        let resp: Resp = self.query(&query, Some(variables)).await?;
        Ok((resp.project.issues.nodes, resp.project.issues.page_info))
    }

    pub async fn cycles(
        &self,
        team_id: &str,
        after: Option<&str>,
    ) -> Result<(Vec<Cycle>, PageInfo)> {
        #[derive(Deserialize)]
        struct TeamResp {
            team: TeamWithCycles,
        }
        #[derive(Deserialize)]
        struct TeamWithCycles {
            cycles: Connection<Cycle>,
        }
        let variables = serde_json::json!({
            "id": team_id,
            "after": after,
            "first": SUBLIST_PAGE_SIZE,
        });
        let resp: TeamResp = self
            .query(
                r#"query($id: String!, $after: String, $first: Int!) {
                    team(id: $id) {
                        cycles(orderBy: createdAt, first: $first, after: $after) {
                            nodes {
                                id
                                name
                                number
                                startsAt
                                endsAt
                                progress
                            }
                            pageInfo { hasNextPage endCursor }
                        }
                    }
                }"#,
                Some(variables),
            )
            .await?;
        Ok((resp.team.cycles.nodes, resp.team.cycles.page_info))
    }

    pub async fn cycle_issues(
        &self,
        cycle_id: &str,
        after: Option<&str>,
    ) -> Result<(Vec<Issue>, PageInfo)> {
        #[derive(Deserialize)]
        struct Resp {
            cycle: CycleWithIssues,
        }
        #[derive(Deserialize)]
        struct CycleWithIssues {
            issues: Connection<Issue>,
        }
        let variables = serde_json::json!({
            "id": cycle_id,
            "after": after,
            "first": SUBLIST_PAGE_SIZE,
        });
        let query = format!(
            r#"query($id: String!, $after: String, $first: Int!) {{
                cycle(id: $id) {{
                    issues(first: $first, after: $after) {{
                        nodes {{ {ISSUE_FIELDS} }}
                        pageInfo {{ hasNextPage endCursor }}
                    }}
                }}
            }}"#
        );
        let resp: Resp = self.query(&query, Some(variables)).await?;
        Ok((resp.cycle.issues.nodes, resp.cycle.issues.page_info))
    }

    // --- Mutations ---

    pub async fn update_issue_state(&self, issue_id: &str, state_id: &str) -> Result<()> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "issueUpdate")]
            _issue_update: MutationSuccess,
        }
        let variables = serde_json::json!({
            "id": issue_id,
            "stateId": state_id,
        });
        let _: Resp = self
            .query(
                r#"mutation($id: String!, $stateId: String!) {
                    issueUpdate(id: $id, input: { stateId: $stateId }) {
                        success
                    }
                }"#,
                Some(variables),
            )
            .await?;
        Ok(())
    }

    pub async fn update_issue_priority(&self, issue_id: &str, priority: u8) -> Result<()> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "issueUpdate")]
            _issue_update: MutationSuccess,
        }
        let variables = serde_json::json!({
            "id": issue_id,
            "priority": priority,
        });
        let _: Resp = self
            .query(
                r#"mutation($id: String!, $priority: Int!) {
                    issueUpdate(id: $id, input: { priority: $priority }) {
                        success
                    }
                }"#,
                Some(variables),
            )
            .await?;
        Ok(())
    }

    pub async fn update_issue_assignee(
        &self,
        issue_id: &str,
        assignee_id: Option<&str>,
    ) -> Result<()> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "issueUpdate")]
            _issue_update: MutationSuccess,
        }
        let variables = serde_json::json!({
            "id": issue_id,
            "assigneeId": assignee_id,
        });
        let _: Resp = self
            .query(
                r#"mutation($id: String!, $assigneeId: String) {
                    issueUpdate(id: $id, input: { assigneeId: $assigneeId }) {
                        success
                    }
                }"#,
                Some(variables),
            )
            .await?;
        Ok(())
    }

    pub async fn create_comment(&self, issue_id: &str, body: &str) -> Result<()> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "commentCreate")]
            _comment_create: MutationSuccess,
        }
        let variables = serde_json::json!({
            "issueId": issue_id,
            "body": body,
        });
        let _: Resp = self
            .query(
                r#"mutation($issueId: String!, $body: String!) {
                    commentCreate(input: { issueId: $issueId, body: $body }) {
                        success
                    }
                }"#,
                Some(variables),
            )
            .await?;
        Ok(())
    }

    /// Create an issue and return it, fully populated, for optimistic insertion.
    pub async fn create_issue(
        &self,
        team_id: &str,
        title: &str,
        description: Option<&str>,
        priority: u8,
    ) -> Result<Issue> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "issueCreate")]
            issue_create: IssueCreatePayload,
        }
        #[derive(Deserialize)]
        struct IssueCreatePayload {
            success: bool,
            issue: Option<Issue>,
        }
        let variables = serde_json::json!({
            "teamId": team_id,
            "title": title,
            "description": description,
            "priority": priority,
        });
        let query = format!(
            r#"mutation($teamId: String!, $title: String!, $description: String, $priority: Int) {{
                issueCreate(
                    input: {{
                        teamId: $teamId
                        title: $title
                        description: $description
                        priority: $priority
                    }}
                ) {{
                    success
                    issue {{ {ISSUE_FIELDS} }}
                }}
            }}"#
        );
        let resp: Resp = self.query(&query, Some(variables)).await?;
        if !resp.issue_create.success {
            anyhow::bail!("Linear rejected the issue");
        }
        resp.issue_create
            .issue
            .context("issueCreate returned no issue")
    }
}
