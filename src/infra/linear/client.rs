use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::error::{ApiError, GraphQLError};
use crate::core::entity::*;

const API_URL: &str = "https://api.linear.app/graphql";

/// Upper bound on one API call. A stalled connection would otherwise keep its
/// request — and the spinner — going until the app is closed.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

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
    team { id }
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

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Where the `Authorization` header comes from.
///
/// An API key never changes, but an OAuth access token expires, and a TUI
/// session easily outlives one. The client asks for the header before every
/// request and, when Linear refuses it, gives the source one chance to replace
/// it before the request is retried.
pub trait Credentials: Send + Sync {
    /// The value of the `Authorization` header for the next request.
    fn authorization(&self) -> BoxFuture<'_, Result<String, ApiError>>;

    /// Linear refused `rejected`. Returns whether there is now a different
    /// header worth retrying with.
    fn refresh<'a>(&'a self, rejected: &'a str) -> BoxFuture<'a, Result<bool, ApiError>>;
}

/// A header that is what it is: an API key, or a token nobody can refresh.
pub struct StaticCredentials(pub String);

impl Credentials for StaticCredentials {
    fn authorization(&self) -> BoxFuture<'_, Result<String, ApiError>> {
        let header = self.0.clone();
        Box::pin(async move { Ok(header) })
    }

    fn refresh<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<bool, ApiError>> {
        Box::pin(async { Ok(false) })
    }
}

pub struct LinearClient {
    http: reqwest::Client,
    endpoint: String,
    credentials: Arc<dyn Credentials>,
}

#[derive(serde::Serialize)]
struct GraphQLRequest<'a> {
    query: &'a str,
    variables: &'a Value,
}

/// A GraphQL response before `data` is given a type, so that errors survive
/// even when `data` is partial or null.
#[derive(Deserialize)]
struct RawResponse {
    #[serde(default)]
    data: Option<Value>,
    #[serde(default)]
    errors: Option<Vec<GraphQLError>>,
}

type Paged<T> = Result<(Vec<T>, PageInfo), ApiError>;

impl LinearClient {
    pub fn new(credentials: Arc<dyn Credentials>) -> Self {
        Self::with_endpoint(API_URL, credentials)
    }

    /// A client for a fixed `Authorization` header.
    pub fn with_header(header: String) -> Self {
        Self::new(Arc::new(StaticCredentials(header)))
    }

