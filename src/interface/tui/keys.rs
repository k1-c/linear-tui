//! Keybindings (Controller).
//!
//! Every documented binding is one row of [`BINDINGS`]: the keys it answers
//! to, the contexts it applies in, what it does, and how the help overlay and
//! the status bar describe it. Dispatch, hints, and help all read that table,
//! so adding a binding means adding a row.
//!
//! The command palette is the fourth reader: a row with a [`Command`] is
//! listed there, with the keys shown next to it, so the palette cannot offer
//! an action the keyboard lacks or advertise a key that does something else.
//!
//! Only the overlays that document their own keys in their frame — the error
//! popup, the help overlay, the pick-one popups, and the palette — keep a
//! hand-written handler.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::core::entity::Priority;
use crate::interface::tui::app::{
    App, FormField, Input, InputMode, Nav, Popup, Screen, TeamSection,
};
use crate::interface::tui::grouping::Preset;

/// Where a key is pressed. A binding lists the contexts it applies in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ctx {
    IssueList,
    IssueDetail,
    ProjectList,
    ProjectDetail,
    CycleList,
    CycleDetail,
    ViewList,
    /// The navigation sidebar has focus.
    Sidebar,
    /// A `g` is pending; the next key finishes the chord.
    GoTo,
    Search,
    Comment,
    /// Typing a note for the agent.
    Note,
    IssueTitle,
    IssueDescription,
    IssuePriority,
}

impl From<Screen> for Ctx {
    fn from(screen: Screen) -> Self {
        match screen {
            Screen::IssueList => Self::IssueList,
            Screen::IssueDetail => Self::IssueDetail,
            Screen::ProjectList => Self::ProjectList,
            Screen::ProjectDetail => Self::ProjectDetail,
            Screen::CycleList => Self::CycleList,
            Screen::CycleDetail => Self::CycleDetail,
            Screen::ViewList => Self::ViewList,
        }
    }
}

use Ctx::*;

/// Every screen, with the sidebar unfocused.
const SCREENS: &[Ctx] = &[
    IssueList,
    IssueDetail,
    ProjectList,
    ProjectDetail,
    CycleList,
    CycleDetail,
    ViewList,
];
/// Every screen, and the sidebar.
const NORMAL: &[Ctx] = &[
    IssueList,
    IssueDetail,
    ProjectList,
    ProjectDetail,
    CycleList,
    CycleDetail,
    ViewList,
    Sidebar,
];
/// Screens with a cursor over rows.
const LISTS: &[Ctx] = &[
    IssueList,
    ProjectList,
    ProjectDetail,
    CycleList,
    CycleDetail,
    ViewList,
];
/// Screens that scroll: the lists and the issue view.
const SCROLLING: &[Ctx] = &[
    IssueList,
    IssueDetail,
    ProjectList,
    ProjectDetail,
    CycleList,
    CycleDetail,
    ViewList,
];
/// Screens listing issues.
const ISSUE_LISTS: &[Ctx] = &[IssueList, ProjectDetail, CycleDetail];
/// Screens with an issue under the cursor.
const ISSUE_SCREENS: &[Ctx] = &[IssueList, IssueDetail, ProjectDetail, CycleDetail];
/// Top-level destinations, where a number jumps to another one.
const INDEXES: &[Ctx] = &[IssueList, ProjectList, CycleList, ViewList];
/// Where `q` quits rather than steps back.
const TOP_LEVEL: &[Ctx] = &[IssueList, ProjectList, CycleList, ViewList, Sidebar];
/// Where `q` steps back.
const NESTED: &[Ctx] = &[IssueDetail, ProjectDetail, CycleDetail];
/// Team pages, where the team can be switched.
const TEAM_PAGES: &[Ctx] = &[IssueList, ProjectList, CycleList, Sidebar];
/// Text fields.
const TEXT: &[Ctx] = &[Search, Comment, Note, IssueTitle, IssueDescription];
/// Multi-line text fields.
const MULTILINE: &[Ctx] = &[Comment, Note, IssueDescription];
/// The new-issue form, whichever field has focus.
const FORM: &[Ctx] = &[IssueTitle, IssueDescription, IssuePriority];
/// Anything submitted with Ctrl+Enter.
const SUBMITTABLE: &[Ctx] = &[Comment, Note, IssueTitle, IssueDescription, IssuePriority];
/// Every input mode that Esc abandons.
const EDITING: &[Ctx] = &[
    Search,
    Comment,
    Note,
    IssueTitle,
    IssueDescription,
    IssuePriority,
];

/// Which modifiers a [`Key`] requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mods {
    /// Anything goes.
    Any,
    /// No Ctrl; Shift and Alt are fine.
    NoCtrl,
    /// Neither Ctrl nor Alt.
    Bare,
    /// Ctrl, with or without Shift.
    Ctrl,
    /// Ctrl without Shift.
    CtrlNoShift,
    /// Ctrl and Shift.
    CtrlShift,
    /// Ctrl or Alt.
    CtrlOrAlt,
}

/// One key a binding answers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    code: KeyCode,
    mods: Mods,
}

impl Key {
    fn matches(&self, key: &KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        key.code == self.code
            && match self.mods {
                Mods::Any => true,
                Mods::NoCtrl => !ctrl,
                Mods::Bare => !ctrl && !alt,
                Mods::Ctrl => ctrl,
                Mods::CtrlNoShift => ctrl && !shift,
                Mods::CtrlShift => ctrl && shift,
                Mods::CtrlOrAlt => ctrl || alt,
            }
    }
}

/// A character typed without Ctrl.
const fn plain(c: char) -> Key {
    Key {
        code: KeyCode::Char(c),
        mods: Mods::NoCtrl,
    }
}

const fn ctrl(c: char) -> Key {
    Key {
        code: KeyCode::Char(c),
        mods: Mods::Ctrl,
    }
}

const fn ctrl_no_shift(c: char) -> Key {
    Key {
        code: KeyCode::Char(c),
        mods: Mods::CtrlNoShift,
    }
}

const fn ctrl_shift(c: char) -> Key {
    Key {
        code: KeyCode::Char(c),
        mods: Mods::CtrlShift,
    }
}

/// A non-character key, whatever the modifiers.
const fn code(code: KeyCode) -> Key {
    Key {
        code,
        mods: Mods::Any,
    }
}

/// A key without Ctrl or Alt.
const fn bare(code: KeyCode) -> Key {
    Key {
        code,
        mods: Mods::Bare,
    }
}

const fn ctrl_or_alt(code: KeyCode) -> Key {
    Key {
        code,
        mods: Mods::CtrlOrAlt,
    }
}

/// A heading of the help overlay, in the order they are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Navigation,
    Sidebar,
    GoTo,
    ListDisplay,
    IssueActions,
    CopyOpen,
    SearchFilter,
    /// Handing work to a coding agent.
    Agent,
    Mouse,
    Other,
    Scrolling,
    Editing,
}

impl Section {
    pub const ALL: [Section; 12] = [
        Self::Navigation,
        Self::Sidebar,
        Self::GoTo,
        Self::ListDisplay,
        Self::IssueActions,
        Self::CopyOpen,
        Self::SearchFilter,
        Self::Agent,
        Self::Mouse,
        Self::Other,
        Self::Scrolling,
        Self::Editing,
    ];

