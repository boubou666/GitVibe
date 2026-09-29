# Roadmap

GitVibe aims to cover daily Git work in a visual Rust desktop client. The command console provides access to Git features while dedicated screens are built. No AI features are planned for the initial releases.

## Near term

1. Add theme choices and improve recent repository management.
2. Improve topology rendering for complex merge graphs and add a global history search. Remote branch badges and commit pagination are in place.
3. Add richer diff rendering and file history beyond the current text view.
4. Add a visual line by line conflict editor and per-line staging. Whole-file conflict choices and per-hunk staging are in place.

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
