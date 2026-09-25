//! The interface: what the user sees and does, turned into use cases.
//!
//! - Controllers take input: [`keys`] (the binding table), [`palette`] (the
//!   `Ctrl+K` command palette, with [`fuzzy`] its matcher), and [`event`]
//!   (terminal events, routed).
//! - [`app`] holds the session: where the user is, how the screen is
//!   shaped, and every state transition — resolving the issue under the
//!   cursor, then calling the use case.
//! - Presenters draw: [`ui`] renders a frame from `app`, with [`look`]
//!   (glyphs and colours) and [`grouping`] (how a list is grouped).
//! - [`message`] is how the adapters' answers come back to `app`.
//!
//! It depends on `usecase`, `store`, and `entity`, never on `adapter`.

pub mod app;
pub mod event;
pub mod fuzzy;
pub mod grouping;
pub mod keys;
pub mod look;
pub mod message;
pub mod palette;
pub mod ui;
