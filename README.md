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
| `Tab` / `l` / `→` | Next tab |
| `Shift+Tab` / `h` / `←` | Previous tab |
| `1` / `2` / `3` | Jump to Dashboard / Results / Help |
| `j` / `k` (or `↓` / `↑`) | Navigate rows (category headers included) |
| `Space` | Toggle item — on a category header, toggle the whole category |
| `a` / `A` | Select all / none (respects the active filter) |
| `z` / `Z` | Fold the highlighted category / fold or unfold all |
| `/`     | Filter by name or path (`Enter` keeps it, `Esc` clears it) |
| `s`     | Cycle sort: default → size → name |
| `r`     | Rescan |
| `d`     | **Toggle Mode** (Dry-Run ↔ Danger) |
| `Enter` | Clean selected items (asks for confirmation) |
| `y` / `n` / `Esc` | Confirm / cancel the cleanup dialog |
| `q`     | Quit |

The scan runs in the background, so the interface appears immediately and the
footer shows `⟳ Scanning n/m` until every location has been checked.

In the Results tab each row shows the item name, the **exact path** that will be
touched, its size, and — after a run — the outcome: `would delete` (dry-run),
`deleted`, or `failed: <reason>`. On terminals at least 100 columns wide a
**Details** pane on the right shows the full path, size, file count, cleaning
mode, status and description of the highlighted row; narrower terminals show a
one-line info bar instead. After a run a summary dialog reports what was
deleted (or would be, in dry-run), how much space was freed, and any failures.

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
    category: "System"              # optional — groups rows in the Results tab
    description: "Arch Linux package cache"
    rules:
      - os: "arch"
        path: "/var/cache/pacman/pkg/"
        mode: "contents"            # optional — contents (default) | dir

  - id: "npm_cache"
    name: "NPM Cache"
    category: "Developer Tools"
    rules:
      - os: "any"
        path: "~/.npm"

  - id: "user_trash"
    name: "Trash Bin"
    category: "System"
    rules:
      - os: "any"
        path: "~/.local/share/Trash"
        keep_days: 30               # optional — only entries older than 30 days
        min_size: "10 MiB"          # optional — hide the item when smaller
```

| Field | Meaning |
|:---|:---|
| `os` | `arch`, `ubuntu`, `debian`, `fedora`, `opensuse`, or `any` |
| `path` | Absolute path; `~` expands to your home directory |
| `mode` | `contents` **empties** the folder but keeps it (safe for caches that tools expect to exist). `dir` removes the folder itself. Defaults to `contents`. |
| `category` | Optional heading used to group items in the Results tab. Defaults to `Other`. |
| `keep_days` | Optional. Only entries last modified at least this many days ago are counted and removed (top-level entries for `contents`, the target itself for `dir`/files). |
| `min_size` | Optional. Hide the item unless it is at least this large, e.g. `"10 MiB"` or `"500 KB"`. |

## 🏗️ Technical Stack

- **TUI**: [ratatui](https://github.com/ratatui-org/ratatui) + [crossterm](https://github.com/crossterm-rs/crossterm)
- **Parallelism**: [rayon](https://github.com/rayon-rs/rayon)
- **Serialization**: [serde](https://serde.rs/)

## 🤝 Contributing

Contributions are welcome! Please open an issue or submit a PR.

## 📜 License

Distributed under the MIT License. See `LICENSE` for more information.
