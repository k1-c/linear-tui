# linear-tui

Read [AGENTS.md](AGENTS.md) first — it holds the build commands, project
structure, architecture invariants, keybinding policy, API type rules, and the
commit/PR/release flow. This file only adds what is specific to Claude Code.

## Rules

- To change what a user or an agent can do, start in `src/usecase/<aggregate>.rs`: write the rule into the use case's doc comment and a test per rule, as `docs/development.md` ("The use case layer") describes. `tests/usecase_spec.rs` and `tests/architecture.rs` must pass.
- After completing any implementation task, ALWAYS run `/verify` before marking it done.
- When committing, ALWAYS use the `conventional-commit` skill. Never create commits manually with `git commit`.
- Before preparing a release, use the `release` skill to check readiness. It does not publish — release-plz does, per AGENTS.md.
- When you notice recurring patterns, user corrections, or conventions worth codifying, use `/skill-creator` to create a new skill.

## Skill Routing

- For ownership/borrow/lifetime errors (E0382, E0597 etc.), use the `m01-ownership` skill.
- For concurrency/async issues (Send/Sync, tokio, deadlock), use the `m07-concurrency` skill.
- For error handling design (Result, anyhow, thiserror), use the `m06-error-handling` skill.
- For performance concerns, use the `m10-performance` skill.
- For unsafe code or FFI, use the `unsafe-checker` skill.
- For Rust version/crate info lookups, use the `rust-learner` skill.
- For code navigation (go-to-definition, find references), use the `rust-code-navigator` skill.
- For refactoring (rename, extract, move), use the `rust-refactor-helper` skill.
- For code style and naming conventions, use the `coding-guidelines` skill.
