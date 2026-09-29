# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/boubou666/GitVibe/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/boubou666/GitVibe/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/boubou666/GitVibe/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/boubou666/GitVibe/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/boubou666/GitVibe/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/boubou666/GitVibe/releases/tag/v0.1.0
