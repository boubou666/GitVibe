# Roadmap

GitVibe aims to cover daily Git work in a visual Rust desktop client. The command console provides access to Git features while dedicated screens are built. No AI features are planned for the initial releases.

## Completed in 0.6.0

1. Theme choices, favorite and recent repository management, and remembered repository tabs.
2. Wider merge graph rendering without a lane cap and search across all refs and history.
3. Numbered diffs with highlighted changes and structured file history.
4. Visual line by line conflict resolution and per-line staging.

## Later

1. Interactive rebase editor and merge tooling.
2. Worktree and submodule management.
3. GitHub, GitLab, and Bitbucket pull request workflows.
4. Sign and notarize desktop packages. Windows has an installer and in-app install flow; Linux and macOS still require manual replacement after an in-app download.
5. Accessibility and keyboard navigation review.

## Architecture notes

- `src/git.rs` owns Git execution and parsing.
- `src/app.rs` owns UI state and sends operations to worker threads.
- Use Git porcelain or explicit machine-readable formats for new parsers.
- Preserve user Git configuration and credential helpers; do not store credentials in GitVibe.
- Add repository integration tests for operations that change refs or the index.
