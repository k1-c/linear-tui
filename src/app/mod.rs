use crate::config::{Config, Theme};
use crate::entity::*;
use crate::grouping::{GroupBy, Preset, Section, group};
use crate::message::Message;
pub use crate::store::{IssueSource, PerSource, Store, TeamContext};
use crate::usecase::Request;
use crate::usecase::{self, Refusal};

mod actions;
mod agents;
mod cursor;
mod frame;
mod input;
mod lists;
mod messages;
mod mouse;
mod navigation;
mod notes;
mod outbox;
mod palette;
mod popups;
mod restore;
mod sidebar;
mod snapshot;
#[cfg(test)]
mod tests;
mod view;

pub use frame::*;
pub use input::*;
pub use lists::*;
pub use navigation::*;
pub use outbox::*;
pub use sidebar::*;
pub use view::*;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// How close to the bottom of the list the cursor must get before the next page
/// is requested.
const PREFETCH_MARGIN: usize = 5;

/// A destination in the sidebar — everything the content pane can be showing.
///
/// Linear's navigation is a tree of places, not a strip of four tabs, and the
/// team a place belongs to is part of the place: `Platform > Cycles` and
/// `Development > Cycles` are different destinations. Carrying the team index
/// here is what lets the sidebar jump between teams without a modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    MyIssues,
    /// The index of saved views.
    Views,
    /// One saved view, by its position in `custom_views`.
    View(usize),
    Team(usize, TeamSection),
    /// A favorite opened in place — a project, cycle, or issue — by its
    /// position in `favorites`. Favorites that point at a view or a team page
    /// use that destination instead, so the sidebar highlights one row.
    Favorite(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamSection {
    Issues,
    Cycles,
    Projects,
    /// Saved views scoped to the team.
    Views,
}

/// What a saved view lists — Linear's Issues / Projects tabs on a Views page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewKind {
    #[default]
    Issues,
    Projects,
}

impl ViewKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Issues => "Issues",
            Self::Projects => "Projects",
        }
    }

    fn of(view: &CustomView) -> Self {
        if view.lists_issues() {
            Self::Issues
        } else {
            Self::Projects
        }
    }
}

/// A clickable chip in a toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chip {
    Preset(Preset),
    ViewKind(ViewKind),
}

