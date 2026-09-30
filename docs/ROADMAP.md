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

## Completed in 0.9.0

1. Side by side diff mode for working-tree and commit changes.
2. Worktrees in the branch rail, with change counts and their own WIP rows. Branch context menus can start a worktree.
3. Edit remote URLs in the Branches & tags view.

## Completed in 0.10.0

1. Searchable command palette for repository navigation and common actions.
2. Repository tab cycling, graph arrow navigation, and selected-file stage and unstage shortcuts.

## Completed in 0.11.0

1. Guarded undo and redo for staging and unstaging files, hunks, and lines, including binary and initial-commit staging. The index is replayed only when the repository, branch, HEAD, and index still match.

## Completed in 0.12.0

1. File-aware syntax highlighting for unified and side by side diffs, retaining parser state within each old and new hunk stream.
2. Keyboard navigation through changed files with Alt+Up and Alt+Down.
3. Native AccessKit bridge, labels and selection state for custom-painted tabs, branches, and commit rows, and improved Aurora muted-text contrast.

## Completed in 0.13.0

1. Local merge-target conflict checks for committed branch tips, with per-repository target selection and conflicting-file details. The check leaves the index and working tree untouched.

## Completed in 0.14.0

1. Git LFS status, local setup, pattern tracking and untracking, and pulling objects from the remote through the installed Git LFS extension.
2. A persistent syntax-color switch in unified and side by side diff views.

## Completed in 0.15.0

1. Restore a file from a selected commit into the working tree and index, with a guard against overwriting local changes.
2. Load a configured Git commit template into the commit editor and add validated co-author trailers.

## Next: GitKraken workflow parity

1. **Diff review.** Add better syntax parser state recovery across distant hunks. GitKraken documents its diff modes and actions in its [diff guide](https://help.gitkraken.com/gitkraken-desktop/diff/).
2. **Broader undo and redo.** Extend explicit recovery data beyond staging to safe, reversible local Git actions. History rewriting and working-tree discards need separate safeguards. See GitKraken's [undo guide](https://help.gitkraken.com/gitkraken-desktop/undo-and-redo/).
3. **Accessibility.** Continue reviewing focus order, labels, contrast, and screen-reader semantics across all pages. Use the [GitKraken shortcut reference](https://help.gitkraken.com/gitkraken-desktop/keyboard-shortcuts/) as a comparison for further keyboard coverage.
4. **Repository operations.** Clone provider tabs currently guide URL entry; account-backed repository browsing remains to be designed.
5. **More GitKraken workflows.** Add external file editing, amend controls, and direct commit context actions for rewording and dropping commits. Add a clear push-after-commit option. GitKraken describes these in its [file editing](https://help.gitkraken.com/gitkraken-desktop/editing-files/) and [commit](https://help.gitkraken.com/gitkraken-desktop/commits/) guides.
6. **Git LFS follow-up.** Add locked-file workflows and safe local-object pruning. Define how GitVibe should present LFS migration, which can rewrite existing history, before offering it in the UI. See GitKraken's [LFS guide](https://help.gitkraken.com/gitkraken-desktop/git-lfs/).

## Needs account or distribution decisions

1. GitLab and Bitbucket pull request workflows need a chosen authentication model and API scope. GitHub currently uses the user's local `gh` login. A common built-in OAuth integration would require app credentials and a secure token storage design.
2. Package signing and notarization need Windows signing credentials and Apple Developer identities. Windows has an installer and in-app install flow; Linux and macOS still require manual replacement after download.
3. A true embedded PTY needs a cross-platform terminal backend. The current command dock runs shell commands and keeps working-directory history, but interactive editors and full-screen programs cannot use it.
4. Issue and team panes require a provider and account model. GitKraken's [integrations](https://help.gitkraken.com/gitkraken-desktop/integrations/) and [team features](https://help.gitkraken.com/gitkraken-desktop/teams/) rely on hosted services.
5. Commit and tag signing controls need a decision on whether GitVibe should expose only the user's existing Git signing configuration or also manage signing keys. GitKraken supports both GPG and SSH signing in its [signing guide](https://help.gitkraken.com/gitkraken-desktop/commit-signing-with-gpg/).

## Architecture notes

- `src/git.rs` owns Git execution and parsing.
- `src/github.rs` owns the optional GitHub CLI bridge.
- `src/app.rs` owns UI state and sends operations to worker threads.
- Use Git porcelain or explicit machine-readable formats for new parsers.
- Preserve user Git configuration and credential helpers; do not store credentials in GitVibe.
- Add repository integration tests for operations that change refs or the index.