    /// Where the palette lists the section: acting on the issue at hand
    /// first, as Linear's command menu does, and moving around last.
    fn palette_rank(self) -> u8 {
        match self {
            Self::IssueActions => 0,
            Self::CopyOpen => 1,
            Self::SearchFilter => 2,
            Self::Agent => 2,
            Self::ListDisplay => 3,
            Self::GoTo => 4,
            Self::Navigation => 5,
            Self::Sidebar => 6,
            Self::Other => 7,
            Self::Mouse | Self::Scrolling | Self::Editing => 8,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Navigation => "Navigation",
            Self::Sidebar => "Sidebar",
            Self::GoTo => "Go to",
            Self::ListDisplay => "List display",
            Self::IssueActions => "Issue actions",
            Self::CopyOpen => "Copy & open",
            Self::SearchFilter => "Search & filter",
            Self::Agent => "Agent",
            Self::Mouse => "Mouse",
            Self::Other => "Other",
            Self::Scrolling => "Scrolling",
            Self::Editing => "Editing",
        }
    }
}

/// A row of the help overlay.
#[derive(Debug, Clone, Copy)]
pub struct Help {
    pub section: Section,
    /// The keys as written, e.g. `j/k`. One row may cover a pair of bindings.
    pub keys: &'static str,
    pub text: &'static str,
}

/// A status-bar hint.
#[derive(Debug, Clone, Copy)]
pub struct Hint {
    /// Position on the status bar, lowest first; ties keep table order.
    pub rank: u8,
    pub keys: &'static str,
    pub what: &'static str,
    /// Where the hint shows — some or all of the binding's contexts.
    pub on: &'static [Ctx],
}

/// A command palette entry.
#[derive(Debug, Clone, Copy)]
pub struct Command {
    pub section: Section,
    pub title: &'static str,
    /// Other words it is found by.
    pub keywords: &'static [&'static str],
    /// Where the palette offers it — the binding's own contexts unless it
    /// says otherwise, as for the second key of a `g …` chord.
    pub on: &'static [Ctx],
}

/// A keybinding: what it answers to, where, what it does, and how it is shown.
#[derive(Clone, Copy)]
pub struct Binding {
    /// Every key that triggers it — a kitty-protocol original sits next to
    /// its plain-key alias, so the pair cannot drift apart. Empty for a row
    /// that documents input handled elsewhere (the mouse).
    pub keys: &'static [Key],
    pub context: &'static [Ctx],
    pub action: fn(&mut App),
    pub help: Option<Help>,
    pub hint: Option<Hint>,
    pub command: Option<Command>,
    /// Only inside herdr: elsewhere it neither answers nor is listed.
    pub herdr_only: bool,
}

impl Binding {
    const fn help(mut self, section: Section, keys: &'static str, text: &'static str) -> Self {
        self.help = Some(Help {
            section,
            keys,
            text,
        });
        self
    }

    /// A hint on every context the binding applies in.
    const fn hint(self, rank: u8, keys: &'static str, what: &'static str) -> Self {
        let on = self.context;
        self.hint_on(on, rank, keys, what)
    }

    /// A hint on some of the binding's contexts.
    const fn hint_on(
        mut self,
        on: &'static [Ctx],
        rank: u8,
        keys: &'static str,
        what: &'static str,
    ) -> Self {
        self.hint = Some(Hint {
            rank,
            keys,
            what,
            on,
        });
        self
    }

