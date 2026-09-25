//! Workspaces: the Linear workspaces the user is signed in to, and moving
//! between them.
//!
//! [`signed_in`] lists the ones to offer; [`open_switcher`] and [`pick`]
//! are the switcher; [`leave`] and [`view_on_return`] bring each workspace
//! back on the page it was left on. Signing in and renewing a token are
//! `crate::infra::linear::auth`'s; ending one session and starting the next
//! is the main loop's.

use std::collections::HashMap;

use super::Refusal;
use crate::core::entity::snapshot::ViewSnapshot;
use crate::core::entity::{Organization, OrganizationId, Workspace};

/// The views workspaces were left on during this run, by workspace.
pub type LeftViews = HashMap<OrganizationId, ViewSnapshot>;

/// **Offer the signed-in workspaces**, one per sign-in, marking the one in
/// use.
///
/// A sign-in Linear has not yet said the workspace of is left out: there is
/// nothing to show for it until the next launch names it.
pub fn signed_in(
    sign_ins: impl IntoIterator<Item = (Option<Organization>, bool)>,
) -> Vec<Workspace> {
    sign_ins
        .into_iter()
        .filter_map(|(organization, current)| {
            Some(Workspace {
                organization: organization?,
                current,
            })
        })
        .collect()
}

/// **Switch workspace** (the palette's "Switch workspace…"): pick another
/// signed-in workspace. Answers the row the switcher starts on: the
/// workspace in use.
///
/// With only one signed in there is nothing to pick, so it is refused with
/// how to add another.
pub fn open_switcher(workspaces: &[Workspace]) -> Result<usize, Refusal> {
    if workspaces.len() < 2 {
        return Err(Refusal::OnlyOneWorkspace);
    }
    Ok(workspaces.iter().position(|w| w.current).unwrap_or(0))
}

/// **Pick a workspace** in the switcher: linear-tui moves there, on the
/// same screen, with nothing carried over from the one it leaves.
///
/// Picking the workspace in use changes nothing.
pub fn pick(workspace: &Workspace) -> Option<OrganizationId> {
    (!workspace.current).then(|| workspace.organization.id.clone())
}

/// **Leave a workspace**: the page it was left on is kept for this run, to
/// come back to. Leaving with nothing on screen keeps nothing.
pub fn leave(left: &mut LeftViews, workspace: OrganizationId, view: Option<ViewSnapshot>) {
    if let Some(view) = view {
        left.insert(workspace, view);
    }
}

/// **Come back to a workspace**: it opens on the page it was left on this
/// run, or else on the one last recorded there (`recorded`, see
/// `instance::to_reopen`).
///
/// A directory config.toml pins to a team reopens no view, here as on
/// launch. A view is come back to once: leaving again records it afresh.
pub fn view_on_return(
    left: &mut LeftViews,
    workspace: &OrganizationId,
    pinned: bool,
    recorded: impl FnOnce() -> Option<ViewSnapshot>,
) -> Option<ViewSnapshot> {
    if pinned {
        return None;
    }
    left.remove(workspace).or_else(recorded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn organization(id: &str) -> Organization {
        Organization {
            id: id.into(),
            name: id.to_uppercase(),
            url_key: id.into(),
        }
    }

    fn workspace(id: &str, current: bool) -> Workspace {
        Workspace {
            organization: organization(id),
            current,
        }
    }

    /// A view recorded at `hour`, to tell views apart.
    fn view(hour: u32) -> ViewSnapshot {
        serde_json::from_str(&format!(
            r#"{{"version":1,"workspace":"/repo","cwd":"/repo","pid":1,
                "updated_at":"2026-09-25T{hour:02}:00:00Z","destination":{{"kind":"my_issues"}},
                "screen":"issue_list","group_by":"status"}}"#
        ))
        .unwrap()
    }

    /// Every named sign-in is offered, the one in use marked; one Linear
    /// has not named yet is left out.
    #[test]
    fn named_sign_ins_are_offered_and_unnamed_ones_left_out() {
        let offered = signed_in([
            (Some(organization("acme")), false),
            (None, false),
            (Some(organization("globex")), true),
        ]);
        assert_eq!(
            offered,
            [workspace("acme", false), workspace("globex", true)]
        );
    }

    /// The switcher starts on the workspace in use.
    #[test]
    fn the_switcher_starts_on_the_workspace_in_use() {
        let workspaces = [workspace("acme", false), workspace("globex", true)];
        assert_eq!(open_switcher(&workspaces), Ok(1));
    }

    /// With one workspace signed in, switching is refused with how to add
    /// another.
    #[test]
    fn with_one_workspace_switching_says_how_to_add_another() {
        let refusal = open_switcher(&[workspace("acme", true)]).unwrap_err();
        assert_eq!(refusal, Refusal::OnlyOneWorkspace);
        assert!(refusal.to_string().contains("linear-tui auth login"));
    }

    /// Picking another workspace moves there; picking the one in use does
    /// nothing.
    #[test]
    fn picking_another_workspace_moves_there_and_the_current_one_does_nothing() {
        assert_eq!(pick(&workspace("acme", false)), Some("acme".into()));
        assert_eq!(pick(&workspace("globex", true)), None);
    }

    /// Leaving a workspace keeps its page; leaving with nothing on screen
    /// keeps nothing.
    #[test]
    fn leaving_keeps_the_page_and_nothing_when_there_was_none() {
        let mut left = LeftViews::new();
        leave(&mut left, "acme".into(), Some(view(9)));
        leave(&mut left, "globex".into(), None);
        assert_eq!(left.get(&"acme".into()), Some(&view(9)));
        assert!(!left.contains_key(&"globex".into()));
    }

    /// Coming back opens the page left this run, over the one recorded,
    /// and only once.
    #[test]
    fn coming_back_opens_the_page_left_this_run_once() {
        let mut left = LeftViews::new();
        leave(&mut left, "acme".into(), Some(view(9)));
        let back = view_on_return(&mut left, &"acme".into(), false, || Some(view(8)));
        assert_eq!(back, Some(view(9)));
        let again = view_on_return(&mut left, &"acme".into(), false, || Some(view(8)));
        assert_eq!(again, Some(view(8)));
    }

    /// A workspace not left this run opens on the view recorded there.
    #[test]
    fn a_workspace_not_left_this_run_opens_on_its_recorded_view() {
        let mut left = LeftViews::new();
        let back = view_on_return(&mut left, &"acme".into(), false, || Some(view(8)));
        assert_eq!(back, Some(view(8)));
    }

    /// A pinned directory reopens no view, even one left this run.
    #[test]
    fn a_pinned_directory_reopens_no_view_on_return() {
        let mut left = LeftViews::new();
        leave(&mut left, "acme".into(), Some(view(9)));
        let back = view_on_return(&mut left, &"acme".into(), true, || Some(view(8)));
        assert_eq!(back, None);
    }
}