    /// A client against another GraphQL endpoint — a mock server, in tests.
    pub fn with_endpoint(endpoint: impl Into<String>, credentials: Arc<dyn Credentials>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .connect_timeout(CONNECT_TIMEOUT)
                .build()
                .expect("a client with only timeouts set always builds"),
            endpoint: endpoint.into(),
            credentials,
        }
    }

    /// Run a query and deserialize its `data`.
    ///
    /// A refused credential is refreshed once and the request retried, so an
    /// access token that expires mid-session costs one round trip instead of
    /// a restart.
    async fn query<T: DeserializeOwned>(
        &self,
        query: &str,
        variables: Value,
    ) -> Result<T, ApiError> {
        let operation = operation_name(query);
        let header = self.credentials.authorization().await?;
        let data = match self.send(operation, query, &variables, &header).await {
            Err(ApiError::Unauthorized) if self.credentials.refresh(&header).await? => {
                tracing::info!(operation, "credentials refreshed, retrying");
                let header = self.credentials.authorization().await?;
                self.send(operation, query, &variables, &header).await?
            }
            result => result?,
        };
        serde_json::from_value(data).map_err(|e| decode_error(operation, e))
    }

    /// Run a query and pull the connection at `path` out of its `data`.
    async fn connection<T: DeserializeOwned>(
        &self,
        query: &str,
        variables: Value,
        path: &str,
    ) -> Paged<T> {
        let data: Value = self.query(query, variables).await?;
        let connection = data
            .pointer(path)
            .cloned()
            .ok_or_else(|| ApiError::Decode(format!("{path} missing from the response")))?;
        let connection: Connection<T> = serde_json::from_value(connection)
            .map_err(|e| decode_error(operation_name(query), e))?;
        Ok((connection.nodes, connection.page_info))
    }

    /// Run a mutation and return its payload, the object under `field`.
    ///
    /// Linear reports a refused mutation as `success: false` rather than as a
    /// GraphQL error, so it is checked here: the UI has already applied the
    /// change optimistically and has to hear that it did not stick.
    async fn mutate(
        &self,
        query: &str,
        variables: Value,
        field: &str,
        rejected: &'static str,
    ) -> Result<Value, ApiError> {
        let mut data: Value = self.query(query, variables).await?;
        let payload = data
            .get_mut(field)
            .map(Value::take)
            .ok_or_else(|| ApiError::Decode(format!("{field} missing from the response")))?;
        if payload.get("success").and_then(Value::as_bool) != Some(true) {
            return Err(ApiError::Rejected(rejected));
        }
        Ok(payload)
    }

    async fn send(
        &self,
        operation: &str,
        query: &str,
        variables: &Value,
        header: &str,
    ) -> Result<Value, ApiError> {
        tracing::debug!(operation, "sending GraphQL request");
        tracing::trace!(operation, variables = %variables, "request variables");

        let resp = self
            .http
            .post(&self.endpoint)
            .header(reqwest::header::AUTHORIZATION, header)
            .json(&GraphQLRequest { query, variables })
            .send()
            .await
            .map_err(ApiError::Transport)?;

        let status = resp.status();
        let retry_after = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .map(Duration::from_secs);
        let body = resp.text().await.map_err(ApiError::Transport)?;
        tracing::debug!(operation, %status, body_len = body.len(), "received response");

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ApiError::Unauthorized);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ApiError::RateLimited { retry_after });
        }

        let parsed: Option<RawResponse> = serde_json::from_str(&body).ok();
        if let Some(errors) = parsed.as_ref().and_then(|r| r.errors.clone())
            && !errors.is_empty()
        {
            let error = ApiError::from_graphql(errors, retry_after);
            tracing::error!(operation, %status, %error, "GraphQL errors");
            return Err(error);
        }
        if !status.is_success() {
            tracing::error!(operation, %status, "API error");
            return Err(ApiError::Http { status, body });
        }
        match parsed {
            Some(RawResponse {
                data: Some(data), ..
            }) => Ok(data),
            Some(_) => Err(ApiError::Decode("no data in the response".into())),
            None => {
                // The body is workspace content; it goes to the log only when
                // asked for with RUST_LOG=trace.
                tracing::error!(operation, body_len = body.len(), "response is not GraphQL");
                tracing::trace!(operation, response_body = %body, "unparsed response");
                Err(ApiError::Decode("the response is not JSON".into()))
            }
        }
    }

    pub async fn teams(&self) -> Result<Vec<Team>, ApiError> {
        let (teams, _) = self
            .connection(
                "query Teams { teams(first: 100) { nodes { id name key color cyclesEnabled } } }",
                Value::Null,
                "/teams",
            )
            .await?;
        Ok(teams)
    }

    /// A team's issues, newest activity first, optionally narrowed to some
    /// workflow categories (`state` is an `IssueFilter.state` clause).
    pub async fn issues(
        &self,
        team_id: &TeamId,
        state: Option<Value>,
        after: Option<&str>,
        first: u32,
    ) -> Paged<Issue> {
        let mut filter = json!({ "team": { "id": { "eq": team_id } } });
        if let Some(state) = state {
            filter["state"] = state;
        }
        let query = format!(
            r#"query TeamIssues($filter: IssueFilter, $after: String, $first: Int!) {{
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
        self.connection(
            &query,
            json!({ "filter": filter, "after": after, "first": first }),
            "/issues",
        )
        .await
    }

    pub async fn issue_detail(&self, issue_id: &IssueId) -> Result<Issue, ApiError> {
        #[derive(Deserialize)]
        struct Resp {
            issue: Issue,
        }
        let query = format!(
            r#"query IssueDetail($id: String!) {{
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
        let resp: Resp = self.query(&query, json!({ "id": issue_id })).await?;
        Ok(resp.issue)
    }

    pub async fn workflow_states(&self, team_id: &TeamId) -> Result<Vec<WorkflowState>, ApiError> {
        let (states, _) = self
            .connection(
                r#"query WorkflowStates($teamId: ID!) {
                    workflowStates(filter: { team: { id: { eq: $teamId } } }) {
                        nodes { id name color type position }
                    }
                }"#,
                json!({ "teamId": team_id }),
                "/workflowStates",
            )
            .await?;
        Ok(states)
    }

    pub async fn team_members(&self, team_id: &TeamId) -> Result<Vec<User>, ApiError> {
        let (members, _) = self
            .connection(
                r#"query TeamMembers($id: String!) {
                    team(id: $id) {
                        members { nodes { id name displayName } }
                    }
                }"#,
                json!({ "id": team_id }),
                "/team/members",
            )
            .await?;
        Ok(members)
    }

    pub async fn viewer(&self) -> Result<Viewer, ApiError> {
        #[derive(Deserialize)]
        struct Resp {
            viewer: Viewer,
        }
        let resp: Resp = self
            .query(
                "query Viewer { viewer { id name displayName organization { id name urlKey } } }",
                Value::Null,
            )
            .await?;
        Ok(resp.viewer)
    }

    pub async fn my_issues(
        &self,
        user_id: &UserId,
        after: Option<&str>,
        first: u32,
    ) -> Paged<Issue> {
        let query = format!(
            r#"query MyIssues($userId: ID!, $after: String, $first: Int!) {{
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
        self.connection(
            &query,
            json!({ "userId": user_id, "after": after, "first": first }),
            "/issues",
        )
        .await
    }

    /// Workspace-wide full-text search, optionally scoped to one team.
    pub async fn search_issues(
        &self,
        term: &str,
        team_id: Option<&TeamId>,
        first: u32,
    ) -> Paged<Issue> {
        let query = format!(
            r#"query SearchIssues($term: String!, $teamId: String, $first: Int!) {{
                searchIssues(term: $term, teamId: $teamId, first: $first) {{
                    nodes {{ {ISSUE_FIELDS} }}
                    pageInfo {{ hasNextPage endCursor }}
                }}
            }}"#
        );
        self.connection(
            &query,
            json!({ "term": term, "teamId": team_id, "first": first }),
            "/searchIssues",
        )
        .await
    }

    /// Every saved view the user can open. The list is small and rarely
    /// changes, so it is fetched once at startup and lives in the sidebar.
    pub async fn custom_views(&self) -> Result<Vec<CustomView>, ApiError> {
        let (views, _) = self
            .connection(
                r#"query CustomViews {
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
                Value::Null,
                "/customViews",
            )
            .await?;
        Ok(views)
    }

    /// The user's Favorites, in no particular order (the caller sorts them by
    /// `sortOrder`, as Linear's sidebar does).
    pub async fn favorites(&self) -> Result<Vec<Favorite>, ApiError> {
        let (favorites, _) = self
            .connection(
                r#"query Favorites {
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
                Value::Null,
                "/favorites",
            )
            .await?;
        Ok(favorites)
    }

    /// Issues belonging to a saved view.
    ///
    /// The filter is evaluated by Linear, not here: `filterData` is an opaque
    /// JSON blob whose semantics are Linear's to define, and reimplementing it
    /// would drift the moment a user adds a condition this client has not seen.
    pub async fn custom_view_issues(
        &self,
        view_id: &CustomViewId,
        after: Option<&str>,
        first: u32,
    ) -> Paged<Issue> {
        let query = format!(
            r#"query CustomViewIssues($id: String!, $after: String, $first: Int!) {{
                customView(id: $id) {{
                    issues(first: $first, after: $after) {{
                        nodes {{ {ISSUE_FIELDS} }}
                        pageInfo {{ hasNextPage endCursor }}
                    }}
                }}
            }}"#
        );
        self.connection(
            &query,
            json!({ "id": view_id, "after": after, "first": first }),
            "/customView/issues",
        )
        .await
    }

    /// Projects belonging to a saved project view — filtered by Linear, as
    /// with issue views.
    pub async fn custom_view_projects(
        &self,
        view_id: &CustomViewId,
        after: Option<&str>,
    ) -> Paged<Project> {
        let query = format!(
            r#"query CustomViewProjects($id: String!, $after: String, $first: Int!) {{
                customView(id: $id) {{
                    projects(first: $first, after: $after) {{
                        nodes {{ {PROJECT_FIELDS} }}
                        pageInfo {{ hasNextPage endCursor }}
                    }}
                }}
            }}"#
        );
        self.connection(
            &query,
            json!({ "id": view_id, "after": after, "first": SUBLIST_PAGE_SIZE }),
            "/customView/projects",
        )
        .await
    }

    pub async fn projects(&self, team_id: &TeamId, after: Option<&str>) -> Paged<Project> {
        let query = format!(
            r#"query TeamProjects($id: String!, $after: String, $first: Int!) {{
                team(id: $id) {{
                    projects(first: $first, after: $after) {{
                        nodes {{ {PROJECT_FIELDS} }}
                        pageInfo {{ hasNextPage endCursor }}
                    }}
                }}
            }}"#
        );
        self.connection(
            &query,
            json!({ "id": team_id, "after": after, "first": SUBLIST_PAGE_SIZE }),
            "/team/projects",
        )
        .await
    }

    pub async fn project_issues(
        &self,
        project_id: &ProjectId,
        after: Option<&str>,
    ) -> Paged<Issue> {
        let query = format!(
            r#"query ProjectIssues($id: String!, $after: String, $first: Int!) {{
                project(id: $id) {{
                    issues(first: $first, after: $after) {{
                        nodes {{ {ISSUE_FIELDS} }}
                        pageInfo {{ hasNextPage endCursor }}
                    }}
                }}
            }}"#
        );
        self.connection(
            &query,
            json!({ "id": project_id, "after": after, "first": SUBLIST_PAGE_SIZE }),
            "/project/issues",
        )
        .await
    }

    pub async fn cycles(&self, team_id: &TeamId, after: Option<&str>) -> Paged<Cycle> {
        self.connection(
            r#"query TeamCycles($id: String!, $after: String, $first: Int!) {
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
            json!({ "id": team_id, "after": after, "first": SUBLIST_PAGE_SIZE }),
            "/team/cycles",
        )
        .await
    }

    pub async fn cycle_issues(&self, cycle_id: &CycleId, after: Option<&str>) -> Paged<Issue> {
        let query = format!(
            r#"query CycleIssues($id: String!, $after: String, $first: Int!) {{
                cycle(id: $id) {{
                    issues(first: $first, after: $after) {{
                        nodes {{ {ISSUE_FIELDS} }}
                        pageInfo {{ hasNextPage endCursor }}
                    }}
                }}
            }}"#
        );
        self.connection(
            &query,
            json!({ "id": cycle_id, "after": after, "first": SUBLIST_PAGE_SIZE }),
            "/cycle/issues",
        )
        .await
    }

    // --- Mutations ---

    /// Apply one `IssueUpdateInput` to an issue.
    async fn update_issue(&self, issue_id: &IssueId, input: Value) -> Result<(), ApiError> {
        self.mutate(
            r#"mutation UpdateIssue($id: String!, $input: IssueUpdateInput!) {
                issueUpdate(id: $id, input: $input) {
                    success
                }
            }"#,
            json!({ "id": issue_id, "input": input }),
            "issueUpdate",
            "Linear rejected the update",
        )
        .await?;
        Ok(())
    }

    pub async fn update_issue_state(
        &self,
        issue_id: &IssueId,
        state_id: &WorkflowStateId,
    ) -> Result<(), ApiError> {
        self.update_issue(issue_id, json!({ "stateId": state_id }))
            .await
    }

    pub async fn update_issue_priority(
        &self,
        issue_id: &IssueId,
        priority: Priority,
    ) -> Result<(), ApiError> {
        self.update_issue(issue_id, json!({ "priority": priority.as_u8() }))
            .await
    }

    pub async fn update_issue_assignee(
        &self,
        issue_id: &IssueId,
        assignee_id: Option<&UserId>,
    ) -> Result<(), ApiError> {
        self.update_issue(issue_id, json!({ "assigneeId": assignee_id }))
            .await
    }

    pub async fn create_comment(&self, issue_id: &IssueId, body: &str) -> Result<(), ApiError> {
        self.mutate(
            r#"mutation CreateComment($issueId: String!, $body: String!) {
                commentCreate(input: { issueId: $issueId, body: $body }) {
                    success
                }
            }"#,
            json!({ "issueId": issue_id, "body": body }),
            "commentCreate",
            "Linear rejected the comment",
        )
        .await?;
        Ok(())
    }

    /// Create an issue and return it, fully populated, for optimistic insertion.
    pub async fn create_issue(
        &self,
        team_id: &TeamId,
        title: &str,
        description: Option<&str>,
        priority: Priority,
    ) -> Result<Issue, ApiError> {
        let query = format!(
            r#"mutation CreateIssue($teamId: String!, $title: String!, $description: String, $priority: Int) {{
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
        let mut payload = self
            .mutate(
                &query,
                json!({
                    "teamId": team_id,
                    "title": title,
                    "description": description,
                    "priority": priority.as_u8(),
                }),
                "issueCreate",
                "Linear rejected the issue",
            )
            .await?;
        let issue = payload
            .get_mut("issue")
            .map(Value::take)
            .filter(|v| !v.is_null())
            .ok_or_else(|| ApiError::Decode("issueCreate returned no issue".into()))?;
        serde_json::from_value(issue).map_err(|e| decode_error("CreateIssue", e))
    }
}

