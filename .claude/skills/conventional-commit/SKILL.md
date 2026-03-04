---
name: conventional-commit
description: Create commit messages following the Conventional Commits specification
---

# Conventional Commit Skill

This skill helps create commit messages following the [Conventional Commits](https://www.conventionalcommits.org/) specification.

## Instructions

You are a commit message assistant. Your task is to analyze the current changes and create a well-formatted commit message following the Conventional Commits specification.

### Steps to Follow

1. **Analyze Changes**: First, run `git status` and `git diff --staged` (or `git diff` if nothing is staged) to understand what has been changed.

2. **Determine the Commit Type**: Based on the changes, select the appropriate type:
   - `feat`: A new feature
   - `fix`: A bug fix
   - `docs`: Documentation only changes
   - `style`: Changes that do not affect the meaning of the code (white-space, formatting, etc.)
   - `refactor`: A code change that neither fixes a bug nor adds a feature
   - `perf`: A code change that improves performance
   - `test`: Adding missing tests or correcting existing tests
   - `build`: Changes that affect the build system or external dependencies
   - `ci`: Changes to CI configuration files and scripts
   - `chore`: Other changes that don't modify src or test files
   - `revert`: Reverts a previous commit

3. **Identify the Scope** (optional): Determine if there's a specific component, module, or area affected.

4. **Write the Description**: Create a concise description in imperative mood (e.g., "add feature" not "added feature").

5. **Add Body** (if needed): For complex changes, add a body explaining:
   - The motivation for the change
   - Contrast with previous behavior

6. **Add Footer** (if needed): Include any breaking changes or issue references.

### Commit Message Format

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

### Examples

Simple commit:

```
feat: add user authentication
```

With scope:

```
fix(parser): handle empty input gracefully
```

With body and footer:

```
feat(api): add endpoint for user preferences

Add a new REST endpoint that allows users to save and retrieve
their application preferences. This enables persistent settings
across sessions.

BREAKING CHANGE: The /settings endpoint has been renamed to /preferences
Closes #123
```

### Important Rules

- Keep the first line (header) under 72 characters
- Use lowercase for type and scope
- Do not end the description with a period
- Use imperative mood in the description
- Separate header from body with a blank line
- Wrap body at 72 characters
- Do NOT include auto-generated footers (e.g., "Generated with Claude Code", "Co-Authored-By")

### Interactive Workflow

After analyzing the changes, present the user with:

1. A summary of what changed
2. The proposed commit message
3. Ask for confirmation or modifications before executing the commit

If the user provides additional context or wants changes, adjust the message accordingly.
