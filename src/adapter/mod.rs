//! The adapters: linear-tui's edges with the world outside it.
//!
//! - [`api`]: Linear's GraphQL API, and [`dispatch`], which carries out a
//!   use case's `Request` through it.
//! - [`auth`]: signing in (OAuth, API keys) and keeping the token, with
//!   [`private_file`] for files only the user may read.
//! - [`cli`]: the subcommands, including the headless ones agents run.
//! - [`herdr`]: the hand-off to the herdr plugin.
//! - [`snapshot`]: the view snapshots on disk.
//!
//! Adapters may depend on every layer further in.

pub mod api;
pub mod auth;
pub mod cli;
pub mod control;
pub mod dispatch;
pub mod herdr;
pub mod private_file;
pub mod runtime;
pub mod snapshot;
