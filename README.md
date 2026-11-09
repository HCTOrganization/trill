# Trill

Trill provides rollback netplay for Mega Man Battle Network, with desktop
and browser frontends, save editors, and replay playback and video export.

## Build and run

Install Rust stable, a C/C++ compiler, CMake, Ninja, and Protocol Buffers
(`protoc`). Native dependencies include emulator and networking libraries
built from source. Platform package lists and release setup are in
[the native CI workflow](.github/workflows/ci.yaml).

Emulator cores, networking, and the crates this project used to consume
from separate repositories all live under `vendor/` and build as workspace
members. Only libdatachannel and Mbed TLS remain submodules, so clone
recursively (or initialize them afterwards):

```sh
git clone --recurse-submodules <repository-url>
git submodule update --init --recursive   # in an existing checkout
```

```sh
cargo run --release --bin trill
```

The desktop app enables all supported games by default. For a build with
only one game, disable the defaults explicitly:

```sh
cargo run --release --bin trill --no-default-features --features gamesupport-bn6
```

Use `cargo build --release --bin trill` to build without launching.
Platform packaging scripts in `linux/`, `macos/`, and `win/` use the
optimized `release-dist` profile. The browser has a separate toolchain
and server requirements; see [its build instructions](trill-lite-web/README.md).

## Find your way around

| Area | Location | Responsibility |
| --- | --- | --- |
| Desktop app | `trill` | Application state, tabs, native input/audio/video, updates |
| Browser app | `trill-lite-web` | Browser UI, canvas, audio worklet, IndexedDB |
| Library | `trill-library` | Game registry, catalogs, shared loadout preparation and stats persistence |
| Sessions | `trill-session` | Single-player, optional netplay, training, replay drivers |
| Match engine | `trill-match` | Backend interfaces, rollback coordination, audio, telemetry |
| Emulators | `trill-backend-mgba`, `trill-backend-melonds` | GBA and DS implementations |
| Vendored crates | `vendor` | Emulator bindings, transport, encoding, and the crates formerly extracted from this repository |
| Matchmaking | `trill-lobby`, `trill-net-protocol` | Lobby state and wire messages |
| Transport / platform | `trill-net`, `trill-platform` | Connections, reconnect, portable tasks and timers |
| Replays | `trill-replay`, `trill-replay-renderer` | Recording format and video export |
| Game interface | `trill-gamesupport` | ROM identity, save/editor contracts, engine hooks |
| Per-game support | `trill-gamesupport-<game>` | Registration and game-specific engine integration |
| Save and ROM data | `trill-gamesupport-<game>-dataview` | Binary layouts, parsing, assets |
| Save editors | `trill-gamesupport-<game>-ui` | Per-game editor presentation |
| Shared game support | `trill-gamesupport-common*` | Telemetry, parsing, editor state and controls |
| Shared UI | `trill-ui` | Styles, widgets, animation, copy feedback |

Per-game crates join the workspace through path dependencies. A plain
root build uses `default-members`, excluding the browser-only target.
Use explicit packages when checking portable code; `--workspace` also
selects the browser, which cannot compile for a native target. Crates in
`vendor` join the workspace the same way, so `--workspace` builds them
too.

[ARCHITECTURE.md](ARCHITECTURE.md) traces session startup, netplay, and
replay processing. [CONTRIBUTING.md](CONTRIBUTING.md) documents checks and
how to add a game or dependency.

## License

GPL-3.0-or-later — see [LICENSE](LICENSE) and [CREDITS.md](CREDITS.md).
Trill links GPL-licensed [melonDS](https://melonds.kuribo64.net/).
The game-support crates in this repository use the same license.
