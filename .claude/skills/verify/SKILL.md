---
name: verify
description: Run format, lint, and build checks on the Rust project. Automatically invoked after completing a task or implementation step. Use this to ensure code quality before moving on.
allowed-tools: Bash, Read, Edit
---

Run the following checks sequentially, stopping on first failure:

1. **Format**: `cargo fmt --all`
2. **Lint**: `cargo clippy --all-targets -- -D warnings 2>&1`
3. **Build**: `cargo build 2>&1`

## Behavior

- If format changes files, report which files were formatted
- If clippy produces warnings or errors, fix them before proceeding
- If build fails, fix compilation errors
- After all checks pass, report a brief summary: "verify: OK (fmt, clippy, build)"
- If any step required fixes, re-run all checks from the beginning to confirm

## When to invoke automatically

This skill MUST be run after:

- Completing any implementation task
- Before marking a task as completed (TaskUpdate status=completed)
- Before creating a commit

$ARGUMENTS
