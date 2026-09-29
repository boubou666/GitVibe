# GitVibe

GitVibe is a native desktop Git client written in Rust for Windows, Linux, and macOS. Its vivid, layered interface pairs a visual repository view with the Git executable already installed on your computer. GitVibe is an independent project and is not affiliated with GitKraken.

## Current features

- Open, initialize, and clone repositories, including local and authenticated remote URLs.
- Reopen recent repositories, restore window placement, and choose folders with native dialogs.
- Check for new releases on launch or from the Updates screen. Download a platform package with SHA-256 verification; Windows can launch the installer directly from the app.
- Inspect a colored commit graph with local and remote refs, load older commits in batches, search loaded history, view commit details, and compare any two commits.
- Review working tree status and diffs; stage or unstage files and individual text hunks, discard edits, and commit changes.
- Resolve merge conflicts file by file using Git's ours or theirs version, or edit a file externally and mark it resolved; complete or abort an in-progress merge.
- Inspect file history and blame; view a commit patch or compare it with its first parent.
- Cherry-pick and revert non-merge commits with confirmation.
- Create and switch branches, track remote branches, merge branches, and create tags.
- Delete merged local branches and local tags with confirmation; reset HEAD using soft, mixed, or hard mode.
- Fetch, pull, and push using your normal Git credential helper.
- Create, inspect, apply, and pop stashes.
- Run any Git command from the integrated Git console. This is an argument parser, not a shell.
- Drag a repository folder onto the window to open it.

The visual history initially loads the most recent 300 commits across local and remote refs; use "Load 300 more commits" at the bottom of the graph to extend it. Search applies to the commits currently loaded. Hunk actions apply to text diffs; binary files and untracked files still use whole-file staging. Conflict choices apply to an entire file; use an external editor to combine lines from both sides. During a rebase, Git's ours/theirs meaning differs from an ordinary merge. The console is the way to access Git features without a dedicated screen yet, such as rebase, bisect, submodules, and worktrees. Commands needing a terminal editor or interactive stdin are not supported in the console yet.

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

The app invokes Git directly with argument arrays. It does not pass text through PowerShell, `cmd`, or another shell. Remote operations use your existing Git configuration, SSH keys, and credential helper. Read and write commands run on a worker thread so the window stays responsive.

Discarding working tree changes asks for confirmation. Hard reset also requires typing `RESET`. Local branch deletion uses Git's safe `-d` mode, which refuses to delete unmerged work. The Git console can execute destructive Git commands, so review commands before running them.

## Development plan

See [docs/ROADMAP.md](docs/ROADMAP.md) for the feature gaps to close toward a full GitKraken-like workflow. Notable upcoming work includes theme choices, true topology rendering for complex merge graphs, a visual line by line conflict editor, interactive rebase, remote hosting integrations, signed installers, and automatic replacement on Linux and macOS. Release maintainers should follow [docs/RELEASING.md](docs/RELEASING.md).

## License

MIT. See [LICENSE](LICENSE).
