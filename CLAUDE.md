# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A.R.M.O.R.Y. (Arcade Runner Menu On Rails, Yeah!) is a launcher for arcade light gun setups: a set of tabs and extra-large buttons, usable with a light gun, that start games, emulators and tools. Everything (tabs, buttons, labels, commands) is configured by the user in a YAML file.

It is a **single Rust crate** that ships as one standalone binary for Linux, Windows and macOS. No firmware, no workspace, no extra crates.

Status: skeleton only. UI toolkit, config format and architecture are not decided yet.

## Build Commands

```bash
cargo build                  # debug build
cargo build --release        # release build (LTO, opt-level "z", panic=abort, stripped)
cargo test                   # run tests
cargo clippy --all-targets -- -D warnings   # lint (warnings are CI failures)
cargo fmt --check            # CI formatting check
shellcheck scripts/*.sh && scripts/test.sh  # script checks, as in CI
```
The toolchain is pinned in `rust-toolchain.toml` (1.98 with clippy and rustfmt).

## Development Workflow

Uses **GitFlow**: features branch from `develop`; releases merge to `main` and then back to `develop`. PRs to `main` and `develop` need 1 review and passing CI. See `GITFLOW.md` and `.github/WORKFLOWS.md`.

Branch naming: `feature/*`, `bugfix/*`, `release/*`, `hotfix/*`

## CI/CD

See `.github/WORKFLOWS.md`.

- **ci.yml**: every push to a GitFlow branch and every PR to `main`/`develop`. Computes the version, then runs the reusable **build.yml**:
  - build + tests (Linux x86_64, Windows x86_64, macOS universal), clippy, fmt
  - shellcheck + `scripts/test.sh`
- **release-start.yml**: manual dispatch (`kind` release/hotfix, `bump`). Creates `release/X.Y.Z` from `develop` or `hotfix/X.Y.Z` from `main`, cuts the CHANGELOG section, opens a PR to `main`.
- **release-finish.yml**: runs when that PR is merged. Builds `X.Y.Z`, creates tag + GitHub Release with artifacts, and opens a back-merge PR into `develop` via `backmerge/X.Y.Z`.

If the Linux build later needs system libraries (e.g. for a GUI toolkit), add them to the `app` and `clippy` jobs in `build.yml` and to README.md.

### Versioning
SemVer, computed by `scripts/version.sh` from the branch and the `vX.Y.Z` tags; nothing is committed:
- tag `vX.Y.Z` → `X.Y.Z`
- `release/X.Y.Z` or `hotfix/X.Y.Z` → `X.Y.Z-rc.N`
- anything else (incl. `develop`) → `0.0.0+<sha>`

The version is injected at build time via the env var `ARMORY_VERSION`, read by `build.rs` and available as `env!("ARMORY_VERSION")`. `Cargo.toml` stays at `0.0.0`; don't bump it. Add changes to `## [Unreleased]` in CHANGELOG.md.

## Rust Code Standards

- No `unwrap()` without justification; use `Result<T, E>` with anyhow/thiserror.
- UI elements must stay large and easy to hit with a light gun; keep this in mind for every UI change.
