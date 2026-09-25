//! An entry in the user's Favorites.

use serde::Deserialize;

use super::cycle::Cycle;
use super::ids::*;
use super::issue::IssueRef;
use super::page::Ref;
use super::project::Project;

/// One entry in the user's Favorites — Linear's own sidebar, and the way most
/// people actually get around a workspace.
///
/// A favorite can point at nearly anything (about twenty kinds); `type` says
/// which, and exactly one of the object fields is set to match. `title`,
/// `color`, and `url` are resolved by Linear, so every kind can be shown — and
/// opened in the browser — even when this client has no screen for it.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Favorite {
    pub id: FavoriteId,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default, rename = "sortOrder")]
    pub sort_order: f64,
    /// The folder this favorite sits in, when it is in one.
    #[serde(default)]
    pub parent: Option<Ref<FavoriteId>>,
    #[serde(default, rename = "folderName")]
    pub folder_name: Option<String>,
    /// For a built-in page ("issues", "projects", "cycles", …).
    #[serde(default, rename = "predefinedViewType")]
    pub predefined_view_type: Option<String>,
    #[serde(default, rename = "predefinedViewTeam")]
    pub predefined_view_team: Option<Ref<TeamId>>,
    #[serde(default, rename = "customView")]
    pub custom_view: Option<Ref<CustomViewId>>,
    #[serde(default)]
    pub issue: Option<IssueRef>,
    #[serde(default)]
    pub project: Option<Project>,
    #[serde(default)]
    pub cycle: Option<Cycle>,
}

impl Favorite {
    pub fn is_folder(&self) -> bool {
        self.kind == "folder"
    }

    /// What the sidebar calls it.
    pub fn label(&self) -> String {
        self.title
            .clone()
            .or_else(|| self.folder_name.clone())
            .or_else(|| self.project.as_ref().map(|p| p.name.clone()))
            .or_else(|| self.issue.as_ref().map(|i| i.title.clone()))
            .unwrap_or_else(|| self.kind.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fav(json: &str) -> Favorite {
        serde_json::from_str(json).unwrap()
    }

    /// The sidebar calls a favorite by its title; without one, by its
    /// folder's name, then the project's or issue's, then its kind.
    #[test]
    fn a_favorite_is_called_by_its_title_then_what_it_holds() {
        assert_eq!(
            fav(r#"{"id":"f","type":"project","title":"T"}"#).label(),
            "T"
        );
        assert_eq!(
            fav(r#"{"id":"f","type":"folder","folderName":"Ops"}"#).label(),
            "Ops"
        );
        assert_eq!(
            fav(r#"{"id":"f","type":"project","project":{"id":"p","name":"P"}}"#).label(),
            "P"
        );
        assert_eq!(
            fav(
                r#"{"id":"f","type":"issue","issue":{"id":"i","identifier":"X-1","title":"Crash"}}"#
            )
            .label(),
            "Crash"
        );
        assert_eq!(fav(r#"{"id":"f","type":"document"}"#).label(), "document");
    }

    /// Only a folder folds.
    #[test]
    fn only_a_folder_folds() {
        assert!(fav(r#"{"id":"f","type":"folder"}"#).is_folder());
        assert!(!fav(r#"{"id":"f","type":"project"}"#).is_folder());
    }
}
