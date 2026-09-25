//! The signed-in user: who "me" is.
//!
//! [`load`] asks Linear, and [`take_viewer`] keeps the answer. "Assign to
//! me" and "My issues" wait on it.

use crate::entity::UserId;
use crate::store::Store;

/// What the user use cases ask of Linear.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Who the signed-in user is.
    Viewer,
}

/// **Know who I am.** Asked for as linear-tui starts.
pub fn load() -> Request {
    Request::Viewer
}

/// **Linear says who I am.** The user's name comes with the teams they
/// belong to; the id is enough for everything else.
pub fn take_viewer(store: &mut Store, id: UserId) {
    store.viewer_id = Some(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Once Linear answers, the user is known by id.
    #[test]
    fn the_answer_says_who_i_am() {
        let mut store = Store::default();
        assert_eq!(load(), Request::Viewer);
        take_viewer(&mut store, UserId::from("me"));
        assert_eq!(store.viewer_id, Some(UserId::from("me")));
    }
}