    /// A palette entry, under the binding's help section.
    const fn command(mut self, title: &'static str, keywords: &'static [&'static str]) -> Self {
        let section = match self.help {
            Some(help) => help.section,
            None => Section::Other,
        };
        self.command = Some(Command {
            section,
            title,
            keywords,
            on: self.context,
        });
        self
    }

    /// Offer the palette entry in `on` rather than the binding's contexts.
    const fn on(mut self, on: &'static [Ctx]) -> Self {
        if let Some(command) = &mut self.command {
            command.on = on;
        }
        self
    }

    /// File the palette entry under `section`.
    const fn in_section(mut self, section: Section) -> Self {
        if let Some(command) = &mut self.command {
            command.section = section;
        }
        self
    }

    /// Mark a binding that only makes sense inside herdr.
    const fn herdr_only(mut self) -> Self {
        self.herdr_only = true;
        self
    }

    /// Whether it exists: a herdr-only binding exists only inside herdr
    /// (`App::herdr`).
    pub(crate) fn shown(&self, herdr: bool) -> bool {
        !self.herdr_only || herdr
    }

    fn applies_in(&self, ctx: Ctx) -> bool {
        self.context.contains(&ctx)
    }

    fn matches(&self, key: &KeyEvent) -> bool {
        self.keys.iter().any(|k| k.matches(key))
    }
}

const fn bind(keys: &'static [Key], context: &'static [Ctx], action: fn(&mut App)) -> Binding {
    Binding {
        keys,
        context,
        action,
        help: None,
        hint: None,
        command: None,
        herdr_only: false,
    }
}

/// An action reached only from the command palette, in `context`.
const fn palette_only(context: &'static [Ctx], action: fn(&mut App)) -> Binding {
    bind(&[], context, action)
}

/// A help row for input dispatched outside this table.
const fn documented(section: Section, keys: &'static str, text: &'static str) -> Binding {
    bind(&[], &[], |_| {}).help(section, keys, text)
}

/// Every binding. The help overlay lists rows by [`Section`], in table order
/// within each; the status bar orders hints by [`Hint::rank`].
///
/// Shortcuts mirror Linear's own (see AGENTS.md). Where a terminal cannot
/// deliver Linear's key, the original is listed for the kitty keyboard
/// protocol together with a plain alias.
pub static BINDINGS: &[Binding] = &[
    // --- Navigation ---
    // Linear's command menu — Cmd/Ctrl+K. Inside a text field Ctrl+K keeps
    // its readline meaning.
    bind(&[ctrl('k')], NORMAL, App::open_palette)
        .help(Section::Navigation, "C-k", "Command palette")
        .hint_on(SCREENS, 11, "^K", "commands"),
    bind(&[plain('j'), code(KeyCode::Down)], NORMAL, down)
        .help(Section::Navigation, "j/k", "Move cursor down/up")
        .hint_on(
            &[Sidebar, ViewList, ProjectList, CycleList],
            75,
            "j/k",
            "move",
        ),
    bind(&[plain('k'), code(KeyCode::Up)], NORMAL, up),
    // `g` opens a chord: `gg` jumps to the top, `gm`/`gp`/… switch views.
    bind(&[plain('g')], SCROLLING, App::start_goto_chord),
    bind(&[code(KeyCode::Home)], NORMAL, first),
    bind(&[plain('g')], &[Sidebar], first),
    bind(&[plain('G'), code(KeyCode::End)], NORMAL, last).help(
        Section::Navigation,
        "gg/G",
        "First/last item",
    ),
    // Space is Linear's peek; with no split pane it simply opens the row.
    bind(
        &[code(KeyCode::Enter), plain(' ')],
        LISTS,
        App::open_selected,
    )
    .help(Section::Navigation, "Enter", "Open")
    .hint(10, "Enter", "open"),
    bind(
        &[code(KeyCode::Enter), plain(' ')],
        &[Sidebar],
        App::sidebar_activate,
    )
    .hint(76, "Enter", "open"),
    bind(
        &[code(KeyCode::Esc)],
        &[IssueList, IssueDetail, ProjectDetail, CycleDetail],
        back,
    )
    .help(Section::Navigation, "Esc", "Back / close")
    .hint_on(&[IssueDetail], 1, "Esc", "back"),
    bind(&[plain('q')], NESTED, leave),
    // Step to the neighbouring issue without leaving the detail view.
    bind(&[plain('J')], &[IssueDetail], |app| app.step_issue(1))
        .help(Section::Navigation, "J/K", "Next/previous issue (detail)")
        .hint(2, "J/K", "next/prev")
        .command("Next issue", &["down", "step"]),
    bind(&[plain('K')], &[IssueDetail], |app| app.step_issue(-1))
        .command("Previous issue", &["up", "step"])
        .in_section(Section::Navigation),
    // --- Sidebar ---
    bind(&[code(KeyCode::Tab)], SCREENS, |app| {
        app.focus_sidebar(true)
    })
    .help(Section::Sidebar, "Tab", "Focus sidebar / content")
    .hint_on(LISTS, 80, "Tab", "sidebar")
    .command("Focus sidebar", &["navigation"]),
    bind(
        &[code(KeyCode::Tab), code(KeyCode::Esc)],
        &[Sidebar],
        |app| app.focus_sidebar(false),
    )
    .hint(80, "Tab", "content")
    .command("Focus content", &["list"])
    .in_section(Section::Sidebar),
    bind(&[ctrl('b')], NORMAL, App::toggle_sidebar)
        .help(Section::Sidebar, "C-b", "Show/hide sidebar")
        .hint_on(&[Sidebar], 85, "^B", "hide")
        .command("Toggle sidebar", &["show", "hide"]),
    // A tree folds with h/l, as in every file explorer.
    bind(
        &[
            plain('h'),
            code(KeyCode::Left),
            plain('l'),
            code(KeyCode::Right),
        ],
        &[Sidebar],
        App::sidebar_toggle,
    )
    .help(Section::Sidebar, "h/l", "Fold/unfold a Favorites folder")
    .hint(77, "h/l", "fold")
    .command(
        "Fold or unfold folder",
        &["favorites", "collapse", "expand"],
    ),
    // --- Go to: the second key of a `g …` chord ---
    // Linear reaches its view presets with `G` then `A`/`B`/`E` (Active,
    // Backlog, All issues) and its pages with the other letters.
    bind(&[plain('a')], &[GoTo], |app| {
        team_preset(app, Preset::Active)
    })
    .help(Section::GoTo, "g a", "Active issues")
    .hint(0, "a", "active")
    .command("Go to active issues", &["preset"])
    .on(NORMAL),
    bind(&[plain('b')], &[GoTo], |app| {
        team_preset(app, Preset::Backlog)
    })
    .help(Section::GoTo, "g b", "Backlog")
    .hint(0, "b", "backlog")
    .command("Go to backlog", &["preset"])
    .on(NORMAL),
    bind(&[plain('e')], &[GoTo], |app| team_preset(app, Preset::All))
        .help(Section::GoTo, "g e", "All issues")
        .hint(0, "e", "all issues")
        .command("Go to all issues", &["preset", "everything"])
        .on(NORMAL),
    bind(&[plain('m')], &[GoTo], |app| app.activate(Nav::MyIssues))
        .help(Section::GoTo, "g m", "My issues")
        .hint(0, "m", "my issues")
        .command("Go to my issues", &["assigned", "mine"])
        .on(NORMAL),
    bind(&[plain('v')], &[GoTo], |app| app.activate(Nav::Views))
        .help(Section::GoTo, "g v", "Views")
        .hint(0, "v", "views")
        .command("Go to views", &["saved", "custom"])
        .on(NORMAL),
    bind(&[plain('p')], &[GoTo], |app| {
        app.go_to_team_section(TeamSection::Projects)
    })
    .help(Section::GoTo, "g p", "Projects")
    .hint(0, "p", "projects")
    .command("Go to projects", &[])
    .on(NORMAL),
    bind(&[plain('c')], &[GoTo], |app| {
        app.go_to_team_section(TeamSection::Cycles)
    })
    .help(Section::GoTo, "g c", "Cycles")
    .hint(0, "c", "cycles")
    .command("Go to cycles", &["sprint"])
    .on(NORMAL),
    // Inside herdr: the pane of the agent working on the issue. Linear has
    // no agents beside it, and no `g w`.
    bind(&[plain('w')], &[GoTo], App::jump_to_agent)
        .help(
            Section::Agent,
            "g w",
            "Go to the agent working on the issue",
        )
        .hint(0, "w", "agent")
        .command(
            "Go to the agent working on this issue",
            &["herdr", "pane", "workspace"],
        )
        .on(ISSUE_SCREENS)
        .herdr_only(),
    // vim: gg
    bind(&[plain('g')], &[GoTo], first).hint(0, "g", "top"),
    // Destination jumps by number, a TUI shorthand for the sidebar. Linear has
    // no equivalent — it reaches pages with `g …`, which works here too.
    bind(&[plain('1')], INDEXES, App::go_to_team_issues)
        .help(Section::GoTo, "1-5", "Issues/My/Projects/Cycles/Views")
        .command("Go to team issues", &["list"])
        .on(NORMAL),
    bind(&[plain('2')], INDEXES, |app| app.activate(Nav::MyIssues)),
    bind(&[plain('3')], INDEXES, |app| {
        app.go_to_team_section(TeamSection::Projects)
    }),
    bind(&[plain('4')], INDEXES, |app| {
        app.go_to_team_section(TeamSection::Cycles)
    }),
    bind(&[plain('5')], INDEXES, |app| app.activate(Nav::Views)),
    // --- List display ---
    // Neither preset nor grouping has a Linear keybinding — the web app puts
    // both behind a display-options menu — so they take keys Linear leaves free.
    bind(&[code(KeyCode::BackTab)], ISSUE_LISTS, App::cycle_preset)
        .help(
            Section::ListDisplay,
            "S-Tab",
            "Next preset (Active/Backlog/All)",
        )
        .hint(50, "S-Tab", "preset")
        .command("Next preset", &["active", "backlog", "all", "display"]),
    // Linear's Issues / Projects tabs on a Views page.
    bind(&[code(KeyCode::BackTab)], &[ViewList], App::cycle_view_kind)
        .hint(50, "S-Tab", "issues/projects")
        .command("Switch between issue and project views", &["tab"])
        .in_section(Section::ListDisplay),
    bind(&[plain('D')], ISSUE_LISTS, App::cycle_group_by)
        .help(
            Section::ListDisplay,
            "D",
            "Group by status/assignee/\u{2026}",
        )
        .hint(60, "D", "group")
        .command("Next grouping", &["group by", "display"]),
    palette_only(ISSUE_LISTS, App::open_group_by)
        .command(
            "Group by\u{2026}",
            &["status", "assignee", "priority", "project", "display"],
        )
        .in_section(Section::ListDisplay),
    bind(&[plain('z')], ISSUE_LISTS, App::toggle_selected_group)
        .help(Section::ListDisplay, "z / Z", "Fold group / all groups")
        .hint(70, "z", "fold")
        .command("Fold or unfold group", &["collapse", "expand"]),
    bind(&[plain('Z')], ISSUE_LISTS, App::toggle_all_groups)
        .command("Fold or unfold all groups", &["collapse", "expand"])
        .in_section(Section::ListDisplay),
    // --- Notes for the agent, a TUI-only addition: Linear has no agent
    // beside it to hand remarks to. ---
    bind(&[plain('n')], ISSUE_SCREENS, App::start_note_on_issue)
        .help(Section::Agent, "n", "Note on the issue under the cursor")
        .command(
            "Add a note for your agent",
            &["feedback", "remark", "review"],
        ),
    bind(&[plain('N')], SCREENS, App::start_note_on_view)
        .help(Section::Agent, "N", "Note on the whole view")
        .command("Add a note on this view", &["feedback", "remark", "review"]),
    bind(&[ctrl('s')], SCREENS, App::send_notes)
        .help(Section::Agent, "C-s", "Send the notes to your agent")
        .command("Send notes to your agent", &["prompt", "feedback", "herdr"]),
    palette_only(SCREENS, App::discard_notes)
        .command("Discard notes", &["clear", "feedback"])
        .in_section(Section::Agent),
    // --- Issue actions, matching Linear's single-key bindings ---
    // `c` creates an issue from anywhere, as in Linear.
    bind(&[plain('c')], SCREENS, App::start_new_issue)
        .help(Section::IssueActions, "c", "Create issue")
        .hint_on(ISSUE_LISTS, 30, "c", "new")
        .command("Create new issue", &["add", "file"]),
    bind(&[plain('s')], ISSUE_SCREENS, App::open_status_change)
        .help(Section::IssueActions, "s", "Change status")
        .hint(20, "s/p/a", "status/priority/assignee")
        .command("Change status\u{2026}", &["state", "workflow", "move"]),
    bind(&[plain('p')], ISSUE_SCREENS, App::open_priority_change)
        .help(Section::IssueActions, "p", "Change priority")
        .command("Change priority\u{2026}", &["urgency"]),
    // Direct priority — Linear: Shift+1..4 / Shift+0.
    bind(&[plain('!')], ISSUE_SCREENS, |app| {
        app.set_priority(Priority::Urgent)
    })
    .help(
        Section::IssueActions,
        "!@#$)",
        "Urgent/High/Medium/Low/None",
    )
    .command("Set priority to Urgent", &["priority"]),
    bind(&[plain('@')], ISSUE_SCREENS, |app| {
        app.set_priority(Priority::High)
    })
    .command("Set priority to High", &["priority"])
    .in_section(Section::IssueActions),
    bind(&[plain('#')], ISSUE_SCREENS, |app| {
        app.set_priority(Priority::Medium)
    })
    .command("Set priority to Medium", &["priority"])
    .in_section(Section::IssueActions),
    bind(&[plain('$')], ISSUE_SCREENS, |app| {
        app.set_priority(Priority::Low)
    })
    .command("Set priority to Low", &["priority"])
    .in_section(Section::IssueActions),
    bind(&[plain(')')], ISSUE_SCREENS, |app| {
        app.set_priority(Priority::None)
    })
    .command("Remove priority", &["priority", "none"])
    .in_section(Section::IssueActions),
    bind(&[plain('a')], ISSUE_SCREENS, App::open_assignee_change)
        .help(Section::IssueActions, "a", "Assign to someone")
        .command("Assign to\u{2026}", &["assignee", "owner", "unassign"]),
    bind(&[plain('i')], ISSUE_SCREENS, App::assign_to_me)
        .help(Section::IssueActions, "i", "Assign to me")
        .command("Assign to me", &["assignee", "self", "take"]),
    // Add comment — Linear: Ctrl+M. A legacy terminal reports Ctrl+M as
    // Enter, so plain `m` is accepted too (Linear leaves `m` for relations,
    // which this client does not support).
    bind(&[ctrl('m'), plain('m')], ISSUE_SCREENS, App::start_comment)
        .help(Section::IssueActions, "m", "Add comment (Ctrl+M)")
        .hint_on(&[IssueDetail], 21, "m", "comment")
        .command("Add comment", &["reply", "message"]),
    // --- Copy & open ---
    // Copy issue ID — Linear: Ctrl+.
    bind(
        &[ctrl_no_shift('.'), plain('y')],
        ISSUE_SCREENS,
        App::copy_identifier,
    )
    .help(Section::CopyOpen, "y", "Copy issue ID (Ctrl+.)")
    .hint_on(&[IssueDetail], 23, "y", "copy ID")
    .command("Copy issue ID", &["identifier", "clipboard"]),
    // Copy issue URL — Linear: Ctrl+Shift+, (some terminals report `<`).
    bind(
        &[ctrl_shift(','), ctrl_shift('<'), plain('Y')],
        ISSUE_SCREENS,
        App::copy_url,
    )
    .help(Section::CopyOpen, "Y", "Copy issue URL (Ctrl+Shift+,)")
    .command("Copy issue URL", &["link", "clipboard"]),
    // Copy git branch name — Linear: Ctrl+Shift+. (or `>`).
    bind(
        &[ctrl_shift('.'), ctrl_shift('>'), plain('b')],
        ISSUE_SCREENS,
        App::copy_branch_name,
    )
    .help(Section::CopyOpen, "b", "Copy branch name (Ctrl+Shift+.)")
    .command("Copy git branch name", &["clipboard", "checkout"]),
    // A TUI-only action, since Linear is already in the browser.
    bind(&[plain('o')], SCREENS, App::open_in_browser)
        .help(Section::CopyOpen, "o", "Open on linear.app")
        .hint_on(&[IssueDetail], 22, "o", "open")
        .command("Open in browser", &["linear.app", "web"]),
    // --- Search & filter ---
    bind(&[plain('/')], ISSUE_LISTS, App::start_search)
        .help(Section::SearchFilter, "/", "Filter as you type")
        .hint(40, "/", "filter")
        .command("Search in this list", &["find", "filter"]),
    bind(&[code(KeyCode::Enter)], &[Search], App::finish_search).hint(1, "Enter", "keep"),
    bind(&[code(KeyCode::Esc)], EDITING, cancel).hint_on(&[Search], 2, "Esc", "clear"),
    bind(&[ctrl('g')], &[Search], App::search_workspace)
        .help(Section::SearchFilter, "C-g", "Search all of Linear")
        .hint(3, "Ctrl+G", "search all of Linear"),
    bind(&[plain('f')], ISSUE_LISTS, App::open_filter)
        .help(Section::SearchFilter, "f/F", "Filter / clear filters")
        .command("Filter\u{2026}", &["status", "priority"]),
    bind(&[plain('F')], ISSUE_LISTS, App::clear_filters)
        .command("Clear filters", &["reset"])
        .in_section(Section::SearchFilter),
    // --- Mouse, routed by `App::click` ---
    documented(Section::Mouse, "click", "Select; click again to open"),
    documented(Section::Mouse, "click", "Sidebar, chips, group headers"),
    documented(Section::Mouse, "wheel", "Scroll"),
    // --- Other ---
    bind(&[plain('t')], TEAM_PAGES, App::open_team_select)
        .help(Section::Other, "t", "Switch team")
        .command("Switch team\u{2026}", &["workspace"]),
    // Linear switches workspaces from its own menu, with no shortcut to mirror.
    palette_only(SCREENS, App::open_workspace_select)
        .command("Switch workspace\u{2026}", &["organization", "account"])
        .in_section(Section::Other),
    // Linear syncs live and binds plain `r` to Rename, so this client uses the
    // terminal convention instead and leaves `r` free.
    bind(&[code(KeyCode::F(5)), ctrl('r')], SCREENS, refresh)
        .help(Section::Other, "F5/C-r", "Refresh")
        .hint_on(&[ViewList, ProjectList, CycleList], 90, "^R", "refresh")
        .command("Refresh", &["reload", "sync"]),
    bind(&[plain('?')], NORMAL, App::open_help)
        .help(Section::Other, "?", "Toggle this help")
        .hint(99, "?", "help")
        .command("Show keyboard shortcuts", &["help", "keys"]),
    bind(&[plain('q')], TOP_LEVEL, App::quit)
        .help(Section::Other, "q", "Quit")
        .command("Quit", &["exit", "close"])
        .on(NORMAL),
    // --- Scrolling ---
    bind(&[ctrl('d')], NORMAL, half_page_down).help(
        Section::Scrolling,
        "C-d/C-u",
        "Half page down/up",
    ),
    bind(&[ctrl('u')], NORMAL, half_page_up),
    bind(&[code(KeyCode::PageDown)], SCROLLING, page_down).help(
        Section::Scrolling,
        "PgDn/PgUp",
        "Full page down/up",
    ),
    bind(&[code(KeyCode::PageUp)], SCROLLING, page_up),
    // --- Editing: readline-style, in every text field ---
    bind(&[ctrl('w')], TEXT, |app| edit(app, Input::kill_word)).help(
        Section::Editing,
        "C-w",
        "Delete previous word",
    ),
    bind(&[ctrl('u')], TEXT, |app| edit(app, Input::kill_to_start)).help(
        Section::Editing,
        "C-u/C-k",
        "Delete to start/end",
    ),
    bind(&[ctrl('k')], TEXT, |app| edit(app, Input::kill_to_end)),
    bind(&[ctrl('a'), code(KeyCode::Home)], TEXT, |app| {
        edit(app, Input::home)
    })
    .help(Section::Editing, "C-a/C-e", "Jump to start/end"),
    bind(&[ctrl('e'), code(KeyCode::End)], TEXT, |app| {
        edit(app, Input::end)
    }),
    bind(&[code(KeyCode::Left)], TEXT, |app| edit(app, Input::left)),
    bind(&[code(KeyCode::Right)], TEXT, |app| edit(app, Input::right)),
    bind(&[code(KeyCode::Backspace)], TEXT, |app| {
        edit(app, Input::backspace)
    }),
    bind(&[code(KeyCode::Delete)], TEXT, |app| {
        edit(app, Input::delete)
    }),
    // Ctrl/Alt+Enter submits; a bare Enter breaks the line.
    bind(&[ctrl_or_alt(KeyCode::Enter)], SUBMITTABLE, submit).help(
        Section::Editing,
        "C-Enter",
        "Submit",
    ),
    bind(&[bare(KeyCode::Enter)], MULTILINE, |app| {
        edit(app, |input| input.insert('\n'))
    }),
    // A bare Enter on the title advances rather than inserting a newline.
    bind(&[bare(KeyCode::Enter)], &[IssueTitle], |app| {
        app.new_issue_cycle_field(true)
    }),
    bind(&[code(KeyCode::Tab)], FORM, |app| {
        app.new_issue_cycle_field(true)
    }),
    bind(&[code(KeyCode::BackTab)], FORM, |app| {
        app.new_issue_cycle_field(false)
    }),
    bind(
        &[plain('j'), code(KeyCode::Down), code(KeyCode::Right)],
        &[IssuePriority],
        |app| app.new_issue_cycle_priority(1),
    ),
    bind(
        &[plain('k'), code(KeyCode::Up), code(KeyCode::Left)],
        &[IssuePriority],
        |app| app.new_issue_cycle_priority(-1),
    ),
];

/// The context a key press lands in.
pub fn context(app: &App) -> Ctx {
    match app.view.input_mode {
        InputMode::Search => Search,
        InputMode::Comment => Comment,
        InputMode::Note => Note,
        InputMode::NewIssue => match app.view.new_issue.as_ref().map(|form| form.field) {
            Some(FormField::Description) => IssueDescription,
            Some(FormField::Priority) => IssuePriority,
            Some(FormField::Title) | None => IssueTitle,
        },
        InputMode::Normal if app.view.pending_chord == Some('g') => GoTo,
        InputMode::Normal if app.view.sidebar.focus => Sidebar,
        InputMode::Normal => app.nav.screen.into(),
    }
}

/// The palette's commands for a context: by section, what acts on the issue
/// at hand first, then in table order.
pub fn commands(ctx: Ctx, herdr: bool) -> Vec<&'static Binding> {
    let mut commands: Vec<&Binding> = BINDINGS
        .iter()
        .filter(|b| b.shown(herdr) && b.command.is_some_and(|c| c.on.contains(&ctx)))
        .collect();
    commands.sort_by_key(|b| b.command.map(|c| c.section.palette_rank()));
    commands
}

