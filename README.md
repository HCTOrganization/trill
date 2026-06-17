# Trill

Trill is a rollback netcode enabled emulator for Battle Network games.

## Supported games

| Name                                                  | Gameplay support            | Save viewer support                                |
| ----------------------------------------------------- | --------------------------- | -------------------------------------------------- |
| Mega Man Battle Network 6: Cybeast Falzar (US)        | ✅ Works great!             | 🤷 Folder, NaviCust                                |
| Mega Man Battle Network 6: Cybeast Gregar (US)        | ✅ Works great!             | 🤷 Folder, NaviCust                                |
| Rockman EXE 6: Dennoujuu Falzer (JP)                  | ✅ Works great!             | 🤷 Folder, NaviCust, Patch Cards                   |
| Rockman EXE 6: Dennoujuu Glaga (JP)                   | ✅ Works great!             | 🤷 Folder, NaviCust, Patch Cards                   |
| Mega Man Battle Network 5: Team Protoman (US)         | ✅ Works great!             | 🤷 Folder, NaviCust, Patch Cards, Auto Battle Data |
| Mega Man Battle Network 5: Team Colonel (US)          | ✅ Works great!             | 🤷 Folder, NaviCust, Patch Cards, Auto Battle Data |
| Rockman EXE 5: Team of Blues (JP)                     | ✅ Works great!             | 🤷 Folder, NaviCust, Patch Cards, Auto Battle Data |
| Rockman EXE 5: Team of Colonel (JP)                   | ✅ Works great!             | 🤷 Folder, NaviCust, Patch Cards, Auto Battle Data |
| Rockman EXE 4.5: Real Operation (JP)                  | ✅ Works great!             | ✅ Navi, Folder                                    |
| Mega Man Battle Network 4: Blue Moon (US)             | ✅ Works great!             | 🤷 Folder, NaviCust, Patch Cards, Auto Battle Data |
| Mega Man Battle Network 4: Red Sun (US)               | ✅ Works great!             | 🤷 Folder, NaviCust, Patch Cards, Auto Battle Data |
| Rockman EXE 4: Tournament Blue Moon (Rev 1 only) (JP) | ✅ Works great!             | 🤷 Folder, NaviCust, Patch Cards, Auto Battle Data |
| Rockman EXE 4: Tournament Red Sun (Rev 1 only) (JP)   | ✅ Works great!             | 🤷 Folder, NaviCust, Patch Cards, Auto Battle Data |
| Megaman Battle Network 3: Blue (US)                   | ✅ Works great!             | 🤷 Folder, NaviCust                                |
| Megaman Battle Network 3: White (US)                  | ✅ Works great!             | 🤷 Folder, NaviCust                                |
| Battle Network Rockman EXE 3: Black (Rev 1 only) (JP) | ✅ Works great!             | 🤷 Folder, NaviCust                                |
| Battle Network Rockman EXE 3 (Rev 1 only) (JP)        | ✅ Works great!             | 🤷 Folder, NaviCust                                |
| Megaman Battle Network 2 (US)                         | 🤷 Works, with minor issues | 🤷 Folder                                          |
| Battle Network Rockman EXE 2 (AdColle only) (JP)      | 🤷 Works, with minor issues | 🤷 Folder                                          |
| Megaman Battle Network (US)                           | 🤷 Works, with minor issues | 🤷 Folder                                          |
| Battle Network Rockman EXE (JP)                       | 🤷 Works, with minor issues | 🤷 Folder                                          |

## Building

These steps mirror what the release workflow (`.github/workflows/release.yaml`) does, adapted for a local checkout.

### Clone with submodules

The build depends on vendored C libraries (`libdatachannel`, `mgba`, etc.) that live in git submodules, so clone recursively:

```sh
git clone --recursive https://github.com/<owner>/trill5.git
cd trill5
```

If you already cloned without `--recursive`:

```sh
git submodule update --init --recursive
```

### Common prerequisites (all platforms)

- **Rust** (stable toolchain) via [rustup](https://rustup.rs/)
- **Protocol Buffers compiler** (`protoc`) on your `PATH`
- **CMake** and a **C/C++ compiler** (for the bundled C libraries)
- **Python 3** with the build-script dependencies:

  ```sh
  pip install semver==3.0.0-dev3 toml mako
  ```

> Note: the release workflow sets `CMAKE_POLICY_VERSION_MINIMUM=3.5`. If CMake errors out on the bundled deps, export the same:
>
> ```sh
> export CMAKE_POLICY_VERSION_MINIMUM=3.5   # Windows (cmd): set CMAKE_POLICY_VERSION_MINIMUM=3.5
> ```

### Quick development build

For day-to-day development you only need to compile the `tango` binary; you can skip the packaging steps below:

```sh
cargo build --bin tango              # debug build
cargo run --bin tango                # build and run
cargo build --bin tango --release    # optimized, fast to rebuild
```

Shipped binaries use the `release-dist` profile (thin LTO, `codegen-units = 1`) for extra runtime performance at the cost of longer compiles:

```sh
cargo build --bin tango --profile release-dist
```

### Platform-specific full builds (packaging)

The platform scripts build the `release-dist` binary, add an icon, bundle `ffmpeg`, and produce a distributable artifact in `dist/`. Run them from the repo root.

#### Windows (`win/build.sh`)

Produces `dist/trill-x86_64-windows.exe`.

Additional prerequisites:

- Run from a shell with MSVC tooling on `PATH` (e.g. a "Developer" shell, or after sourcing `vcvars`) so `cl.exe`/`link.exe` and the Windows SDK are available. The script also reorders `PATH` so MSVC's `link.exe` wins over the MSYS one under Git Bash.
- The `x86_64-pc-windows-msvc` Rust target: `rustup target add x86_64-pc-windows-msvc`
- **NASM** (required by `aws-lc-sys`)
- **ImageMagick** (`magick`) for icon generation
- **NSIS** (`makensis`) for the installer; the script downloads the Nsis7z plugin automatically
- `curl`, `unzip`, `7z` available on `PATH`

```sh
./win/build.sh
```

Other Windows targets have their own scripts: `win/build_i686.sh` and `win/build_arm64.sh` (add the matching `i686-pc-windows-msvc` / `aarch64-pc-windows-msvc` target).

#### macOS (`macos/build.sh`)

Produces `dist/trill-macos.dmg` as a universal (arm64 + x86_64) binary.

Additional prerequisites:

- Both Rust targets: `rustup target add aarch64-apple-darwin x86_64-apple-darwin`
- Extra Python deps (a venv is recommended):

  ```sh
  python3 -m venv ./macos/venv
  source ./macos/venv/bin/activate
  python3 -m pip install semver==3.0.0-dev3 toml dmgbuild pyobjc-framework-Quartz mako
  ```

```sh
./macos/build.sh
```

#### Linux x86_64 (`linux/build.sh`)

Produces `dist/trill-x86_64-linux.AppImage`.

Additional prerequisites (Debian/Ubuntu package names from the workflow):

```sh
sudo apt-get install -y alsa build-essential clang cmake curl fuse git \
  libnss3 librust-atk-dev librust-gdk-pixbuf-dev librust-gdk-sys-dev \
  librust-pango-dev libsdl2-dev pkgconf wget
```

Then:

```sh
./linux/build.sh
```

For ARM64, use `linux/build_arm64.sh` (add the `aarch64-unknown-linux-gnu` target).

> The packaging scripts download `ffmpeg`, ANGLE (Windows), and `appimagetool` (Linux) at build time, so an internet connection is required for the full build.