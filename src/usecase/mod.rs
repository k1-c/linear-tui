//! What the user can do, independent of how they asked.
//!
//! A use case takes the [`Store`](crate::store::Store) and explicit
//! arguments — which issue, which value — applies the optimistic change, and
//! returns the [`Request`](crate::message::Request) that makes it real. It
//! never looks at a cursor or a popup: resolving "the issue under the cursor"
//! is the caller's job, so a key, a click, the command palette, and a
//! headless subcommand all end up here.

pub mod issue;

/// Why a use case declined to run. The text is shown to the user as is.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("Current user is not loaded yet")]
    ViewerNotLoaded,
    #[error("A title is required")]
    TitleRequired,
}