impl Key {
    /// How the key is written in the palette, e.g. `s`, `Ctrl+B`, `F5`.
    pub fn label(&self) -> String {
        let name = match self.code {
            KeyCode::Char(' ') => "Space".to_string(),
            KeyCode::Char(c) => match self.mods {
                Mods::Ctrl | Mods::CtrlNoShift | Mods::CtrlShift | Mods::CtrlOrAlt => {
                    c.to_ascii_uppercase().to_string()
                }
                _ => c.to_string(),
            },
            KeyCode::Enter => "Enter".into(),
            KeyCode::Esc => "Esc".into(),
            KeyCode::Tab => "Tab".into(),
            KeyCode::BackTab => "Shift+Tab".into(),
            KeyCode::Backspace => "Backspace".into(),
            KeyCode::Delete => "Del".into(),
            KeyCode::Home => "Home".into(),
            KeyCode::End => "End".into(),
            KeyCode::PageUp => "PgUp".into(),
            KeyCode::PageDown => "PgDn".into(),
            KeyCode::Up => "\u{2191}".into(),
            KeyCode::Down => "\u{2193}".into(),
            KeyCode::Left => "\u{2190}".into(),
            KeyCode::Right => "\u{2192}".into(),
            KeyCode::F(n) => format!("F{n}"),
            other => format!("{other:?}"),
        };
        match self.mods {
            Mods::Ctrl | Mods::CtrlNoShift => format!("Ctrl+{name}"),
            Mods::CtrlShift => format!("Ctrl+Shift+{name}"),
            Mods::CtrlOrAlt => format!("Ctrl+{name}"),
            Mods::Any | Mods::NoCtrl | Mods::Bare => name,
        }
    }
}

