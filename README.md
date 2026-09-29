# GitVibe

GitVibe is a native desktop Git client written in Rust. It pairs a dark, visual repository view with the Git executable already installed on your computer. GitVibe is an independent project and is not affiliated with GitKraken.

## Current features

- Open, initialize, and clone repositories, including local and authenticated remote URLs.
- Inspect a colored commit graph, search recent commits, and view commit details.
- Review working tree status and diffs; stage, unstage, discard, and commit changes.
- Create and switch branches, merge branches, and create tags.
- Fetch, pull, and push using your normal Git credential helper.
- Create, inspect, apply, and pop stashes.
- Run any Git command from the integrated Git console. This is an argument parser, not a shell.
- Drag a repository folder onto the window to open it.

The visual history currently loads the most recent 300 commits across local and remote refs. The console is the way to access Git features without a dedicated screen yet, such as cherry-pick, rebase, bisect, submodules, and worktrees. Commands needing a terminal editor or interactive stdin are not supported in the console yet.

## Build and run

Prerequisites: stable Rust (1.97 or newer) and Git on `PATH`.

```powershell
cargo run --release
```

Open a repository immediately:

```powershell
cargo run --release -- D:\path\to\repository
```

For development:

```powershell
cargo test
cargo run
```

Windows is the primary tested platform. The UI framework supports macOS and Linux, but those builds and packaging are not yet verified. On Windows, the release executable is `target\release\gitvibe.exe`.

## How it works

The app invokes Git directly with argument arrays. It does not pass text through PowerShell, `cmd`, or another shell. Remote operations use your existing Git configuration, SSH keys, and credential helper. Read and write commands run on a worker thread so the window stays responsive.

Discarding working tree changes asks for confirmation. The Git console can execute destructive Git commands, so review commands before running them.

## Development plan

See [docs/ROADMAP.md](docs/ROADMAP.md) for the feature gaps to close toward a full GitKraken-like workflow. Notable upcoming work includes true topology rendering for complex merge graphs, per-hunk staging, conflict resolution, interactive rebase, remote hosting integrations, and packaging.

## License

MIT. See [LICENSE](LICENSE).
