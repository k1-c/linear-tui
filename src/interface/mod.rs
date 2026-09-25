//! The ways in: who works linear-tui, and how.
//!
//! - [`tui`]: a person, in the terminal.
//! - [`control`]: an agent, working a running TUI the way a person does.
//! - [`cli`]: an agent or a script, through the subcommands.
//!
//! Each turns what it was asked into use cases (`crate::core`). `control`
//! reads the screen `tui` draws; nothing else here knows another way in.
//! Only `cli` reaches into `crate::infra`: each subcommand runs on its own,
//! so it assembles the Linear client and the files it needs, as `main` does
//! for the TUI.

pub mod cli;
pub mod control;
pub mod tui;
