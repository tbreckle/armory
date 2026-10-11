# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Project skeleton: single Rust crate, GitFlow, CI/CD (build and test on Linux, Windows and macOS universal, clippy, fmt, script tests)
- Release workflows: `release-start.yml` creates release/hotfix branches, `release-finish.yml` tags, builds and publishes GitHub Releases
- SemVer versioning via `scripts/version.sh`, injected at build time (unofficial builds are `0.0.0+<sha>`)
- Light gun friendly launcher UI (Slint): tabs as big buttons in a column on the left, extra-large buttons with image and/or label, dividers with optional label, text labels; page up/down buttons instead of scrollbars
- Mouse wheel (and touchpad) scrolls the buttons of a tab row by row
- Starts at full display size as a normal (not fullscreen) window
- YAML configuration (`armory.yaml`, see `docs/CONFIGURATION.md`) with line-accurate error screen and reload; looked up via `--config`, next to the executable, or in the user's config directory
- Buttons start executables, scripts (`.sh`, `.bash`, `.bat`, `.cmd`, `.ps1`) or files through their file association; `with` overrides the program; built-in actions `quit`, `reload`, `minimize`
- `on_launch: minimize` minimizes ARMORY while a program runs and restores and focuses it afterwards; `wait_for` waits for another process instead
- App icon (shield with crosshair) for the window and, on Windows, embedded into `armory.exe` together with version info
- Example icons for the reload, minimize and quit buttons
- Example configuration: OpenFIRE section in the Tools tab (OpenFIRE App button, status label that warns while the app holds the gun's COM port)
- Status labels monitor a process every second (on / off / fault), with `{STATUS}` placeholder and global or per-label, partial status texts
- `launchbox-images`: command line tool (second crate in a Cargo workspace, own dependencies, included in the release packages) that searches the LaunchBox Games Database by title and saves a game's clear logo as `images/<name>.png`
- `launchbox-images` icon (the A.R.M.O.R.Y. shield in cyan with a picture and a download arrow), embedded into `launchbox-images.exe` on Windows together with version info

---

## Guide for Updating the Changelog

When making changes to the project:

1. Add your changes to the "Unreleased" section under the appropriate category
2. When a release starts, `release-start.yml` moves the "Unreleased" entries into a new
   `## [X.Y.Z] - date` section (`scripts/changelog.sh release`). That section becomes the
   GitHub Release notes (`scripts/changelog.sh notes`), empty categories are left out.

### Categories

- **Added**: New features
- **Changed**: Changes in existing functionality
- **Fixed**: Bug fixes
- **Deprecated**: Soon-to-be removed features
- **Removed**: Removed features
- **Security**: Security fixes