impl Binding {
    /// The keys that trigger it, as the palette shows them: the plain alias
    /// first, since it works in every terminal.
    pub fn keys_label(&self) -> String {
        let plain = |k: &&Key| matches!(k.mods, Mods::Any | Mods::NoCtrl | Mods::Bare);
        // A kitty original can come in several spellings (`Ctrl+Shift+,` is
        // also reported as `<`); one of them is enough.
        let labels: Vec<String> = self
            .keys
            .iter()
            .filter(plain)
            .chain(self.keys.iter().find(|k| !plain(k)))
            .map(Key::label)
            // The second key of a `g …` chord does something else alone.
            .map(|label| {
                if self.context == [GoTo] {
                    format!("g {label}")
                } else {
                    label
                }
            })
            .collect();
        labels.join(" / ")
    }
}

/// The status-bar hints for a context, in display order.
pub fn hints(ctx: Ctx, herdr: bool) -> Vec<&'static Hint> {
    let mut hints: Vec<&Hint> = BINDINGS
        .iter()
        .filter(move |b| b.shown(herdr))
        .filter_map(|b| b.hint.as_ref())
        .filter(|h| h.on.contains(&ctx))
        .collect();
    hints.sort_by_key(|h| h.rank);
    hints
}

/// The help overlay's rows under one heading, in table order.
pub fn help_rows(section: Section, herdr: bool) -> impl Iterator<Item = &'static Help> {
    BINDINGS
        .iter()
        .filter(move |b| b.shown(herdr))
        .filter_map(|b| b.help.as_ref())
        .filter(move |h| h.section == section)
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    if ctrl && key.code == KeyCode::Char('c') {
        app.quit();
        return;
    }

    // Error popup dismisses on any key
    if app.view.error_popup.is_some() {
        app.dismiss_error();
        return;
    }

    // Help overlay: j/k scrolls, anything else closes
    if app.view.show_help {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => app.scroll_help(1),
            KeyCode::Char('k') | KeyCode::Up => app.scroll_help(-1),
            _ => app.close_help(),
        }
        return;
    }

    // Esc abandons a half-typed chord
    if key.code == KeyCode::Esc && app.view.pending_chord.take().is_some() {
        return;
    }

    // The palette types into its own query.
    if app.view.popup == Popup::Palette {
        crate::interface::tui::palette::handle_key(app, key);
        return;
    }

    // Popup takes priority
    if app.view.popup != Popup::None {
        handle_popup_keys(app, key);
        return;
    }

    let ctx = context(app);
    // A pending `g` swallows the next key, whether or not it finishes a chord.
    if ctx == GoTo {
        app.end_chord();
    }
    if let Some(binding) = BINDINGS
        .iter()
        .find(|b| b.shown(app.herdr) && b.applies_in(ctx) && b.matches(&key))
    {
        (binding.action)(app);
        return;
    }
    // Anything else typed into a text field is text.
    if TEXT.contains(&ctx)
        && let KeyCode::Char(c) = key.code
        && !ctrl
    {
        edit(app, |input| input.insert(c));
    }
}

