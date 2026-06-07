# Releasing & Homebrew

`hop` is distributed as a Homebrew tap backed by prebuilt binaries. The
`.github/workflows/release.yml` workflow runs on every `v*` tag: it builds
binaries for macOS (arm64) and Linux (arm64 + x86_64), publishes them to a
GitHub Release, then regenerates `Formula/hop.rb` in the tap repo.

> Intel macOS (`x86_64-apple-darwin`) is intentionally not built — the GitHub
> `macos-13` runners queue for a long time. Add a matrix entry + `on_intel`
> formula block back if you need it.

End users install with:

```sh
brew install amittamari/tap/hop
```

## Cutting a release

1. Bump `version` in `Cargo.toml`, run `cargo build` so `Cargo.lock` updates,
   commit.
2. Tag and push (the tag drives the workflow):

   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```

3. Watch it:

   ```sh
   gh run watch
   ```

   When it finishes you'll have a GitHub Release with four `.tar.gz` assets and
   an updated `Formula/hop.rb` in `amittamari/homebrew-tap`.

4. Verify the install:

   ```sh
   brew install amittamari/tap/hop
   hop --version
   ```

   On an already-tapped machine, upgrade with:

   ```sh
   brew update && brew upgrade hop
   ```

## Notes

- The formula's `version "X.Y.Z"` comes from the tag (`v` stripped), so keep the
  tag in sync with `Cargo.toml`.
- The formula is generated; never hand-edit it in the tap. Re-run a release to
  change it.
- Linux arm64 builds use GitHub's `ubuntu-24.04-arm` runners (free for public
  repos). Drop that matrix entry if you don't want Linux arm64.
