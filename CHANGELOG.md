# Changelog

All notable changes to this project will be documented in this file.

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