/// The operation name of a query — `TeamIssues` in `query TeamIssues(…)` —
/// for the log.
fn operation_name(query: &str) -> &str {
    let mut words = query.split_whitespace();
    words.next();
    words
        .next()
        .and_then(|w| w.split(['(', '{']).next())
        .filter(|w| !w.is_empty())
        .unwrap_or("anonymous")
}

fn decode_error(operation: &str, error: serde_json::Error) -> ApiError {
    tracing::error!(operation, %error, "response does not match the expected shape");
    ApiError::Decode(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use wiremock::matchers::{body_partial_json, header, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn fixture(name: &str) -> Value {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn client(server: &MockServer) -> LinearClient {
        LinearClient::with_endpoint(
            server.uri(),
            Arc::new(StaticCredentials("lin_api_test".into())),
        )
    }

    fn data(value: Value) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(json!({ "data": value }))
    }

    #[test]
    fn operation_names_are_read_from_the_query() {
        assert_eq!(operation_name("query Teams { teams }"), "Teams");
        assert_eq!(operation_name("query TeamIssues($x: Int) {}"), "TeamIssues");
        assert_eq!(
            operation_name("mutation UpdateIssue($id: String!)"),
            "UpdateIssue"
        );
        assert_eq!(operation_name("query { viewer { id } }"), "anonymous");
    }

    #[tokio::test]
    async fn sends_the_credentials_and_reads_a_connection() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(header("authorization", "lin_api_test"))
            .respond_with(data(fixture("teams.json")))
            .expect(1)
            .mount(&server)
            .await;

        let teams = client(&server).teams().await.unwrap();
        assert!(!teams.is_empty());
    }

    #[tokio::test]
    async fn pages_carry_their_cursor_and_variables() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(json!({
                "variables": { "id": "team-1", "after": "c1", "first": 100 }
            })))
            .respond_with(data(json!({ "team": { "projects": {
                "nodes": [{ "id": "p1", "name": "Launch", "lead": null }],
                "pageInfo": { "hasNextPage": true, "endCursor": "c2" }
            } } })))
            .expect(1)
            .mount(&server)
            .await;

        let (projects, info) = client(&server)
            .projects(&TeamId::new("team-1"), Some("c1"))
            .await
            .unwrap();
        assert_eq!(projects[0].id, "p1");
        assert!(info.has_next_page);
        assert_eq!(info.end_cursor.as_deref(), Some("c2"));
    }

    #[tokio::test]
    async fn graphql_errors_are_reported_with_their_message() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "errors": [{ "message": "Entity not found", "extensions": { "code": "INVALID_INPUT" } }]
            })))
            .mount(&server)
            .await;

        let error = client(&server)
            .issue_detail(&IssueId::new("x"))
            .await
            .unwrap_err();
        assert!(matches!(&error, ApiError::GraphQL(e) if e[0].code() == Some("INVALID_INPUT")));
        assert!(error.to_string().contains("Entity not found"));
    }

    #[tokio::test]
    async fn rate_limiting_is_told_apart() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(400)
                    .insert_header("retry-after", "12")
                    .set_body_json(json!({
                        "errors": [{ "message": "Rate limit exceeded", "extensions": { "code": "RATELIMITED" } }]
                    })),
            )
            .mount(&server)
            .await;

        let error = client(&server).viewer().await.unwrap_err();
        assert!(matches!(
            error,
            ApiError::RateLimited { retry_after: Some(d) } if d == Duration::from_secs(12)
        ));
    }

    #[tokio::test]
    async fn a_refused_mutation_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(data(json!({ "issueUpdate": { "success": false } })))
            .mount(&server)
            .await;

        let error = client(&server)
            .update_issue_priority(&IssueId::new("i1"), Priority::High)
            .await
            .unwrap_err();
        assert!(matches!(error, ApiError::Rejected(_)));
    }

    #[tokio::test]
    async fn a_mutation_sends_the_priority_as_a_number() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(json!({
                "variables": { "id": "i1", "input": { "priority": 2 } }
            })))
            .respond_with(data(json!({ "issueUpdate": { "success": true } })))
            .expect(1)
            .mount(&server)
            .await;

        client(&server)
            .update_issue_priority(&IssueId::new("i1"), Priority::High)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_body_that_is_not_json_is_a_decode_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>"))
            .mount(&server)
            .await;

        let error = client(&server).viewer().await.unwrap_err();
        assert!(matches!(error, ApiError::Decode(_)));
    }

    /// Hands out `old` until refreshed, then `new`.
    struct Rotating {
        current: Mutex<String>,
        refreshes: AtomicUsize,
    }

    impl Credentials for Rotating {
        fn authorization(&self) -> BoxFuture<'_, Result<String, ApiError>> {
            let header = self.current.lock().unwrap().clone();
            Box::pin(async move { Ok(header) })
        }

        fn refresh<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<bool, ApiError>> {
            Box::pin(async move {
                self.refreshes.fetch_add(1, Ordering::SeqCst);
                *self.current.lock().unwrap() = "Bearer new".into();
                Ok(true)
            })
        }
    }

    #[tokio::test]
    async fn an_expired_token_is_refreshed_once_and_the_request_retried() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(header("authorization", "Bearer old"))
            .respond_with(ResponseTemplate::new(401))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(header("authorization", "Bearer new"))
            .respond_with(data(json!({ "viewer": { "id": "u1", "name": "Ada" } })))
            .expect(1)
            .mount(&server)
            .await;

        let credentials = Arc::new(Rotating {
            current: Mutex::new("Bearer old".into()),
            refreshes: AtomicUsize::new(0),
        });
        let client = LinearClient::with_endpoint(server.uri(), credentials.clone());
        assert_eq!(client.viewer().await.unwrap().id, "u1");
        assert_eq!(credentials.refreshes.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn credentials_that_cannot_refresh_report_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "errors": [{ "message": "Authentication required", "extensions": { "code": "AUTHENTICATION_ERROR" } }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let error = client(&server).viewer().await.unwrap_err();
        assert!(matches!(error, ApiError::Unauthorized));
    }
}
