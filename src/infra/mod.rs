//! The systems outside linear-tui that it calls on.
//!
//! - [`linear`]: Linear's GraphQL API, and signing in to it.
//! - [`herdr`]: the herdr plugin, for the actions that exist only inside it.
//! - [`disk`]: what linear-tui keeps in files: the view snapshots.
//! - [`dispatch`]: carrying out a use case's `Request` through them.
//!
//! It depends on `crate::core`, never on `crate::interface`.

pub mod disk;
pub mod dispatch;
pub mod herdr;
pub mod linear;
