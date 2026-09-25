//! Talking to Linear from a subcommand: through the TUI's own request code,
//! so a subcommand and a key press send exactly the same query.

use anyhow::{Result, bail};

use crate::core::message::{self, Message};
use crate::core::usecase::Request;

/// Linear, signed in: carries out a use case's request the way the TUI does.
pub trait Linear {
    /// Linear's answer to `request`, a failure included.
    async fn execute(&self, request: Request) -> Message;

    /// Run one request; a failure becomes an error that says what failed.
    async fn run(&self, request: impl Into<Request>) -> Result<Message> {
        match self.execute(request.into()).await {
            Message::Failed { request, error } => {
                bail!("{}: {error}", message::failure(&request))
            }
            message => Ok(message),
        }
    }
}
