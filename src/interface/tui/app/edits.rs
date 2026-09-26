//! Editing the issue itself and its thread: renaming it (`r`), rewriting
//! its description (`e`), and replying to, editing, or deleting a comment.
//! The rules are `usecase::issue`'s; this resolves which issue and which
//! comment, and where the text is typed.

use super::*;
use crate::core::entity::{Comment, CommentId};
use crate::core::usecase::issue::Edit;

/// What a comment picker does with the comment picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentAction {
    Reply,
    Edit,
    Delete,
}

impl CommentAction {
    /// How the picker is titled.
    pub fn title(self) -> &'static str {
        match self {
            Self::Reply => "Reply to comment",
            Self::Edit => "Edit comment",
            Self::Delete => "Delete comment",
        }
    }
}

/// What the comment field posts.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum CommentTarget {
    /// A new comment on the issue.
    #[default]
    New,
    /// A reply in this comment's thread.
    Reply(CommentId),
    /// A new body for this comment.
    Edit(CommentId),
}

/// A description to write in `$EDITOR`, which the main loop opens.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorJob {
    pub issue_id: IssueId,
    pub text: String,
}

impl App {
    /// `r`: rename the issue under the cursor.
    pub fn start_rename(&mut self) {
        let Some(issue) = self.focused_issue() else {
            return;
        };
        let (id, title) = (issue.id.clone(), issue.title.clone());
        self.view.editing = Some(id);
        self.view.title = Input::with(&title);
        self.view.input_mode = InputMode::Title;
    }

    pub fn submit_rename(&mut self) {
        let title = std::mem::take(&mut self.view.title).value;
        self.view.input_mode = InputMode::Normal;
        let Some(issue_id) = self.view.editing.take() else {
            return;
        };
        if self
            .store
            .issue(&issue_id)
            .is_some_and(|i| i.title == title)
        {
            return;
        }
        let edit = Edit {
            title: Some(title),
            ..Edit::default()
        };
        let outcome = usecase::issue::update(&mut self.store, &issue_id, edit);
        self.run(outcome);
    }

    /// `e`: rewrite the description of the issue under the cursor — in
    /// `$EDITOR` when linear-tui has a terminal to lend it, otherwise in a
    /// field of its own (a headless instance, driven by an agent).
    pub fn start_description(&mut self) {
        let Some(issue) = self.focused_issue() else {
            return;
        };
        let id = issue.id.clone();
        let text = issue.description.clone().unwrap_or_default();
        if self.external_editor {
            self.set_status("Editing the description in your editor\u{2026}");
            self.outbox.editor = Some(EditorJob { issue_id: id, text });
            return;
        }
        self.view.editing = Some(id);
        self.view.description = Input::with(&text);
        self.view.input_mode = InputMode::Description;
    }

    pub fn submit_description(&mut self) {
        let text = std::mem::take(&mut self.view.description).value;
        self.view.input_mode = InputMode::Normal;
        if let Some(issue_id) = self.view.editing.take() {
            self.description_edited(&issue_id, Some(text));
        }
    }

    /// The description came back from the editor or the field: `None` when
    /// the editor was quit without saving. An unchanged one is not sent.
    pub fn description_edited(&mut self, issue_id: &IssueId, text: Option<String>) {
        let Some(text) = text else {
            return self.set_status("Description left as it was");
        };
        let current = self
            .store
            .issue(issue_id)
            .and_then(|i| i.description.clone())
            .unwrap_or_default();
        if current.trim_end() == text.trim_end() {
            return self.set_status("Description unchanged");
        }
        let edit = Edit {
            description: Some(text),
            ..Edit::default()
        };
        let outcome = usecase::issue::update(&mut self.store, issue_id, edit);
        if self.run(outcome) {
            self.set_status("Saving the description\u{2026}");
        }
    }

    /// Esc in the title or description field.
    pub fn cancel_edit(&mut self) {
        self.view.input_mode = InputMode::Normal;
        self.view.editing = None;
        self.view.title.clear();
        self.view.description.clear();
    }

    /// Open a picker of the open issue's comments: all of them to reply to,
    /// the user's own to edit or delete.
    pub fn open_comment_pick(&mut self, action: CommentAction) {
        if self.nav.screen != Screen::IssueDetail {
            return;
        }
        let Some(issue) = self.store.current_issue.as_ref() else {
            return;
        };
        if issue.comments.is_none() {
            return self.set_status("The thread is still loading");
        }
        if self.pickable_comments(action).is_empty() {
            return self.set_status(match action {
                CommentAction::Reply => "No comments to reply to",
                CommentAction::Edit | CommentAction::Delete => "No comments of yours here",
            });
        }
        self.show_comment_pick(action);
    }

    fn show_comment_pick(&mut self, action: CommentAction) {
        self.view.popup = Popup::CommentPick(action);
        self.view.popup_index = 0;
        self.view.popup_query.clear();
    }

    pub fn open_reply_pick(&mut self) {
        self.open_comment_pick(CommentAction::Reply);
    }

    pub fn open_edit_comment_pick(&mut self) {
        self.open_comment_pick(CommentAction::Edit);
    }

    pub fn open_delete_comment_pick(&mut self) {
        self.open_comment_pick(CommentAction::Delete);
    }

    /// The comments a picker offers, oldest first: every one to reply to,
    /// and to edit or delete only the user's own. Before Linear has said who
    /// the user is, every one — Linear refuses the others.
    pub fn pickable_comments(&self, action: CommentAction) -> Vec<&Comment> {
        let Some(comments) = self
            .store
            .current_issue
            .as_ref()
            .and_then(|i| i.comments.as_ref())
        else {
            return Vec::new();
        };
        let mine = |c: &&Comment| match (&self.store.viewer_id, &c.user) {
            (Some(me), Some(author)) => &author.id == me,
            _ => true,
        };
        let mut picked: Vec<&Comment> = comments
            .nodes
            .iter()
            .filter(|c| action == CommentAction::Reply || mine(c))
            .collect();
        picked.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        picked
    }

    /// Carry out the comment picked.
    pub fn apply_comment_pick(&mut self, action: CommentAction, index: usize) {
        let Some(comment) = self.pickable_comments(action).get(index).copied() else {
            return;
        };
        let (id, body) = (comment.id.clone(), comment.body.clone());
        let Some(issue_id) = self.store.current_issue.as_ref().map(|i| i.id.clone()) else {
            return;
        };
        self.close_popup();
        match action {
            CommentAction::Reply => {
                self.view.comment = Input::default();
                self.view.comment_target = CommentTarget::Reply(id);
                self.view.input_mode = InputMode::Comment;
            }
            CommentAction::Edit => {
                self.view.comment = Input::with(&body);
                self.view.comment_target = CommentTarget::Edit(id);
                self.view.input_mode = InputMode::Comment;
            }
            CommentAction::Delete => {
                let outcome = usecase::issue::delete_comment(&mut self.store, &issue_id, &id);
                if self.run(outcome) {
                    self.set_status("Deleting the comment\u{2026}");
                }
            }
        }
    }
}
