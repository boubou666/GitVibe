# Roadmap

GitVibe aims to cover daily Git work in a visual Rust desktop client. Its terminal provides access to Git features while dedicated screens are built. No AI features are planned for the initial releases.

## Completed in 0.6.0

1. Theme choices, favorite and recent repository management, and remembered repository tabs.
2. Wider merge graph rendering without a lane cap and search across all refs and history.
3. Numbered diffs with highlighted changes and structured file history.
4. Visual line by line conflict resolution and per-line staging.
5. Shallow and sparse clone options, a working shell command dock, branch switching with local-change choices, and `.git` file support.

## Completed in 0.7.0

1. Create, open, lock, unlock, remove, and prune worktrees. Start one from a selected commit.
2. Add, initialize, update, sync, and open submodules.
3. Rebase onto a branch or commit, with autostash, conflict continuation, skip, and abort.
4. List and create GitHub pull requests through an existing `gh` installation and login.
5. Commit-search and refresh shortcuts, plus refresh after Git command errors so conflict state is visible.
6. Export commit and file patches, and apply a patch from disk.
7. Add and remove remotes, and create annotated tags.

## Completed in 0.8.0

1. Interactive rebase editor for reorder, pick, squash, reword, and drop on linear commit ranges. A confirmation step and reflog recovery path accompany history rewriting.
2. Diff line wrapping, previous and next change navigation, and confirmed hunk discard. Full-file view and word-level highlights were already available.

## Next: GitKraken workflow parity

1. **Diff review.** Add split view and language-aware syntax coloring. GitKraken documents these modes and actions in its [diff guide](https://help.gitkraken.com/gitkraken-desktop/diff/).
2. **Undo and redo.** Track reversible local Git actions with explicit recovery data. GitKraken limits undo to supported recent actions; GitVibe should make the same scope visible before offering a button. See its [undo guide](https://help.gitkraken.com/gitkraken-desktop/undo-and-redo/).
3. **Keyboard workflow.** Add a command palette, tab switching, graph navigation, stage/unstage shortcuts, and an accessibility pass for focus order, labels, contrast, and screen-reader semantics. Use the [GitKraken shortcut reference](https://help.gitkraken.com/gitkraken-desktop/keyboard-shortcuts/) as a comparison.
4. **Worktree context.** Show worktrees in the branch rail and their separate WIP state in the graph, and offer worktree creation from branch context menus. GitKraken's [worktree guide](https://help.gitkraken.com/gitkraken-desktop/worktrees/) describes those interactions.
5. **Repository operations.** Add remote URL editing and a searchable repository command menu. Clone provider tabs currently guide URL entry; account-backed repository browsing remains to be designed.

## Needs account or distribution decisions

1. GitLab and Bitbucket pull request workflows need a chosen authentication model and API scope. GitHub currently uses the user's local `gh` login. A common built-in OAuth integration would require app credentials and a secure token storage design.
2. Package signing and notarization need Windows signing credentials and Apple Developer identities. Windows has an installer and in-app install flow; Linux and macOS still require manual replacement after download.
3. A true embedded PTY needs a cross-platform terminal backend. The current command dock runs shell commands and keeps working-directory history, but interactive editors and full-screen programs cannot use it.

## Architecture notes

- `src/git.rs` owns Git execution and parsing.
- `src/github.rs` owns the optional GitHub CLI bridge.
- `src/app.rs` owns UI state and sends operations to worker threads.
- Use Git porcelain or explicit machine-readable formats for new parsers.
- Preserve user Git configuration and credential helpers; do not store credentials in GitVibe.
- Add repository integration tests for operations that change refs or the index.
