# Packaging & Publishing Guide

## 1. Arch User Repository (AUR)

### Prerequisites

- An account on [aur.archlinux.org](https://aur.archlinux.org/).
- SSH key added to your AUR account.

### Publishing `tidytui-git`

1. Check `packaging/aur/PKGBUILD` and update the `Maintainer` field.
2. Clone the AUR repository (you choose the name, e.g. `tidytui-git`):

   ```bash
   git clone ssh://aur@aur.archlinux.org/tidytui-git.git
   ```

3. Copy the `PKGBUILD` to the cloned directory:

   ```bash
   cp packaging/aur/PKGBUILD tidytui-git/
   ```

4. Generate `.SRCINFO`:

   ```bash
   cd tidytui-git
   makepkg --printsrcinfo > .SRCINFO
   ```

5. Commit and push:

   ```bash
   git add PKGBUILD .SRCINFO
   git commit -m "Initial package release"
   git push
   ```

## 2. Debian / Ubuntu (.deb)

We recommend using `cargo-deb`.

1. Install capabilities:

   ```bash
   cargo install cargo-deb
   ```

2. Build the package:

   ```bash
   cargo deb
   ```

   This will generate a `.deb` file in `target/debian/`.

## 3. Fedora / RHEL (.rpm)

We recommend using `cargo-generate-rpm`.

1. Install capabilities:

   ```bash
   cargo install cargo-generate-rpm
   ```

2. Build the package:

   ```bash
   cargo build --release
   cargo generate-rpm
   ```

## 4. Crates.io

1. Login:

   ```bash
   cargo login <token>
   ```

2. Publish:

   ```bash
   cargo publish
   ```
