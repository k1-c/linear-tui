//! What linear-tui is about: Linear's issues, teams, projects, cycles, saved
//! views, and favorites, and the notes and agents it adds beside them.
//!
//! The innermost layer. It depends on nothing else in the crate, and on
//! nothing that draws, stores, or talks to the network: the Linear client
//! decodes into these types, the use cases change them, and the TUI shows
//! them. How a value looks on screen is `crate::look`'s business.
//!
//! The types keep their serde derives, and with them the field names
//! Linear's GraphQL API uses, so decoding needs no second set of types.

mod agent;
mod cycle;
mod favorite;
pub mod ids;
mod instance;
mod issue;
mod note;
mod page;
mod preset;
mod project;
pub mod snapshot;
mod team;
mod view;

pub use agent::*;
pub use cycle::*;
pub use favorite::*;
pub use ids::*;
pub use instance::*;
pub use issue::*;
pub use note::*;
pub use page::*;
pub use preset::*;
pub use project::*;
pub use team::*;
pub use view::*;
