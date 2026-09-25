//! The terminal UI: a person reading and working Linear.
//!
//! - Controllers take input: [`keys`] (the binding table), [`palette`] (the
//!   `Ctrl+K` command palette, with [`fuzzy`] its matcher), and [`event`]
//!   (terminal events, routed).
//! - [`app`] holds the session: where the user is, how the screen is
//!   shaped, and every state transition — resolving the issue under the
//!   cursor, then calling the use case.
//! - Presenters draw: [`ui`] renders a frame from `app`, with [`look`]
//!   (glyphs and colours) and [`grouping`] (how a list is grouped).

pub mod app;
pub mod event;
pub mod fuzzy;
pub mod grouping;
pub mod keys;
pub mod look;
pub mod palette;
pub mod ui;
