# Releasing & Homebrew

`hop` is distributed as a Homebrew tap backed by prebuilt binaries. The
`.github/workflows/release.yml` workflow runs on every `v*` tag: it builds
binaries for macOS (arm64 + x86_64) and Linux (arm64 + x86_64), publishes them
to a GitHub Release, then regenerates `Formula/hop.rb` in the tap repo.

End users install with:

```sh
brew install amittamari/hop/hop
```

## One-time setup

1. **Create the tap repo.** Homebrew requires the name `homebrew-<tap>`:

   ```sh
   gh repo create amittamari/homebrew-hop --public \
     --description "Homebrew tap for hop"
   ```

   It can stay empty — the release workflow pushes `Formula/hop.rb` into it.

2. **Create a token the workflow can use to push to the tap**, and store it as
   the `HOMEBREW_TAP_TOKEN` secret on this repo.

   - Fine-grained PAT: scope it to `amittamari/homebrew-hop` with
     **Contents: Read and write**.
   - Add it as a secret:

     ```sh
     gh secret set HOMEBREW_TAP_TOKEN --repo amittamari/hop
     # paste the token when prompted
     ```

   (A classic PAT with `repo` scope also works.)

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
   an updated `Formula/hop.rb` in `amittamari/homebrew-hop`.

4. Verify the install:

   ```sh
   brew install amittamari/hop/hop
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