impl Nav {
    /// The screen this destination opens.
    pub fn screen(&self) -> Screen {
        match self {
            // A project view opens a project list instead; see `App::screen_for`.
            Self::MyIssues | Self::View(_) | Self::Team(_, TeamSection::Issues) => {
                Screen::IssueList
            }
            Self::Views | Self::Team(_, TeamSection::Views) => Screen::ViewList,
            Self::Team(_, TeamSection::Cycles) => Screen::CycleList,
            Self::Team(_, TeamSection::Projects) => Screen::ProjectList,
            // Depends on what the favorite is; `App::activate` opens it.
            Self::Favorite(_) => Screen::ProjectDetail,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Screen {
    IssueList,
    IssueDetail,
    ProjectList,
    ProjectDetail,
    CycleList,
    CycleDetail,
    /// The index of saved views.
    ViewList,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
    Comment,
    NewIssue,
    /// Typing a note for the agent.
    Note,
}

/// The open popup, and what it acts on.
///
/// The change popups hold the issue they were opened on by id: the cursor
/// can move under an open popup when a page lands, and Enter must change the
/// issue the user picked, not whichever one is under the cursor by then.
#[derive(Debug, Clone, PartialEq)]
pub enum Popup {
    None,
    /// The command palette; its query and cursor are in `ViewState::palette`.
    Palette,
    /// Pick how issue lists are grouped.
    GroupBy,
    TeamSelect,
    /// The filter popup asks for a status, then a priority.
    Filter(FilterKind),
    StatusChange(IssueId),
    PriorityChange(IssueId),
    AssigneeChange(IssueId),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterKind {
    Status,
    Priority,
}

pub struct App {
    /// Everything Linear has told us.
    pub store: Store,
    /// Where the user is.
    pub nav: Navigation,
    /// How the screen is shaped around it.
    pub view: ViewState,
    /// What the last frame drew, for hit-testing and page-sized moves.
    pub frame: FrameState,
    /// Requests and effects waiting for the main loop.
    pub outbox: Outbox,
    pub should_quit: bool,
    /// Whether linear-tui runs inside herdr, which enables the actions that
    /// hand work to its plugin.
    pub herdr: bool,
    /// herdr's agents and the issues they work on, from the plugin.
    pub agents: Vec<crate::entity::AgentLink>,
    /// Notes written for the agent and not yet sent.
    pub notes: Notes,
    /// A snapshot being reopened, while it still has steps to take.
    restore: Option<restore::Restore>,

    // Settings
    pub theme: Theme,
    pub items_per_page: u32,
    default_team: Option<String>,
}

impl App {
    pub fn new(config: &Config) -> Self {
        let mut app = Self {
            store: Store::default(),
            nav: Navigation::default(),
            view: ViewState::new(config),
            frame: FrameState::default(),
            outbox: Outbox::default(),
            should_quit: false,
            herdr: crate::herdr::available(),
            agents: Vec::new(),
            notes: Default::default(),
            restore: None,
            theme: Theme::from_name(config.ui.theme),
            items_per_page: config.ui.items_per_page,
            default_team: config.ui.default_team.clone(),
        };
        // The config loaded, but not as written. The popup goes on any key,
        // and unlike the status line it outlasts the first page arriving.
        if !config.warnings.is_empty() {
            app.set_error(format!("config.toml:\n{}", config.warnings.join("\n")));
        }
        // The sidebar shows the views and favorites whatever page is open,
        // and every page waits on the teams and on who the user is.
        app.request(usecase::team::load());
        app.request(usecase::user::load());
        let views = usecase::view::reload_views(&mut app.store);
        app.request(views);
        app.request(usecase::favorite::load());
        app
    }

    /// Send what a use case asked for, or say why it declined. True when it ran.
    fn run<R: Into<Request>>(&mut self, outcome: Result<R, Refusal>) -> bool {
        match outcome {
            Ok(request) => {
                self.request(request);
                true
            }
            Err(refusal) => {
                self.set_status(refusal.to_string());
                false
            }
        }
    }

    pub fn request(&mut self, req: impl Into<Request>) {
        self.outbox.push(req.into());
    }

    /// Send what a use case asked for, if it asked for anything.
    fn send(&mut self, req: Option<impl Into<Request>>) {
        if let Some(req) = req {
            self.request(req);
        }
    }

    pub fn loading(&self) -> bool {
        self.outbox.inflight > 0
    }

    pub fn team_id(&self) -> Option<TeamId> {
        self.current_team().map(|t| t.id.clone())
    }

    pub fn current_team(&self) -> Option<&Team> {
        self.store.teams.get(self.nav.team)
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn open_help(&mut self) {
        self.view.show_help = true;
        self.view.help_scroll = 0;
    }

    pub fn close_help(&mut self) {
        self.view.show_help = false;
    }

    /// Scroll the help overlay. The renderer clamps the offset to what the
    /// overlay can show, so scrolling past the end does not bank rows that
    /// then have to be scrolled back through.
    pub fn scroll_help(&mut self, delta: i16) {
        self.view.help_scroll = self.view.help_scroll.saturating_add_signed(delta);
    }

    /// Begin Linear's `g …` chord; the next key completes it.
    pub fn start_goto_chord(&mut self) {
        self.view.pending_chord = Some('g');
    }

    pub fn end_chord(&mut self) {
        self.view.pending_chord = None;
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.view.status_message = Some(msg.into());
    }

    pub fn clear_status(&mut self) {
        self.view.status_message = None;
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.view.error_popup = Some(msg.into());
    }

    pub fn dismiss_error(&mut self) {
        self.view.error_popup = None;
    }

    pub fn tick_spinner(&mut self) {
        self.view.spinner_frame = (self.view.spinner_frame + 1) % SPINNER_FRAMES.len();
    }

    pub fn spinner_symbol(&self) -> &'static str {
        SPINNER_FRAMES[self.view.spinner_frame]
    }
}
