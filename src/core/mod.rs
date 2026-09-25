//! What linear-tui is, apart from how it is used or what it talks to: no
//! terminal, no network, no files.
//!
//! - [`entity`]: Linear's issues, teams, projects, and the rest, and what
//!   linear-tui adds beside them.
//! - [`store`]: everything Linear has told us, kept consistent.
//! - [`usecase`]: what a person or an agent can do — the specification.
//! - [`message`]: Linear's answers to the use cases' requests, on their way
//!   back to the store.
//!
//! Always named as `crate::core::…`: a bare `core::` is Rust's own crate.

pub mod entity;
pub mod message;
pub mod store;
pub mod usecase;
