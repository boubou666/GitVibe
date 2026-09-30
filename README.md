# GitVibe

GitVibe is a native desktop Git client written in Rust for Windows, Linux, and macOS. Its compact three-pane workspace puts branches, the commit graph, and commit details side by side while using the Git executable already installed on your computer. GitVibe is an independent project and is not affiliated with GitKraken.

## Current features

- Open, initialize, and clone repositories, including local and authenticated remote URLs. Cloning supports shallow history and sparse checkout.
- Create, open, lock, unlock, and remove Git worktrees. A commit can be used as the starting point for a new worktree.
- Add, initialize, update, sync, and open submodules, including those that store their Git metadata outside the working tree.
- Keep multiple repositories open in tabs, pin favorites, search recent repositories, and restore window placement. The Changelog tab can be closed and reopened from the `+` menu.
- Choose between Aurora, Cosmic, and Ember themes; the selection persists between launches.
- Check for new releases on launch or from the Updates screen. Download a platform package with SHA-256 verification; Windows can launch the installer directly from the app.
- Inspect a colored commit graph with local and remote refs, load older commits in batches, search all history, view commit files, and compare any two commits. Wide merge graphs scroll horizontally.
- Right-click a commit for checkout, branch and tag creation, reset, cherry-pick, revert, comparisons, and SHA copying. Commit patches open in the center pane.
- Review numbered diffs with highlighted edits; stage or unstage files, individual text hunks, or lines; discard edits and commit changes.
- Export a commit or changed file as a patch and apply a patch from disk.
- Resolve text conflicts visually by selecting individual lines from ours and theirs or editing each result block. Whole-file choices, external editing, merge completion, and merge abort remain available.
- Browse structured file history, view a revision diff or file content, and inspect blame and commit patches.
- Cherry-pick and revert non-merge commits with confirmation.
- Create and switch branches, track remote branches, merge branches, and create tags.
- Add and remove remotes and create annotated tags.
- Rebase the current branch onto another ref with conflict continuation, skip, and abort controls.
- List and create GitHub pull requests when the GitHub CLI (`gh`) is installed and signed in.
- Delete merged local branches and local tags with confirmation; reset HEAD using soft, mixed, or hard mode.
- Fetch, pull, and push using your normal Git credential helper.
- Create, inspect, apply, and pop stashes.
- Run PowerShell commands on Windows or shell commands on Linux and macOS from a docked terminal with command history and working directory navigation.
- Drag a repository folder onto the window to open it.

The visual history initially loads the most recent 300 commits across local and remote refs; use "Load 300 more commits" at the bottom of the graph to extend it. Search scans the full history across all refs and shows the first 300 matches. Hunk and line actions apply to text diffs; binary files and untracked files still use whole-file staging. The visual conflict editor accepts UTF-8 text files up to 2 MB; whole-file choices and external editing remain available for other files. During a rebase, Git's ours/theirs meaning differs from an ordinary merge. The Rebase page can edit a linear sequence of up to 40 commits. The terminal runs one command at a time and does not yet provide a PTY for interactive full-screen programs or editor prompts. Press Ctrl/Cmd+F to search commits and F5 to refresh the repository.

## Download and run

Git must be installed and available on `PATH` on every platform. The [Desktop builds workflow](.github/workflows/desktop.yml) compiles and tests Windows, Linux, and macOS builds and uploads platform packages for each run. Linux folder dialogs use an XDG Desktop Portal backend or Zenity.

- **Windows:** Download `GitVibe-windows-X64-setup.exe` from the [latest release](https://github.com/boubou666/GitVibe/releases/latest). The per-user installer adds a Start Menu shortcut and an uninstaller without asking for administrator rights. A portable zip remains available.
- **Linux:** Extract the Linux archive and run `./gitvibe`. A desktop session with X11 or Wayland and OpenGL support is required.
- **macOS:** Choose the ARM64 archive for Apple Silicon or the X64 archive for Intel. Extract it and open `GitVibe.app`.

The Updates screen checks GitHub's latest published release automatically at startup and when you click **Check for updates**. Downloads are checked against the release asset's SHA-256 digest before they can be opened. On Windows, **Install update** starts the installer and closes GitVibe; finish the setup wizard to replace an existing installation. When updating from the portable zip, the installer creates a separate per-user installation, which you can then open from the Start Menu. On Linux and macOS, GitVibe opens the downloaded archive so you can replace the app manually. Current installers and macOS bundles are unsigned; Windows SmartScreen or macOS Gatekeeper may show a warning.

## Build from source

Prerequisites: stable Rust (1.97 or newer) and Git on `PATH`. On Linux, install the desktop development libraries listed in the [eframe setup guide](https://github.com/emilk/egui/blob/main/crates/eframe/README.md#linux).

```sh
cargo run --release
```

Open a repository immediately:

```sh
cargo run --release -- /path/to/repository
```

For development:

```sh
cargo test
cargo run
```

The integration tests exercise Git repository initialization, status, staging, commits, and snapshots on all three operating systems in CI. A Windows GUI launch has been checked locally. Automated GUI interaction on Linux and macOS is still to be added.

## How it works

The app invokes Git operations directly with argument arrays. Only commands entered into the terminal dock are passed to PowerShell or your POSIX shell. Remote operations use your existing Git configuration, SSH keys, and credential helper. Read and write commands run on a worker thread so the window stays responsive.

Discarding working tree changes asks for confirmation. Hard reset also requires typing `RESET`. Local branch deletion uses Git's safe `-d` mode, which refuses to delete unmerged work. The Git console can execute destructive Git commands, so review commands before running them.

## Development plan

See [docs/ROADMAP.md](docs/ROADMAP.md) for the feature gaps to close toward a full GitKraken-like workflow. Notable upcoming work includes split diffs, undo and redo, remote hosting integrations, signed installers, and automatic replacement on Linux and macOS. Release maintainers should follow [docs/RELEASING.md](docs/RELEASING.md).

## License

MIT. See [LICENSE](LICENSE).
