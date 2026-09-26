# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Project skeleton: single Rust crate, GitFlow, CI/CD (build and test on Linux, Windows and macOS universal, clippy, fmt, script tests)
- Release workflows: `release-start.yml` creates release/hotfix branches, `release-finish.yml` tags, builds and publishes GitHub Releases
- SemVer versioning via `scripts/version.sh`, injected at build time (unofficial builds are `0.0.0+<sha>`)

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