/// Keys in a pick-one popup. Until something is typed, the popup keeps its
/// single-key moves (`j`/`k`, `g`, `q`, digits); after that every letter is
/// part of the query, and only arrows, `Ctrl+N`/`Ctrl+P`, Home/End, and Enter
/// move or pick. An upper-case letter always starts a query, since matching
/// ignores case.
fn handle_popup_keys(app: &mut App, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let typing = !app.view.popup_query.is_empty();
    match key.code {
        KeyCode::Esc => app.popup_escape(),
        KeyCode::Char('q') if !typing && !ctrl => app.popup_escape(),
        KeyCode::Down => app.popup_next(),
        KeyCode::Up => app.popup_prev(),
        KeyCode::Char('n') if ctrl => app.popup_next(),
        KeyCode::Char('p') if ctrl => app.popup_prev(),
        KeyCode::Char('j') if !typing && !ctrl => app.popup_next(),
        KeyCode::Char('k') if !typing && !ctrl => app.popup_prev(),
        KeyCode::Home => app.popup_first(),
        KeyCode::End => app.popup_last(),
        KeyCode::Char('g') if !typing && !ctrl => app.popup_first(),
        KeyCode::Enter => app.apply_popup(),
        KeyCode::Char(c @ '1'..='9') if !typing && !ctrl => {
            app.popup_pick((c as usize) - ('1' as usize))
        }
        KeyCode::Backspace => app.popup_erase(),
        KeyCode::Char('w') if ctrl => app.edit_popup_query(Input::kill_word),
        KeyCode::Char('u') if ctrl => app.edit_popup_query(Input::kill_to_start),
        KeyCode::Char(c) if !ctrl => app.popup_type(c),
        _ => {}
    }
}

// --- Actions that depend on where they run ---

fn down(app: &mut App) {
    if app.view.sidebar.focus {
        app.sidebar_move(1);
    } else if app.nav.screen == Screen::IssueDetail {
        app.scroll_down();
    } else {
        app.move_selection(1);
    }
}

fn up(app: &mut App) {
    if app.view.sidebar.focus {
        app.sidebar_move(-1);
    } else if app.nav.screen == Screen::IssueDetail {
        app.scroll_up();
    } else {
        app.move_selection(-1);
    }
}

fn half_page_down(app: &mut App) {
    if app.view.sidebar.focus {
        app.sidebar_move(app.half_page());
    } else if app.nav.screen == Screen::IssueDetail {
        app.scroll_by(app.frame.detail_viewport as i16 / 2);
    } else {
        app.move_selection(app.half_page());
    }
}

fn half_page_up(app: &mut App) {
    if app.view.sidebar.focus {
        app.sidebar_move(-app.half_page());
    } else if app.nav.screen == Screen::IssueDetail {
        app.scroll_by(-(app.frame.detail_viewport as i16) / 2);
    } else {
        app.move_selection(-app.half_page());
    }
}

fn page_down(app: &mut App) {
    if app.nav.screen == Screen::IssueDetail {
        app.scroll_by(app.frame.detail_viewport as i16);
    } else {
        app.move_selection(app.frame.list_viewport.max(1) as isize);
    }
}

fn page_up(app: &mut App) {
    if app.nav.screen == Screen::IssueDetail {
        app.scroll_by(-(app.frame.detail_viewport as i16));
    } else {
        app.move_selection(-(app.frame.list_viewport.max(1) as isize));
    }
}

fn first(app: &mut App) {
    if app.view.sidebar.focus {
        app.sidebar_move(isize::MIN / 2);
    } else if app.nav.screen == Screen::IssueDetail {
        app.scroll_to_top();
    } else {
        app.select_first();
    }
}

fn last(app: &mut App) {
    if app.view.sidebar.focus {
        app.sidebar_move(isize::MAX / 2);
    } else if app.nav.screen == Screen::IssueDetail {
        app.scroll_to_bottom();
    } else {
        app.select_last();
    }
}

/// Esc: close the issue view; elsewhere drop a search first, and only then
/// leave a project or cycle page.
fn back(app: &mut App) {
    match app.nav.screen {
        Screen::IssueDetail => app.close_detail(),
        Screen::ProjectDetail | Screen::CycleDetail if app.list().search.is_empty() => {
            app.leave_container()
        }
        _ => app.clear_search(),
    }
}

/// `q` on a nested page steps back instead of quitting.
fn leave(app: &mut App) {
    if app.nav.screen == Screen::IssueDetail {
        app.close_detail();
    } else {
        app.leave_container();
    }
}

fn refresh(app: &mut App) {
    if app.nav.screen == Screen::IssueDetail {
        app.refresh_detail();
    } else {
        app.force_reload();
    }
}

fn team_preset(app: &mut App, preset: Preset) {
    app.go_to_team_issues();
    app.set_preset(preset);
}

fn cancel(app: &mut App) {
    match app.view.input_mode {
        InputMode::Search => app.cancel_search(),
        InputMode::Comment => app.cancel_comment(),
        InputMode::Note => app.cancel_note(),
        InputMode::NewIssue => app.cancel_new_issue(),
        InputMode::Normal => {}
    }
}

fn submit(app: &mut App) {
    match app.view.input_mode {
        InputMode::Comment => app.submit_comment(),
        InputMode::Note => app.submit_note(),
        InputMode::NewIssue => app.submit_new_issue(),
        InputMode::Search | InputMode::Normal => {}
    }
}

