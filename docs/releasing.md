# Releasing

[cargo-dist](https://axodotdev.github.io/cargo-dist/) 0.33.1 publishes GitHub Releases. The config is `dist-workspace.toml`. `.github/workflows/release.yml` runs on pull requests and on tags that contain a `MAJOR.MINOR.PATCH` version. A pull request runs `dist plan` and does not publish. Pushing the tag publishes the release.

1. Set `version` in `memo/Cargo.toml` to the new version. The current release is `0.1.0`.
2. Commit that change and push it to `main`.
3. Tag that commit `vX.Y.Z` (the Cargo version with a `v` prefix). Version `0.1.0` is the tag `v0.1.0`.
4. Push the tag: `git push origin vX.Y.Z`.

The workflow builds `memo` for these targets:

- `aarch64-apple-darwin`
- `aarch64-unknown-linux-gnu`
- `aarch64-pc-windows-msvc`
- `x86_64-apple-darwin`
- `x86_64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

The GitHub Release contains one archive per target (`.tar.xz` on macOS and Linux, `.zip` on Windows), a `.sha256` checksum beside each archive, `sha256.sum`, `memo-cli-installer.sh`, `memo-cli-installer.ps1`, `dist-manifest.json`, and `source.tar.gz` with its `.sha256`. Installers copy `memo` to `$CARGO_HOME/bin`, or `~/.cargo/bin` when `CARGO_HOME` is unset.

`dist plan` previews the release locally and does not publish it.
