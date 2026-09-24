//! Turning a [`Request`] into the API calls it stands for.
//!
//! This runs on the tokio runtime, off the UI thread; the main loop spawns one
//! task per request and sends the resulting [`Message`] back over a channel.

use anyhow::Result;

use crate::api::client::LinearClient;
use crate::message::{Message, Page, Request};

/// Run one request against the API and turn the outcome into a [`Message`].
///
/// A failure comes back as [`Message::Failed`] carrying the request, so the app
/// can undo exactly what that request stood for.
pub async fn execute_request(client: &LinearClient, req: Request, per_page: u32) -> Message {
    match run_request(client, &req, per_page).await {
        Ok(msg) => msg,
        Err(e) => Message::Failed {
            request: Box::new(req),
            error: e.to_string(),
        },
    }
}

async fn run_request(client: &LinearClient, req: &Request, per_page: u32) -> Result<Message> {
    let append = req.cursor().is_some();
    Ok(match req {
        Request::Teams => Message::Teams(client.teams().await?),
        Request::Viewer => Message::Viewer(client.viewer().await?.id),
        Request::TeamContext { team_id } => {
            // Independent queries — fetch them concurrently.
            let (states, members) = tokio::join!(
                client.workflow_states(team_id),
                client.team_members(team_id)
            );
            Message::TeamContext {
                team_id: team_id.clone(),
                states: states?,
                members: members?,
            }
        }
        Request::Issues {
            team_id,
            after,
            preset,
        } => {
            let (issues, info) = client
                .issues(team_id, preset.state_filter(), after.as_deref(), per_page)
                .await?;
            Message::Issues {
                team_id: team_id.clone(),
                preset: *preset,
                page: Page::new(issues, info, append),
            }
        }
        Request::MyIssues { user_id, after } => {
            let (issues, info) = client
                .my_issues(user_id, after.as_deref(), per_page)
                .await?;
            Message::MyIssues(Page::new(issues, info, append))
        }
        Request::Search { term, team_id } => {
            let (issues, _) = client
                .search_issues(term, team_id.as_ref(), per_page)
                .await?;
            Message::SearchResults {
                term: term.clone(),
                team_id: team_id.clone(),
                issues,
            }
        }
        Request::PaletteSearch { term, seq } => {
            let (issues, _) = client.search_issues(term, None, per_page).await?;
            Message::PaletteResults { seq: *seq, issues }
        }
        Request::CustomViews => Message::CustomViews(client.custom_views().await?),
        Request::Favorites => Message::Favorites(client.favorites().await?),
        Request::ViewIssues { view_id, after } => {
            let (issues, info) = client
                .custom_view_issues(view_id, after.as_deref(), per_page)
                .await?;
            Message::ViewIssues {
                view_id: view_id.clone(),
                page: Page::new(issues, info, append),
            }
        }
        Request::ViewProjects { view_id, after } => {
            let (projects, info) = client
                .custom_view_projects(view_id, after.as_deref())
                .await?;
            Message::ViewProjects {
                view_id: view_id.clone(),
                page: Page::new(projects, info, append),
            }
        }
        Request::Projects { team_id, after } => {
            let (projects, info) = client.projects(team_id, after.as_deref()).await?;
            Message::Projects {
                team_id: team_id.clone(),
                page: Page::new(projects, info, append),
            }
        }
        Request::Cycles { team_id, after } => {
            let (cycles, info) = client.cycles(team_id, after.as_deref()).await?;
            Message::Cycles {
                team_id: team_id.clone(),
                page: Page::new(cycles, info, append),
            }
        }
        Request::IssueDetail { issue_id } => {
            Message::IssueDetail(Box::new(client.issue_detail(issue_id).await?))
        }
        Request::ProjectIssues { project_id, after } => {
            let (issues, info) = client.project_issues(project_id, after.as_deref()).await?;
            Message::ProjectIssues {
                project_id: project_id.clone(),
                page: Page::new(issues, info, append),
            }
        }
        Request::CycleIssues { cycle_id, after } => {
            let (issues, info) = client.cycle_issues(cycle_id, after.as_deref()).await?;
            Message::CycleIssues {
                cycle_id: cycle_id.clone(),
                page: Page::new(issues, info, append),
            }
        }
        Request::UpdateStatus { issue_id, state_id } => {
            client.update_issue_state(issue_id, state_id).await?;
            Message::Mutated("Status updated")
        }
        Request::UpdatePriority { issue_id, priority } => {
            client.update_issue_priority(issue_id, *priority).await?;
            Message::Mutated("Priority updated")
        }
        Request::UpdateAssignee {
            issue_id,
            assignee_id,
        } => {
            client
                .update_issue_assignee(issue_id, assignee_id.as_ref())
                .await?;
            Message::Mutated("Assignee updated")
        }
        Request::CreateComment { issue_id, body } => {
            client.create_comment(issue_id, body).await?;
            Message::Mutated("Comment posted")
        }
        Request::CreateIssue {
            team_id,
            title,
            description,
            priority,
        } => {
            let issue = client
                .create_issue(team_id, title, description.as_deref(), *priority)
                .await?;
            Message::IssueCreated {
                team_id: team_id.clone(),
                issue: Box::new(issue),
            }
        }
        Request::OpenUrl(url) => {
            open::that_detached(url)?;
            Message::Mutated("Opened in browser")
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::api::client::StaticCredentials;
    use crate::api::ids::TeamId;

    fn client(server: &MockServer) -> LinearClient {
        LinearClient::with_endpoint(server.uri(), Arc::new(StaticCredentials("key".into())))
    }

    #[tokio::test]
    async fn a_page_names_the_team_it_was_fetched_for() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({ "data": { "team": {
                "cycles": { "nodes": [], "pageInfo": { "hasNextPage": false } }
            } } })),
            )
            .mount(&server)
            .await;

        let request = Request::Cycles {
            team_id: TeamId::new("t1"),
            after: Some("c1".into()),
        };
        match execute_request(&client(&server), request, 50).await {
            Message::Cycles { team_id, page } => {
                assert_eq!(team_id, "t1");
                assert!(page.append, "a page after a cursor extends the list");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_failure_carries_the_request_back() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let request = Request::Teams;
        match execute_request(&client(&server), request.clone(), 50).await {
            Message::Failed {
                request: failed,
                error,
            } => {
                assert_eq!(*failed, request);
                assert!(error.contains("500"), "{error}");
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