/// The text field keys are typed into, if any.
fn active_input(app: &mut App) -> Option<&mut Input> {
    match app.view.input_mode {
        InputMode::Search => Some(&mut app.list_mut().search),
        InputMode::Comment => Some(&mut app.view.comment),
        InputMode::Note => Some(&mut app.view.note),
        InputMode::NewIssue => app
            .view
            .new_issue
            .as_mut()
            .and_then(|form| match form.field {
                FormField::Title => Some(&mut form.title),
                FormField::Description => Some(&mut form.description),
                FormField::Priority => None,
            }),
        InputMode::Normal => None,
    }
}

/// Edits the active field. A search filters as you type, so the list stays in
/// sync with the query.
fn edit(app: &mut App, change: impl FnOnce(&mut Input)) {
    if let Some(input) = active_input(app) {
        change(input);
    }
    if app.view.input_mode == InputMode::Search {
        app.apply_search();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::core::entity::Issue;
    use crate::interface::tui::app::{IssueSource, Screen};
    use crossterm::event::{KeyEventKind, KeyEventState};

    fn press(app: &mut App, code: KeyCode) {
        handle_key(
            app,
            KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
        );
    }

    fn stated(id: &str, state_id: &str, name: &str, kind: &str) -> Issue {
        serde_json::from_str(&format!(
            r#"{{"id":"{id}","identifier":"ENG-{id}","title":"t{id}","priority":0,
                "state":{{"id":"{state_id}","name":"{name}","type":"{kind}","position":1}},
                "assignee":null,"description":null,"comments":null,"project":null,"cycle":null}}"#
        ))
        .unwrap()
    }

    /// The API returns these as Todo, In Progress, Todo; grouping shows In
    /// Progress first, so the second row on screen is raw index 0.
    fn reordered() -> Vec<Issue> {
        vec![
            stated("1", "s-todo", "Todo", "unstarted"),
            stated("2", "s-prog", "In Progress", "started"),
            stated("3", "s-todo", "Todo", "unstarted"),
        ]
    }

    fn app() -> App {
        let mut app = App::new(&Config::default());
        app.outbox.requests.clear();
        app
    }

    /// Regression: Enter on a project's issue opened whichever issue sat at the
    /// cursor's position in the raw API order, not the one highlighted.
    #[test]
    fn enter_on_a_project_issue_opens_the_highlighted_one() {
        let mut app = app();
        app.store.issues[IssueSource::Project].items = reordered();
        app.nav.screen = Screen::ProjectDetail;
        press(&mut app, KeyCode::Char('j'));
        let highlighted = app.focused_issue().unwrap().id.clone();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.nav.screen, Screen::IssueDetail);
        assert_eq!(app.store.current_issue.as_ref().unwrap().id, highlighted);
        assert_eq!(highlighted, "1");
    }

    #[test]
    fn enter_on_a_cycle_issue_opens_the_highlighted_one() {
        let mut app = app();
        app.store.issues[IssueSource::Cycle].items = reordered();
        app.nav.screen = Screen::CycleDetail;
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('j'));
        let highlighted = app.focused_issue().unwrap().id.clone();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.store.current_issue.as_ref().unwrap().id, highlighted);
        assert_eq!(highlighted, "3");
    }

    /// Every list screen: keyboard Enter and the highlighted row agree.
    #[test]
    fn enter_on_every_issue_list_opens_the_highlighted_issue() {
        use crate::interface::tui::app::Nav;
        for (screen, nav) in [
            (Screen::IssueList, None),
            (Screen::IssueList, Some(Nav::MyIssues)),
            (Screen::ProjectDetail, None),
            (Screen::CycleDetail, None),
        ] {
            let mut app = app();
            app.store.issues[IssueSource::Team].items = reordered();
            app.store.issues[IssueSource::My].items = reordered();
            app.store.issues[IssueSource::Project].items = reordered();
            app.store.issues[IssueSource::Cycle].items = reordered();
            app.set_preset(crate::interface::tui::grouping::Preset::All);
            app.outbox.requests.clear();
            if let Some(nav) = nav {
                app.nav.dest = nav;
            }
            app.nav.screen = screen;
            for downs in 0..3 {
                app.nav.screen = screen;
                app.set_selected_index(0);
                for _ in 0..downs {
                    press(&mut app, KeyCode::Char('j'));
                }
                let highlighted = app.focused_issue().unwrap().id.clone();
                press(&mut app, KeyCode::Enter);
                assert_eq!(
                    app.store.current_issue.as_ref().unwrap().id,
                    highlighted,
                    "{screen:?} {nav:?} row {downs}"
                );
                app.close_detail();
            }
        }
    }

    fn press_with(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
        handle_key(
            app,
            KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
        );
    }

    fn typed(app: &mut App, text: &str) {
        for c in text.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    /// A team with three active issues on its list.
    fn team_app() -> App {
        let mut app = app();
        app.store.teams =
            vec![serde_json::from_str(r#"{"id":"t","name":"Core","key":"ENG"}"#).unwrap()];
        app.store.issues[IssueSource::Team].items = reordered();
        app
    }

    #[test]
    fn a_g_chord_jumps_and_esc_abandons_it() {
        let mut app = team_app();
        press(&mut app, KeyCode::Char('g'));
        assert_eq!(app.view.pending_chord, Some('g'));
        press(&mut app, KeyCode::Char('m'));
        assert_eq!(app.nav.dest, Nav::MyIssues);
        assert_eq!(app.view.pending_chord, None);

        press(&mut app, KeyCode::Char('g'));
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.view.pending_chord, None);
        assert_eq!(app.nav.dest, Nav::MyIssues, "Esc only drops the chord");
    }

    #[test]
    fn a_number_key_picks_a_popup_entry() {
        let mut app = team_app();
        press(&mut app, KeyCode::Char('p'));
        assert!(matches!(app.view.popup, Popup::PriorityChange(_)));
        // Row 2 of the priority menu is Urgent.
        press(&mut app, KeyCode::Char('2'));
        assert_eq!(app.view.popup, Popup::None);
        assert!(matches!(
            app.outbox.requests.back(),
            Some(crate::core::usecase::Request::Issue(
                crate::core::usecase::issue::Request::SetPriority {
                    priority: Priority::Urgent,
                    ..
                }
            ))
        ));
    }

    #[test]
    fn search_filters_as_you_type_and_esc_clears_it() {
        let mut app = team_app();
        press(&mut app, KeyCode::Char('/'));
        assert_eq!(app.view.input_mode, InputMode::Search);
        typed(&mut app, "t2");
        assert_eq!(app.visible_issues().len(), 1);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.view.input_mode, InputMode::Normal);
        assert_eq!(app.visible_issues().len(), 1, "Enter keeps the query");

        press(&mut app, KeyCode::Esc);
        assert_eq!(app.visible_issues().len(), 3);
    }

    #[test]
    fn keys_typed_into_search_are_not_commands() {
        let mut app = team_app();
        press(&mut app, KeyCode::Char('/'));
        typed(&mut app, "q");
        assert!(!app.should_quit);
        assert_eq!(app.list().search.value, "q");
    }

    #[test]
    fn ctrl_c_quits_from_any_mode() {
        let mut app = team_app();
        press(&mut app, KeyCode::Char('/'));
        press_with(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(app.should_quit);
    }

    #[test]
    fn linears_ctrl_period_and_its_plain_alias_both_copy_the_id() {
        for (code, modifiers) in [
            (KeyCode::Char('.'), KeyModifiers::CONTROL),
            (KeyCode::Char('y'), KeyModifiers::NONE),
        ] {
            let mut app = team_app();
            press_with(&mut app, code, modifiers);
            assert_eq!(app.outbox.clipboard.as_deref(), Some("ENG-2"), "{code:?}");
        }
    }

    #[test]
    fn shift_digits_set_a_priority_directly() {
        let mut app = team_app();
        press(&mut app, KeyCode::Char('!'));
        assert_eq!(app.focused_issue().unwrap().priority, Priority::Urgent);
    }

    #[test]
    fn tab_moves_focus_to_the_sidebar_and_back() {
        let mut app = team_app();
        press(&mut app, KeyCode::Tab);
        assert!(app.view.sidebar.focus);
        press(&mut app, KeyCode::Char('j'));
        assert!(app.view.sidebar.focus, "j moves within the sidebar");
        press(&mut app, KeyCode::Esc);
        assert!(!app.view.sidebar.focus);
    }

    #[test]
    fn q_quits_a_list_but_only_closes_the_detail() {
        let mut app = team_app();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.nav.screen, Screen::IssueDetail);
        press(&mut app, KeyCode::Char('q'));
        assert_eq!(app.nav.screen, Screen::IssueList);
        assert!(!app.should_quit);
        press(&mut app, KeyCode::Char('q'));
        assert!(app.should_quit);
    }

    #[test]
    fn enter_breaks_a_comment_line_and_ctrl_enter_posts_it() {
        let mut app = team_app();
        press(&mut app, KeyCode::Char('m'));
        assert_eq!(app.view.input_mode, InputMode::Comment);
        typed(&mut app, "hi");
        press(&mut app, KeyCode::Enter);
        typed(&mut app, "there");
        assert_eq!(app.view.comment.value, "hi\nthere");
        press_with(&mut app, KeyCode::Enter, KeyModifiers::CONTROL);
        assert_eq!(app.view.input_mode, InputMode::Normal);
        assert!(matches!(
            app.outbox.requests.back(),
            Some(crate::core::usecase::Request::Issue(crate::core::usecase::issue::Request::Comment { body, .. })) if body == "hi\nthere"
        ));
    }

    #[test]
    fn the_help_overlay_closes_on_any_other_key() {
        let mut app = team_app();
        press(&mut app, KeyCode::Char('?'));
        assert!(app.view.show_help);
        press(&mut app, KeyCode::Char('j'));
        assert!(app.view.show_help, "j scrolls the help");
        press(&mut app, KeyCode::Char('x'));
        assert!(!app.view.show_help);
    }
}

