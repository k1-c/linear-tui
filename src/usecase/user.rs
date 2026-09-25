//! The signed-in user: who "me" is.
//!
//! [`load`] asks Linear, and [`take_viewer`] keeps the answer. "Assign to
//! me" and "My issues" wait on it.

use crate::entity::{Organization, UserId};
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

/// **Linear says who I am**, and which workspace the credentials act in.
/// The user's name comes with the teams they belong to; the id is enough
/// for everything else.
pub fn take_viewer(store: &mut Store, id: UserId, organization: Option<Organization>) {
    store.viewer_id = Some(id);
    store.organization = organization;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Once Linear answers, the user is known by id, and so is the
    /// workspace.
    #[test]
    fn the_answer_says_who_i_am_and_where() {
        let mut store = Store::default();
        assert_eq!(load(), Request::Viewer);
        let acme: Organization =
            serde_json::from_str(r#"{"id":"o1","name":"Acme","urlKey":"acme"}"#).unwrap();
        take_viewer(&mut store, UserId::from("me"), Some(acme));
        assert_eq!(store.viewer_id, Some(UserId::from("me")));
        assert_eq!(store.organization.unwrap().url_key, "acme");
    }
}
