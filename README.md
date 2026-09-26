<div align="center">

# 🎯 A.R.M.O.R.Y.

### Arcade Runner Menu On Rails, Yeah!

**A launcher for light gun arcade setups: big tabs and extra-large buttons you can hit with your gun, all configured in a single YAML file.**

[![CI](https://github.com/tbreckle/armory/actions/workflows/ci.yml/badge.svg?branch=develop)](https://github.com/tbreckle/armory/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/tbreckle/armory?sort=semver&color=ff3e6c&logo=github)](https://github.com/tbreckle/armory/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-00c2a8)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-Linux%20%7C%20Windows%20%7C%20macOS-6e56cf)
![Rust](https://img.shields.io/badge/Rust-1.98-000000?logo=rust&logoColor=white)

[**Download**](https://github.com/tbreckle/armory/releases) ·
[Idea](#-what-is-this) ·
[Building](#-building-from-source) ·
[Contributing](#-contributing)

</div>

---

> [!NOTE]
> A.R.M.O.R.Y. is at the very beginning. This repository holds the project skeleton and CI/CD; features are coming.

## 🕹️ What is this?

A.R.M.O.R.Y. is a starter hub for arcade light gun setups. Put down the keyboard and mouse, pick up the gun, and shoot the game or tool you want to start.

- **Tabs and extra-large buttons**, easy to hit with a light gun from the couch
- **Starts games, emulators and tools** from one screen
- **Configured by you** in a single YAML file: tabs, buttons, labels and what each one launches
- **A single self-contained binary** for Linux, Windows and macOS, with no extra firmware or services

## 🚀 Quick start

Grab the latest release from the [Releases page](https://github.com/tbreckle/armory/releases):

| File | Platform |
|---|---|
| `armory-X.Y.Z-linux-x86_64.tar.gz` | Linux |
| `armory-X.Y.Z-windows-x86_64.zip` | Windows |
| `armory-X.Y.Z-macos-universal.tar.gz` | macOS (Apple Silicon + Intel) |

> [!TIP]
> Builds labeled `0.0.0+<commit>` are unofficial development builds.

## 🔧 Building from source

**Requirements:** Rust 1.98 (pinned in `rust-toolchain.toml`, installed automatically by rustup).

```bash
cargo run --release     # build and launch
cargo test              # unit tests
cargo clippy            # lint (warnings fail CI)
cargo fmt --check       # formatting check
```

<details>
<summary><b>🗂️ Repository layout</b></summary>

```
armory/
├── src/                   # Application source (single crate)
├── build.rs               # Injects the build version (ARMORY_VERSION)
├── scripts/               # version.sh, changelog.sh, test.sh
└── .github/workflows/     # CI, release-start, release-finish
```

</details>

## 🤝 Contributing

Contributions are welcome! The project uses **GitFlow**: branch `feature/*` off `develop` and open a PR back to `develop`. CI builds and tests on Linux, Windows and macOS on every push.

- 📖 [GITFLOW.md](GITFLOW.md): branching, versioning and releases
- ⚙️ [.github/WORKFLOWS.md](.github/WORKFLOWS.md): the CI/CD pipelines
- ✅ [.github/DEVELOPMENT_CHECKLIST.md](.github/DEVELOPMENT_CHECKLIST.md): before you open a PR
- 📝 [CHANGELOG.md](CHANGELOG.md): add your change under `## [Unreleased]`

## 📜 License

A.R.M.O.R.Y. is released under the [MIT License](LICENSE).

---

<div align="center">

**🪙 INSERT COIN TO CONTINUE 🪙**

<sub>Made with 🔥 for light gun fans.</sub>

</div>

---

## ⚖️ Legal Disclaimer

**USE AT YOUR OWN RISK**

This software is provided "AS IS" without warranty of any kind, express or implied, including but not limited to the warranties of merchantability, fitness for a particular purpose, and non-infringement.

- The developers and contributors of this project assume **no liability** for any damages, losses, or issues arising from the use, misuse, or inability to use this software
- By using this software, you acknowledge that you do so at your own risk
- This project is an independent effort and is not affiliated with or endorsed by any game, emulator or hardware manufacturer unless explicitly stated
