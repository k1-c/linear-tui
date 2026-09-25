//! The ways in: who works linear-tui, and how.
//!
//! - [`tui`]: a person, in the terminal.
//! - [`control`]: an agent, working a running TUI the way a person does.
//! - [`cli`]: an agent or a script, through the subcommands.
//!
//! Each turns what it was asked into use cases (`crate::core`). `control`
//! reads the screen `tui` draws; nothing else here knows another way in.
//! None reaches into `crate::infra`: what a way in needs from outside comes
//! in from the root — the subcommands through `cli::Host`, which
//! `crate::commands` fills.

pub mod cli;
pub mod control;
pub mod tui;
