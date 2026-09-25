//! Cycles: a team's time-boxed iterations.
//!
//! [`open_team_cycles`] lists them; [`next_page`], [`reload`], and
//! [`take_page`] follow the list as pages land. A cycle's issues are an
//! issue list: `issue::open_cycle_issues`.

use super::Open;
use crate::core::entity::{Cycle, Page, TeamId};
use crate::core::store::{List, ListOf, Store};

/// What the cycle use cases ask of Linear.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// A page of a team's cycles.
    TeamCycles {
        team_id: TeamId,
        after: Option<String>,
    },
}

impl Request {
    /// The page cursor this request continues from, if it asks for a next
    /// page.
    pub fn cursor(&self) -> Option<&str> {
        match self {
            Self::TeamCycles { after, .. } => after.as_deref(),
        }
    }
}

/// **Browse a team's cycles.** A list already holding them is not fetched
/// again; another team's cycles are dropped first.
pub fn open_team_cycles(store: &mut Store, team_id: TeamId) -> Open {
    store
        .open_list(List::Cycles, ListOf::TeamCycles(team_id))
        .into()
}

/// **Scroll on to the next page** of cycles, asked for once.
pub fn next_page(store: &mut Store) -> Option<super::Request> {
    store.next_page(List::Cycles).map(super::Request::page)
}

/// **Refresh the cycles**: their first page again.
pub fn reload(store: &mut Store) -> Option<super::Request> {
    store.reload_list(List::Cycles).map(super::Request::page)
}

/// **A page of cycles lands**, and is taken only while the list still
/// belongs to the team it was fetched for. Returns whether it was taken.
pub fn take_page(store: &mut Store, of: &ListOf, page: Page<Cycle>) -> bool {
    store.cycles.accept_for(of, page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::entity::PageInfo;

    fn cycles(more: bool) -> Page<Cycle> {
        let cycle: Cycle = serde_json::from_str(r#"{"id":"c","number":1}"#).unwrap();
        let info = PageInfo {
            has_next_page: more,
            end_cursor: more.then(|| "c1".into()),
        };
        Page::new(vec![cycle], info, false)
    }

    /// A team's cycles are asked for once, then shown from what is held.
    #[test]
    fn a_teams_cycles_are_fetched_once() {
        let mut store = Store::default();
        assert_eq!(
            open_team_cycles(&mut store, TeamId::from("t")).request(),
            Some(crate::core::usecase::Request::Cycle(Request::TeamCycles {
                team_id: "t".into(),
                after: None
            }))
        );
        take_page(
            &mut store,
            &ListOf::TeamCycles(TeamId::from("t")),
            cycles(false),
        );
        assert_eq!(
            open_team_cycles(&mut store, TeamId::from("t")),
            Open::Cached
        );
    }

    /// Cycles fetched for a team the user has left are dropped.
    #[test]
    fn cycles_for_a_team_left_behind_are_dropped() {
        let mut store = Store::default();
        open_team_cycles(&mut store, TeamId::from("u"));
        assert!(!take_page(
            &mut store,
            &ListOf::TeamCycles(TeamId::from("t")),
            cycles(false)
        ));
    }

    /// The next page is asked for once; a refresh asks for the first again.
    #[test]
    fn the_next_page_is_asked_for_once() {
        let mut store = Store::default();
        open_team_cycles(&mut store, TeamId::from("t"));
        take_page(
            &mut store,
            &ListOf::TeamCycles(TeamId::from("t")),
            cycles(true),
        );
        assert!(next_page(&mut store).is_some());
        assert_eq!(next_page(&mut store), None);
        assert_eq!(
            reload(&mut store),
            Some(crate::core::usecase::Request::Cycle(Request::TeamCycles {
                team_id: "t".into(),
                after: None
            }))
        );
    }
}
