# Releasing

[cargo-dist](https://axodotdev.github.io/cargo-dist/) publishes GitHub Releases. Pushing a `v*` tag whose version matches `memo/Cargo.toml` runs `.github/workflows/release.yml`.

1. Set `version` in `memo/Cargo.toml`, commit, and push to `main`.
2. Tag that commit `vX.Y.Z` (the Cargo version with a `v` prefix). Version `0.1.0` is the tag `v0.1.0`.
3. Push the tag: `git push origin vX.Y.Z`.

The workflow builds `memo` for Linux, macOS, and Windows, each on x86_64 and aarch64. The GitHub Release includes those archives, a `.sha256` file per archive, `sha256.sum`, `memo-cli-installer.sh`, and `memo-cli-installer.ps1`.

`dist plan` (cargo-dist 0.33.1) previews that release locally and does not publish it.