#[cfg(test)]
mod table_tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};

    /// The plainest event a key answers to.
    fn sample(key: &Key) -> KeyEvent {
        let modifiers = match key.mods {
            Mods::Any | Mods::NoCtrl | Mods::Bare => KeyModifiers::NONE,
            Mods::Ctrl | Mods::CtrlNoShift | Mods::CtrlOrAlt => KeyModifiers::CONTROL,
            Mods::CtrlShift => KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        };
        KeyEvent {
            code: key.code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    /// Dispatch takes the first match, so two bindings sharing a key in one
    /// context would leave the second one dead.
    #[test]
    fn no_two_bindings_claim_a_key_in_the_same_context() {
        for (i, a) in BINDINGS.iter().enumerate() {
            for b in &BINDINGS[i + 1..] {
                let Some(ctx) = a.context.iter().find(|c| b.context.contains(c)) else {
                    continue;
                };
                for key in a.keys.iter().chain(b.keys) {
                    let event = sample(key);
                    assert!(
                        !(a.matches(&event) && b.matches(&event)),
                        "{key:?} is bound twice in {ctx:?}"
                    );
                }
            }
        }
    }

    /// A hint must not advertise a key where it does nothing.
    #[test]
    fn hints_show_only_where_their_binding_applies() {
        for binding in BINDINGS {
            if let Some(hint) = &binding.hint {
                for ctx in hint.on {
                    assert!(binding.applies_in(*ctx), "{:?} hint on {ctx:?}", hint.keys);
                }
            }
        }
    }

    /// A keyless row either documents the mouse or is a palette-only command.
    #[test]
    fn every_keyless_binding_is_documented() {
        for binding in BINDINGS.iter().filter(|b| b.keys.is_empty()) {
            if binding.command.is_some() {
                continue;
            }
            let help = binding.help.expect("a keyless binding documents something");
            assert_eq!(help.section, Section::Mouse, "{:?}", help.keys);
        }
    }

    /// Everything the help overlay lists as an action can be run from the
    /// palette, so a new binding cannot be added without an entry. Movement,
    /// scrolling and text editing are exempt, and so are keys that only work
    /// inside a text field, where the palette cannot be opened.
    #[test]
    fn every_documented_action_is_in_the_palette() {
        let exempt = [
            Section::Navigation,
            Section::Mouse,
            Section::Scrolling,
            Section::Editing,
        ];
        for binding in BINDINGS {
            let Some(help) = binding.help else { continue };
            if exempt.contains(&help.section) || binding.context.iter().all(|c| EDITING.contains(c))
            {
                continue;
            }
            assert!(
                binding.command.is_some(),
                "{} ({}) has no palette entry",
                help.text,
                help.keys
            );
        }
    }

    /// A chord's second key is shown with its `g`: alone, `a` assigns.
    #[test]
    fn a_chord_command_shows_both_keys() {
        let active = commands(IssueList, false)
            .into_iter()
            .find(|b| b.command.is_some_and(|c| c.title == "Go to active issues"))
            .unwrap();
        assert_eq!(active.keys_label(), "g a");
        let assign = commands(IssueList, false)
            .into_iter()
            .find(|b| b.command.is_some_and(|c| c.title == "Assign to me"))
            .unwrap();
        assert_eq!(assign.keys_label(), "i");
    }

    /// Two entries with one title in one place could not be told apart.
    #[test]
    fn no_two_commands_share_a_title_in_a_context() {
        for ctx in NORMAL.iter().chain(&[GoTo]) {
            let titles: Vec<&str> = commands(*ctx, true)
                .iter()
                .filter_map(|b| b.command.map(|c| c.title))
                .collect();
            for (i, title) in titles.iter().enumerate() {
                assert!(!titles[i + 1..].contains(title), "{title} twice in {ctx:?}");
            }
        }
    }

    /// The palette can only be opened where nothing is being typed.
    #[test]
    fn commands_are_offered_only_outside_text_fields() {
        for binding in BINDINGS {
            if let Some(command) = binding.command {
                assert!(
                    command.on.iter().all(|c| NORMAL.contains(c)),
                    "{} offered in a text field",
                    command.title
                );
            }
        }
    }

    #[test]
    fn keys_are_labelled_plain_alias_first() {
        let comment = BINDINGS
            .iter()
            .find(|b| b.command.is_some_and(|c| c.title == "Add comment"))
            .unwrap();
        assert_eq!(comment.keys_label(), "m / Ctrl+M");
        let url = BINDINGS
            .iter()
            .find(|b| b.command.is_some_and(|c| c.title == "Copy issue URL"))
            .unwrap();
        assert_eq!(url.keys_label(), "Y / Ctrl+Shift+,");
    }
}
