# TidyTUI 🧹

> **A blazingly fast, terminal-based system cleaner written in Rust.**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/built_with-Rust-d63230.svg)](https://www.rust-lang.org/)

TidyTUI is a lightweight, ncurses-style utility to scan your Linux system for accumulated "junk" (cache, logs, trash) and safely remove it. Built with performance and safety in mind, it uses parallel directory traversal to calculate sizes in milliseconds.

## 📺 Showcase

![Demo](assets/showcase/demo.gif)

## 🚀 Features

- **⚡ Blazingly Fast**: Powered by `rayon` for multi-threaded scanning and `walkdir` for efficient traversal.
- **🛡️ Safety First**: Defaults to **Dry-Run Mode**. You must explicitly toggle "Danger Mode" to delete files.
- **🐧 Distro Agnostic**: Automatically detects your OS (Arch, Ubuntu, Debian, etc.) and applies relevant cleaning rules.
- **🛠️ Configurable**: Define your own cleaning groups and paths in simple YAML.
- **📦 Zero Dependencies**: Compiles to a single binary.

## 📦 Installation

### 📦 Arch Linux (AUR)

You can install `tidytui` from the AUR using your favorite helper:

```bash
yay -S tidytui-git
# or
paru -S tidytui-git
```

### 🦀 Crates.io

If you have Rust installed, you can grab it directly from crates.io:

```bash
cargo install TidyTUI
```

### 🐧 Debian / Ubuntu (.deb)

Download the latest `.deb` from the [Releases](https://github.com/divyaprakash0426/TidyTUI/releases) page and install:

```bash
sudo apt install ./tidytui_*.deb
```

### 🎩 Fedora / RHEL / CentOS (.rpm)

Download the latest `.rpm` from the [Releases](https://github.com/divyaprakash0426/TidyTUI/releases) page and install:

```bash
sudo dnf install ./tidytui-*.rpm
```

### 🛠️ Build from Source

```bash
git clone https://github.com/divyaprakash0426/TidyTUI.git
cd TidyTUI
cargo install --path .
```

## 🎮 Usage

```bash
tidytui
```

### Controls

| Key | Action |
|:---|:---|
| `j` / `k` | Navigate items |
| `Space` | Toggle selection |
| `d`     | **Toggle Mode** (Dry-Run ↔ Danger) |
| `Enter` | Clean selected items |
| `q`     | Quit |

## ⚙️ Configuration

TidyTUI looks for `definitions.yaml` in the following locations (in order):

1. **Current Directory**: Useful for local development or portable use.
2. **XDG Config**: `~/.config/tidytui/definitions.yaml` (Recommended for `cargo` or manual installs).
3. **System Wide**: `/usr/share/tidytui/definitions.yaml` (Used by `.deb`, `.rpm`, or AUR packages).

**Example `definitions.yaml`:**

```yaml
groups:
  - id: "pacman_cache"
    name: "Pacman Cache"
    description: "Arch Linux package cache"
    rules:
      - os: "arch"
        path: "/var/cache/pacman/pkg/"

  - id: "npm_cache"
    name: "NPM Cache"
    rules:
      - os: "any"
        path: "~/.npm"
```

## 🏗️ Technical Stack

- **TUI**: [ratatui](https://github.com/ratatui-org/ratatui) + [crossterm](https://github.com/crossterm-rs/crossterm)
- **Parallelism**: [rayon](https://github.com/rayon-rs/rayon)
- **Serialization**: [serde](https://serde.rs/)

## 🤝 Contributing

Contributions are welcome! Please open an issue or submit a PR.

## 📜 License

Distributed under the MIT License. See `LICENSE` for more information.
