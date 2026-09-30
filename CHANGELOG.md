# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.12.0] - 2026-09-30

### Added

- File-aware syntax highlighting in working-tree and commit diffs, using separate parser state for the old and new sides of each hunk. Unknown file types keep the existing readable fallback.
- Alt+Up and Alt+Down to select changed files without using the mouse; selection scrolls into view in the Changes panel.

## [0.11.0] - 2026-09-30

### Added

- Undo and redo for recent staging and unstaging actions, including files, hunks, lines, and binary files. The toolbar and command palette show the action scope, and Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z work outside text fields.
- Safety checks before replaying a staging action: the repository, branch, HEAD, and index must still match the recorded state. Working files are never rewritten by this undo path.

## [0.10.0] - 2026-09-30

### Added

- Searchable command palette for repository pages and actions, opened with Ctrl/Cmd+K or the toolbar.
- Ctrl/Cmd+Tab and Ctrl/Cmd+Shift+Tab to cycle repository tabs.
- Up and Down arrows to select commits in the graph, scrolling the selection into view.
- Ctrl/Cmd+Shift+S and Ctrl/Cmd+Shift+U to stage or unstage the selected file, plus stage-all commands in the palette.

## [0.9.0] - 2026-09-30

### Added

- Side by side diff mode for working-tree files and commit patches, with aligned old and new line numbers and replacement highlights.
- Worktrees in the branch rail with per-worktree change counts, plus separate WIP rows in history and branch context actions to start a worktree.
- Remote URL editing from the Branches & tags screen.

## [0.8.0] - 2026-09-30

### Added

- Interactive rebase editor to reorder, pick, squash, reword, or drop up to 40 linear commits, with a confirmation step and conflict continuation controls.
- Diff line wrapping and previous/next hunk navigation in the working-tree view.
- Confirmed hunk discard for unstaged changes, leaving other hunks in the same file intact.

### Changed

- Clear a previous file's diff immediately when selecting a new changed file.

## [0.7.0] - 2026-09-30

### Added

- Worktree management for creating a checkout from HEAD or a selected commit, opening it as a repository, locking and unlocking it, removing clean worktrees, and pruning stale metadata.
- Submodule management for adding, initializing, updating, syncing, and opening nested repositories with `.git` file support.
- Rebase controls for replaying the current branch onto a chosen ref, autostashing local changes, resolving conflicts, continuing, skipping, and aborting.
- A GitHub pull request view that lists recent PRs and creates one using the existing GitHub CLI login.
- Keyboard shortcuts for commit search and repository refresh.
- Export a commit or changed file as a patch, and apply a patch from disk.
- Add and remove remotes and create annotated tags from the Branches & tags view.

### Changed

- Refresh repository state after a failed Git command so merge and rebase conflicts appear immediately.

## [0.6.0] - 2026-09-30

### Added

- Repository tabs with remembered open projects, a closable Changelog tab, and a full-width repository manager with favorites and search.
- Aurora, Cosmic, and Ember themes, saved between launches.
- Search across the complete history of all refs by commit message, author, or SHA, including commits beyond the loaded graph.
- Structured file history with revision diffs and file previews, plus changed-file lists in the commit inspector.
- Numbered diff lines with highlighted edits and individual line staging and unstaging.
- A visual conflict editor for selecting lines from either side, editing the result, and staging the resolved file.
- A New Tab page for opening, cloning, and creating repositories, plus repository and branch pickers in the top bar.
- A repository shell dock with command history, working directory navigation, and PowerShell or POSIX shell commands.
- A regression test for repositories whose `.git` file points to metadata stored outside the working tree.
- A two-column clone dialog with source URL presets, destination browsing, shallow clone, and sparse checkout.

### Changed

- Reworked the workspace around a compact top action bar, branch and remote rail, dense uncapped graph, wide diff view, and a right-hand changes and commit panel.
- Render graph lanes without the previous nine-lane limit, with horizontal scrolling for wide merge histories.
- Tightened the desktop layout around flat repository tabs, a narrower branch rail, a graphite theme, aligned graph columns, compact change rows, and a docked Git console.
- Select the newest commit when opening a repository so its details appear immediately.
- Replaced stock tab controls with one aligned tab strip, applied dark styling to popup menus and text fields, and enabled a dark Windows title bar.
- Grouped local and remote branches in the rail with a filter; clicking a branch returns to its graph commit.
- Added a commit right-click menu for checkout, creating branches and tags, reset, cherry-pick, revert, comparisons, and copying the SHA.
- Moved commit patches into a wide center view so the inspector stays focused on commit details and changed files.
- Rendered commit patches by file and hunk with line numbers; selecting a changed file opens its patch in the center view.
- Added inline branch naming at a commit, author badges and names in the graph, collapsible branch groups, and double-click branch switching with choices for local changes.
- Added pull-mode choices to the toolbar arrow and centered the Git actions.
- Improved working-tree diff rows with syntax cues, full-width change highlights, and compact hunk controls.
- Fixed zoom growth across launches, aligned repository action button heights, and highlighted the selected file in both inspector lists.

## [0.5.0] - 2026-09-30

### Added

- Per-user Windows installer with Start Menu shortcut, optional desktop shortcut, and uninstall support.
- Update screen with automatic and manual checks against the latest GitHub release.
- Verified package downloads using GitHub's SHA-256 release asset digest. On Windows, GitVibe opens the installer and closes; on Linux and macOS it opens the downloaded archive for replacement.

### Changed

- The repository is public and `main` requires pull requests and passing Windows, Linux, and both macOS checks.

## [0.4.0] - 2026-09-30

### Added

- Show unresolved files in a dedicated conflict group, inspect their working copy, choose Git's ours or theirs version with confirmation, or mark an externally edited file resolved after checking for conflict markers.
- Detect an in-progress merge, allow its merge commit even when choosing ours leaves no staged diff, and offer a confirmed merge abort.

## [0.3.0] - 2026-09-29

### Added

- Recent repositories and window placement persist between launches.
- Native folder selection for opening repositories and choosing a clone destination parent.
- Compare any two commits by setting a comparison base in the graph.
- Load more commit history beyond the initial 300 rows, show remote branch badges, and track remote branches from the branch view.
- Guarded deletion of local branches and tags, plus soft, mixed, and hard reset choices with an extra hard reset confirmation.

## [0.2.0] - 2026-09-29

### Added

- Windows, Linux, and macOS (Apple Silicon and Intel) build, test, and package jobs.
- Per-hunk staging and unstaging for text diffs.
- File history, blame, commit patch and parent comparison views.
- Confirmed cherry-pick and revert actions for non-merge commits.

### Changed

- Reworked the interface with a vivid dark palette, vector navigation icons, a denser graph with merge lines, staged and unstaged file groups, and a colored detail viewer.
- Use Git's local clone mode for filesystem repositories.

## [0.1.0] - 2026-09-29

### Added

- Native Rust desktop application with a dark Git workspace.
- Repository open, init, and clone flows.
- Commit history, branch and tag views, working tree status, diffs, staging, commits, and stashes.
- Fetch, pull, push, and an integrated Git command console.
- Background Git operations and unit tests for status and command parsing.

[Unreleased]: https://github.com/boubou666/GitVibe/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/boubou666/GitVibe/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/boubou666/GitVibe/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/boubou666/GitVibe/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/boubou666/GitVibe/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/boubou666/GitVibe/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/boubou666/GitVibe/releases/tag/v0.1.0
