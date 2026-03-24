use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::types::*;

const API_URL: &str = "https://api.linear.app/graphql";

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
            .query("query { teams { nodes { id name key } } }", None)
            .await?;
        Ok(resp.teams.nodes)
    }

    pub async fn issues(
        &self,
        team_id: &str,
        after: Option<&str>,
        first: u32,
    ) -> Result<(Vec<Issue>, PageInfo)> {
        #[derive(Deserialize)]
        struct Resp {
            issues: Connection<Issue>,
        }
        let variables = serde_json::json!({
            "teamId": team_id,
            "after": after,
            "first": first,
        });
        let resp: Resp = self
            .query(
                r#"query($teamId: ID!, $after: String, $first: Int!) {
                    issues(
                        filter: { team: { id: { eq: $teamId } } }
                        first: $first
                        after: $after
                        orderBy: updatedAt
                    ) {
                        nodes {
                            id
                            identifier
                            title
                            priority
                            priorityLabel
                            state { id name color type }
                            assignee { id name displayName }
                            labels { nodes { id name color } }
                            description
                            createdAt
                            updatedAt
                        }
                        pageInfo { hasNextPage endCursor }
                    }
                }"#,
                Some(variables),
            )
            .await?;
        Ok((resp.issues.nodes, resp.issues.page_info))
    }

    pub async fn issue_detail(&self, issue_id: &str) -> Result<Issue> {
        #[derive(Deserialize)]
        struct Resp {
            issue: Issue,
        }
        let variables = serde_json::json!({ "id": issue_id });
        let resp: Resp = self
            .query(
                r#"query($id: String!) {
                    issue(id: $id) {
                        id
                        identifier
                        title
                        priority
                        priorityLabel
                        state { id name color type }
                        assignee { id name displayName }
                        labels { nodes { id name color } }
                        description
                        createdAt
                        updatedAt
                        comments {
                            nodes {
                                id
                                body
                                createdAt
                                user { id name displayName }
                            }
                        }
                        project { id name }
                        cycle { id name number }
                    }
                }"#,
                Some(variables),
            )
            .await?;
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
                        nodes { id name color type }
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
        let resp: Resp = self
            .query(
                r#"query($userId: ID!, $after: String, $first: Int!) {
                    issues(
                        filter: { assignee: { id: { eq: $userId } }, completedAt: { null: true } }
                        first: $first
                        after: $after
                        orderBy: updatedAt
                    ) {
                        nodes {
                            id
                            identifier
                            title
                            priority
                            priorityLabel
                            state { id name color type }
                            assignee { id name displayName }
                            labels { nodes { id name color } }
                            description
                            createdAt
                            updatedAt
                        }
                        pageInfo { hasNextPage endCursor }
                    }
                }"#,
                Some(variables),
            )
            .await?;
        Ok((resp.issues.nodes, resp.issues.page_info))
    }

    pub async fn projects(&self, team_id: &str) -> Result<Vec<Project>> {
        #[derive(Deserialize)]
        struct TeamResp {
            team: TeamWithProjects,
        }
        #[derive(Deserialize)]
        struct TeamWithProjects {
            projects: Connection<Project>,
        }
        let variables = serde_json::json!({ "id": team_id });
        let resp: TeamResp = self
            .query(
                r#"query($id: String!) {
                    team(id: $id) {
                        projects {
                            nodes {
                                id
                                name
                                state
                                progress
                                startDate
                                targetDate
                                lead { id name displayName }
                            }
                        }
                    }
                }"#,
                Some(variables),
            )
            .await?;
        Ok(resp.team.projects.nodes)
    }

    pub async fn project_issues(&self, project_id: &str) -> Result<Vec<Issue>> {
        #[derive(Deserialize)]
        struct Resp {
            project: ProjectWithIssues,
        }
        #[derive(Deserialize)]
        struct ProjectWithIssues {
            issues: Connection<Issue>,
        }
        let variables = serde_json::json!({ "id": project_id });
        let resp: Resp = self
            .query(
                r#"query($id: String!) {
                    project(id: $id) {
                        issues {
                            nodes {
                                id
                                identifier
                                title
                                priority
                                priorityLabel
                                state { id name color type }
                                assignee { id name displayName }
                                labels { nodes { id name color } }
                                description
                                createdAt
                                updatedAt
                            }
                        }
                    }
                }"#,
                Some(variables),
            )
            .await?;
        Ok(resp.project.issues.nodes)
    }

    pub async fn cycles(&self, team_id: &str) -> Result<Vec<super::types::Cycle>> {
        #[derive(Deserialize)]
        struct TeamResp {
            team: TeamWithCycles,
        }
        #[derive(Deserialize)]
        struct TeamWithCycles {
            cycles: Connection<super::types::Cycle>,
        }
        let variables = serde_json::json!({ "id": team_id });
        let resp: TeamResp = self
            .query(
                r#"query($id: String!) {
                    team(id: $id) {
                        cycles(orderBy: createdAt) {
                            nodes {
                                id
                                name
                                number
                                startsAt
                                endsAt
                                progress
                            }
                        }
                    }
                }"#,
                Some(variables),
            )
            .await?;
        Ok(resp.team.cycles.nodes)
    }

    pub async fn cycle_issues(&self, cycle_id: &str) -> Result<Vec<Issue>> {
        #[derive(Deserialize)]
        struct Resp {
            cycle: CycleWithIssues,
        }
        #[derive(Deserialize)]
        struct CycleWithIssues {
            issues: Connection<Issue>,
        }
        let variables = serde_json::json!({ "id": cycle_id });
        let resp: Resp = self
            .query(
                r#"query($id: String!) {
                    cycle(id: $id) {
                        issues {
                            nodes {
                                id
                                identifier
                                title
                                priority
                                priorityLabel
                                state { id name color type }
                                assignee { id name displayName }
                                labels { nodes { id name color } }
                                description
                                createdAt
                                updatedAt
                            }
                        }
                    }
                }"#,
                Some(variables),
            )
            .await?;
        Ok(resp.cycle.issues.nodes)
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

    pub async fn update_issue_description(&self, issue_id: &str, description: &str) -> Result<()> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "issueUpdate")]
            _issue_update: MutationSuccess,
        }
        let variables = serde_json::json!({
            "id": issue_id,
            "description": description,
        });
        let _: Resp = self
            .query(
                r#"mutation($id: String!, $description: String) {
                    issueUpdate(id: $id, input: { description: $description }) {
                        success
                    }
                }"#,
                Some(variables),
            )
            .await?;
        Ok(())
    }
}
