//! What can go wrong talking to Linear, told apart so callers can react.

use std::fmt;
use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// The request never got an answer: DNS, TLS, a dropped connection, or
    /// the timeout.
    #[error("could not reach Linear: {0}")]
    Transport(#[source] reqwest::Error),

    /// The credentials were refused, and refreshing them (where possible)
    /// did not help.
    #[error("Linear rejected the credentials. Run `linear-tui auth login` to sign in again.")]
    Unauthorized,

    #[error("Linear is rate limiting requests{}", retry_hint(*.retry_after))]
    RateLimited { retry_after: Option<Duration> },

    /// A non-success HTTP status that is none of the above.
    #[error("API error ({status}): {body}")]
    Http {
        status: reqwest::StatusCode,
        body: String,
    },

    /// Linear answered, but with GraphQL errors instead of data.
    #[error("{}", join(.0))]
    GraphQL(Vec<GraphQLError>),

    /// The response did not have the shape this client expects — the schema
    /// moved under us.
    #[error("unexpected response from Linear: {0}")]
    Decode(String),

    /// A mutation came back with `success: false`.
    #[error("{0}")]
    Rejected(&'static str),

    /// Fetching fresh credentials failed.
    #[error("could not refresh the session: {0}")]
    Refresh(String),
}

/// One entry of a GraphQL `errors` array.
#[derive(Debug, Clone, Deserialize)]
pub struct GraphQLError {
    pub message: String,
    #[serde(default)]
    pub extensions: Option<Extensions>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Extensions {
    /// Linear's machine-readable reason, e.g. `RATELIMITED` or
    /// `AUTHENTICATION_ERROR`.
    #[serde(default)]
    pub code: Option<String>,
}

impl GraphQLError {
    pub fn code(&self) -> Option<&str> {
        self.extensions.as_ref()?.code.as_deref()
    }
}

impl fmt::Display for GraphQLError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl ApiError {
    /// Classify a GraphQL error list: Linear reports authentication and rate
    /// limiting as error codes, often with a 200 or 400 status.
    pub fn from_graphql(errors: Vec<GraphQLError>, retry_after: Option<Duration>) -> Self {
        if errors
            .iter()
            .any(|e| e.code() == Some("AUTHENTICATION_ERROR"))
        {
            Self::Unauthorized
        } else if errors.iter().any(|e| e.code() == Some("RATELIMITED")) {
            Self::RateLimited { retry_after }
        } else {
            Self::GraphQL(errors)
        }
    }
}

fn join(errors: &[GraphQLError]) -> String {
    let messages: Vec<_> = errors.iter().map(|e| e.message.as_str()).collect();
    format!("GraphQL errors: {}", messages.join(", "))
}

fn retry_hint(retry_after: Option<Duration>) -> String {
    match retry_after {
        Some(wait) => format!("; try again in {}s", wait.as_secs().max(1)),
        None => "; try again shortly".to_string(),
    }
}
