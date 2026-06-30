Install ripgrep, which provides the `rg` command.

Detect the operating system you are running on, then choose the single
method that fits it. Do not run more than one.

- Debian / Ubuntu:  apt-get install -y ripgrep
- Fedora:           dnf install -y ripgrep
- Arch:             pacman -S --noconfirm ripgrep
- macOS:            brew install ripgrep
- Windows:          winget install --silent --accept-package-agreements --accept-source-agreements BurntSushi.ripgrep.MSVC

If the preferred method is unavailable, fall back to a reasonable
alternative for the platform (for example `cargo install ripgrep` when a
Rust toolchain is present, or `choco install ripgrep -y` on Windows).

When finished, verify the install by running `rg --version`. If it does
not report a version, the install did not work; fix it before stopping.
