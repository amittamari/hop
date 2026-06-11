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

## How releases work

Releases are driven by [release-plz](https://release-plz.dev) using our
conventional-commit history. You never hand-edit the version or hand-type a tag.

```
push to master ──▶ release-plz.yml ──▶ opens/updates a "Release PR"
                                        (bumps Cargo.toml + CHANGELOG.md)
       merge the Release PR ──────────▶ release-plz tags vX.Y.Z
                            tag push ──▶ release.yml builds binaries,
                                         publishes the GitHub Release,
                                         regenerates Formula/hop.rb in the tap
```

- `release-plz.yml` (on every push to `master`) keeps a **Release PR** open. It
  computes the next version from conventional commits (`feat:` → minor, `fix:` →
  patch, `feat!:`/`BREAKING CHANGE` → major) and writes `CHANGELOG.md`.
- Merging that PR makes release-plz create the `vX.Y.Z` git tag.
- The tag triggers `release.yml` exactly as before: build → GitHub Release → tap
  formula. Tag and `Cargo.toml` can no longer drift, because the tag is derived
  from the version release-plz committed.
- `ci.yml` runs `fmt`, `clippy`, and `cargo test` on every PR and master push, so
  a release can't ship a build that fails CI.

## Cutting a release

1. Land your changes on `master` with conventional-commit messages
   (`feat: …`, `fix: …`). CI must be green.
2. release-plz opens (or updates) a **Release PR** titled like
   `chore: release vX.Y.Z`. Review the version bump and generated changelog.
3. Merge the Release PR. That's it — the tag and `release.yml` run automatically.
4. Watch it:

   ```sh
   gh run watch
   ```

   When it finishes you'll have a GitHub Release with the `.tar.gz` assets and an
   updated `Formula/hop.rb` in `amittamari/homebrew-tap`.

5. Verify the install:

   ```sh
   brew install amittamari/tap/hop
   hop --version
   ```

   On an already-tapped machine, upgrade with:

   ```sh
   brew update && brew upgrade hop
   ```

### Manual fallback

The old flow still works if you ever need it — bump `version` in `Cargo.toml`,
`cargo build` to refresh `Cargo.lock`, commit, then `git tag vX.Y.Z &&
git push origin vX.Y.Z`. `release.yml` triggers on any `v*` tag regardless of how
it was created.

## One-time setup

release-plz needs a token that is **not** the default `GITHUB_TOKEN`, because
tags pushed with `GITHUB_TOKEN` do not trigger other workflows (so `release.yml`
would never fire). Create a token with `contents: write` + `pull-requests: write`
on this repo — a fine-grained PAT or a GitHub App token both work — and add it as
the repo secret `RELEASE_PLZ_TOKEN`. See the
[release-plz token docs](https://release-plz.dev/docs/github/token) for the
GitHub App route (recommended for longevity).

## Notes

- The formula's `version "X.Y.Z"` comes from the tag (`v` stripped), so keep the
  tag in sync with `Cargo.toml`.
- The formula is generated; never hand-edit it in the tap. Re-run a release to
  change it.
- Linux arm64 builds use GitHub's `ubuntu-24.04-arm` runners (free for public
  repos). Drop that matrix entry if you don't want Linux arm64.
