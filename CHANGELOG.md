# Changelog

All notable changes to this project will be documented in this file.

## [0.6.0] - 2026-09-24

### Documentation

- **agents**: Describe the store, use case and app layers

### Features

- **fuzzy**: Add an in-order, word-start-aware matcher
- **keys**: Declare command palette entries on BINDINGS rows
- **palette**: Open a command palette with Ctrl+K
- **palette**: Make the pickers palette pages that narrow as you type
- **palette**: Jump to issues, projects, cycles, views and teams

### Refactoring

- **app**: Gather renderer-recorded state into FrameState
- **store**: Move what Linear told us out of App into Store
- **usecase**: Run issue changes through one use case each
- **app**: Split App into navigation, view and outbox state
- **app**: Open the selected row through one intent



## [0.5.3] - 2026-09-24

### Bug Fixes

- **deps**: Upgrade ratatui to 0.30 and crossterm to 0.29
- **app**: Offer an issue's own team states and members in popups

### Miscellaneous

- Let the audit job write its check run
- Pin GitHub Actions to commit SHAs

### Performance

- **list**: Group the issue list once per frame
- **detail**: Reuse rendered markdown between frames

### Refactoring

- **keys**: Drive dispatch, hints, and help from one binding table



## [0.5.2] - 2026-09-24

### Bug Fixes

- **api**: Report refused issue mutations as failures
- **app**: Drop stale responses and keep the cursor on its issue
- **auth**: Write credentials and the log owner-only
- **auth**: Make the OAuth callback server robust to stray requests
- **api**: Time out stalled requests and keep bodies out of error logs
- **auth**: Refresh an expired OAuth token during a session
- **app**: Keep search and filters per issue list
- **grouping**: Key assignee groups by user and group in linear time
- **ui**: Measure avatars, icons and indents in cells, and theme project states
- **ui**: Correct the help overlay's sidebar and refresh entries
- **ui**: Stop the renderer panicking in a very small terminal
- **config**: Report unknown keys and out-of-range values
- **deps**: Update rustls, rustls-webpki, h2 and time past advisories

### Documentation

- **api**: Bring the type guide in line with the code

### Miscellaneous

- Test on every release platform, check the MSRV, and audit

### Refactoring

- **api**: Type ids and errors, and give the client a test seam
- **app**: Split app.rs into submodules
- Move request dispatch and CLI subcommands out of main.rs
- **app**: Let the popup carry what it acts on
- **keys**: Change app state only through App methods

### Testing

- Cover whole-frame rendering and the keybindings

### Build

- Raise rust-version to 1.88, the oldest toolchain that builds



## [0.5.1] - 2026-09-24

### Documentation

- **readme**: Show a demo recording

### Miscellaneous

- **demo**: Add scripts to seed a workspace and record the demo

### Build

- Keep the demo GIF and scripts out of the crate



## [0.5.0] - 2026-09-24

### Bug Fixes

- **keys**: Open the highlighted issue on project and cycle pages

### Features

- **views**: Support project views and team views



## [0.4.0] - 2026-09-23

### Documentation

- Describe the sidebar, favorites, list display, and mouse

### Features

- **ui**: Rebuild navigation around a sidebar, favorites, and views



## [0.3.1] - 2026-09-23

### Bug Fixes

- **api**: Tolerate unknown workflow state categories

### Miscellaneous

- **dev**: Pin the toolchain and tasks with mise

### Build

- **deps**: Use rustls instead of system OpenSSL



## [0.3.0] - 2026-09-23

### Documentation

- **readme**: Document signing in without an application

### Features

- **auth**: Sign in without registering an application

### Miscellaneous

- **assets**: Add the k1-c/tui application icon



## [0.2.1] - 2026-09-23

### Documentation

- Record the architecture, keybinding policy and release flow



## [0.2.0] - 2026-09-23

### Bug Fixes

- **app**: Keep lists and issue detail in sync after a mutation

### Documentation

- Document the reworked keybindings and features

### Features

- **api**: Add issue urls, branch names, search and issue creation
- **ui**: Add an issue creation form and richer issue detail
- **keys**: Align keybindings with linear's shortcuts

### Refactoring

- Run every api call off the ui thread



## [0.1.3] - 2026-03-05

### Miscellaneous

- Release v0.1.2



## [0.1.2] - 2026-03-04

### Bug Fixes

- **api**: Accept "canceled" variant from Linear API

### Documentation

- **api**: Add API type definition guide

### Features

- **logging**: Add tracing-based structured logging

### Miscellaneous

- Release v0.1.1
- **verify**: Add cargo test step to verify skill
- **skills**: Add commit splitting logic to conventional-commit

### Testing

- **api**: Add deserialization tests with JSON fixtures



## [0.1.1] - 2026-03-04

### Features

- **auth**: Add set-oauth command and fix API key auth header



## [0.1.0] - 2026-03-05

### Features

- OAuth2 + PKCE authentication with browser-based login
- Personal API key fallback authentication
- Issue list with pagination (cursor-based infinite scroll)
- Issue detail view with description and comments
- My Issues view (assigned to current user)
- Project list and project detail with associated issues
- Cycle list and cycle detail with associated issues
- Tab-based navigation (Issues / My Issues / Projects / Cycles)
- Issue mutations: status, priority, assignee changes
- Comment creation on issues
- Team selection and switching
- Issue filtering by status and priority
- Issue search by title and identifier
- Vim-style keybindings (j/k, g/G, /, ?)
- Help overlay with all keybindings
- Error popup overlay for user-visible errors
- Loading spinner animation
- Theme support: Default (dark), Light, Ocean
- Configuration via `~/.config/linear-tui/config.toml`
