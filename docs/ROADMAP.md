# Roadmap

GitVibe aims to cover daily Git work in a visual Rust desktop client. The command console provides access to Git features while dedicated screens are built. No AI features are planned for the initial releases.

## Near term

1. Persist recent repositories, window state, and theme settings.
2. Add native folder pickers and clone destination selection.
3. Render complete commit topology and remote branch badges, with pagination beyond 300 commits.
4. Add richer diff rendering and compare any two commits.
5. Add a conflict resolver and per-line staging.
6. Add guarded branch/tag deletion and reset dialogs.

## Later

1. Interactive rebase editor and merge tooling.
2. Worktree and submodule management.
3. GitHub, GitLab, and Bitbucket pull request workflows.
4. Signed installers, notarization, and update delivery for Windows, Linux, and macOS.
5. Accessibility and keyboard navigation review.

## Architecture notes

- `src/git.rs` owns Git execution and parsing.
- `src/app.rs` owns UI state and sends operations to worker threads.
- Use Git porcelain or explicit machine-readable formats for new parsers.
- Preserve user Git configuration and credential helpers; do not store credentials in GitVibe.
- Add repository integration tests for operations that change refs or the index.
