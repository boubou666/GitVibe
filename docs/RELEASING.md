# Releasing GitVibe

The in-app updater reads the latest published GitHub release. Keep platform asset names stable so the app can select the right package. GitHub supplies each asset's SHA-256 digest in the release API; the app requires it before downloading.

1. Bump `Cargo.toml` and `Cargo.lock`, add a dated Keep a Changelog entry, and update this guide if package names change.
2. Open a pull request. `main` requires the Windows, Linux, Apple Silicon macOS, and Intel macOS jobs to pass.
3. Download the five CI artifacts from the successful PR run and inspect their contents. Windows includes both a portable zip and `GitVibe-windows-X64-setup.exe`.
4. Merge the pull request. Publish `vX.Y.Z` on the merge commit with the five platform packages. Keep the release published and non-prerelease so GitHub's `/releases/latest` endpoint returns it.
5. Verify the public release assets and their `digest` fields. Launch the previous GitVibe version to check that it finds the new version, downloads the matching package, and verifies it. On Windows, check that the installer opens from the Updates screen.

Package names expected by the updater:

| Platform | Asset |
| --- | --- |
| Windows x64 | `GitVibe-windows-X64-setup.exe` |
| Linux x64 | `GitVibe-linux-X64.tar.gz` |
| macOS Apple Silicon | `GitVibe-macos-ARM64.zip` |
| macOS Intel | `GitVibe-macos-X64.zip` |

The Windows zip is for portable use. It is not selected by the updater. Releases should also include `SHA256SUMS.txt` for manual verification.
