# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A.R.M.O.R.Y. (Arcade Runner Menu On Rails, Yeah!) is a launcher for arcade light gun setups: a set of tabs and extra-large buttons, usable with a light gun, that start games, emulators and tools. Everything (tabs, buttons, labels, commands) is configured by the user in a YAML file.

It is a **Cargo workspace**: the launcher is the root crate and ships as one standalone binary for Linux, Windows and macOS. Helper tools are separate member crates under `tools/` with their own dependencies (built only together with them, so the launcher never pulls them in); they share `Cargo.lock`, `target/`, the profiles in the root `Cargo.toml` and the toolchain, and are shipped in the release packages next to the launcher. No firmware, no library crates, no other member crates than tools.

The configuration format is documented for users in `docs/CONFIGURATION.md`; keep it in sync with `src/config.rs` and `examples/armory.yaml` (the example is also a test fixture).

## Architecture

- `src/main.rs`: command line (`--config`, `--help`, `--version`), starts the UI.
- `src/config.rs`: YAML schema (`Raw*` serde types), validation into `Config` with line numbers (`serde_saphyr::Spanned`), config file lookup. Unknown keys are errors (`deny_unknown_fields`).
- `src/ui.rs` + `ui/app.slint`: Slint window. `Controller` owns all state and pushes it into the UI; the `.slint` file only draws and reports clicks. Rust computes item positions (`layout.rs`) because Slint has no wrapping layout. Background results (monitor counts, exited programs) arrive over an mpsc channel that a `slint::Timer` drains on the UI thread.
- `src/layout.rs`: wraps tiles into rows, full-width rows for dividers/labels, page-wise scrolling that keeps rows whole.
- `src/launch.rs`: turns `run`/`args`/`with`/`cwd` into a command line per OS (`Plan::new` is pure and tested for all OSes); `tracked` says whether the started process's exit means the program ended.
- `src/monitor.rs`: thread that counts matching processes (sysinfo) every second for the watched names.
- `src/status.rs`: the three states, status texts with partial overrides, `{STATUS}` templates.
- `tools/launchbox-images/`: separate crate and binary (`ureq` + rustls/ring, `zip`, `quick-xml`, `image`). Downloads LaunchBox's `Metadata.zip`, streams `Metadata.xml` into a gzipped JSON index (games, alternate names, `Clear Logo` images; `index.rs`, bump `INDEX_VERSION` when `Index` changes) in the user cache dir, searches it (`search.rs`) and saves a logo as `images/<slug>.png` in the current folder, converting other formats (`logo.rs`). Hand-written argument parsing like the launcher; version via its own `build.rs` from `ARMORY_VERSION`.

Conventions:
- Light gun first: everything clickable is large, no scrollbars, no hover-only UI, no small targets. Keep this for every UI change.
- Process names are compared normalized (`monitor::normalize`): without folder and `.exe`, case-insensitive on Windows/macOS.
- Maximizing uses the winit window directly (`maximize_when_ready`), because Slint's `set_maximized()` before window creation gets overwritten. Restoring focuses via winit too; both need Slint's `unstable-winit-030` feature, so check them when updating Slint.
- Slint is used under the royalty-free license; keep the "Made with Slint" badge in README.md.
- App icon: `images/icon.svg` is the source; `icon.png` (256 px, window icon via `@image-url` in `app.slint`) and `icon.ico` (16–256 px, embedded into the .exe by `build.rs` with `winresource`, only when building on Windows) are generated from it:
  `for n in 16 24 32 48 64 128 256; do inkscape images/icon.svg --export-type=png --export-width=$n --export-filename=/tmp/icon-$n.png; done; cp /tmp/icon-256.png images/icon.png; convert /tmp/icon-{16,24,32,48,64,128,256}.png images/icon.ico`

## Build Commands

```bash
cargo build                  # debug build of the launcher only (root crate)
cargo build --release --workspace   # release build of all binaries (LTO, opt-level "z", panic=abort, stripped)
cargo test --workspace       # run all tests
cargo clippy --workspace --all-targets -- -D warnings   # lint (warnings are CI failures)
cargo fmt --all --check      # CI formatting check
shellcheck scripts/*.sh && scripts/test.sh  # script checks, as in CI
```
The toolchain is pinned in `rust-toolchain.toml` (1.98 with clippy and rustfmt). On Linux the build needs `libfontconfig-dev` and `pkg-config` (Slint).

```bash
cargo run -- --config examples/armory.yaml   # run with the example configuration
cargo run -p launchbox-images -- search time crisis   # run the LaunchBox image tool
```

## Development Workflow

Uses **GitFlow**: features branch from `develop`; releases merge to `main` and then back to `develop`. PRs to `main` and `develop` need 1 review and passing CI. See `GITFLOW.md` and `.github/WORKFLOWS.md`.

Branch naming: `feature/*`, `bugfix/*`, `release/*`, `hotfix/*`

## CI/CD

See `.github/WORKFLOWS.md`.

- **ci.yml**: every push to a GitFlow branch and every PR to `main`/`develop`. Computes the version, then runs the reusable **build.yml**:
  - build + tests of the whole workspace (Linux x86_64, Windows x86_64, macOS universal), clippy, fmt
  - shellcheck + `scripts/test.sh`
- **release-start.yml**: manual dispatch (`kind` release/hotfix, `bump`). Creates `release/X.Y.Z` from `develop` or `hotfix/X.Y.Z` from `main`, cuts the CHANGELOG section, opens a PR to `main`.
- **release-finish.yml**: runs when that PR is merged. Builds `X.Y.Z`, creates tag + GitHub Release with artifacts, and opens a back-merge PR into `develop` via `backmerge/X.Y.Z`.

Linux system libraries for the build are installed in the `app` and `clippy` jobs of `build.yml`; keep them in sync with README.md. Release packages contain the binaries (`armory`, `launchbox-images`), LICENSE and `examples/`; a new tool needs its binary added to the Build/Package steps (and the `lipo` loop) of `build.yml`.

### Versioning
SemVer, computed by `scripts/version.sh` from the branch and the `vX.Y.Z` tags; nothing is committed:
- tag `vX.Y.Z` → `X.Y.Z`
- `release/X.Y.Z` or `hotfix/X.Y.Z` → `X.Y.Z-rc.N`
- anything else (incl. `develop`) → `0.0.0+<sha>`

The version is injected at build time via the env var `ARMORY_VERSION`, read by `build.rs` and available as `env!("ARMORY_VERSION")`. `Cargo.toml` stays at `0.0.0`; don't bump it. Add changes to `## [Unreleased]` in CHANGELOG.md.

## Rust Code Standards

- No `unwrap()` without justification; use `Result<T, E>` with anyhow/thiserror.
