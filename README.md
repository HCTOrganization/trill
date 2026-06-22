> [Since June 19, 2026, the upstream maintainer decided to add closed-source component into Tango](https://github.com/tangobattle/tango/commit/d112667637649316cfbf9e81ac50e73934477642), [and removed the source code of signaling server](https://github.com/tangobattle/tango/commit/d72d409f33328aad648c4bcdbdb4ba018c195318), making it no longer AGPL-compliant. Therefore, the compatibility of Trill with Tango can no longer be guaranteed. Use Tango with caution.
> If you have anything new and useful to every players that you'd like to implement, please let us know, instead of sending feedback to upstream.
> Hikari Calyx Tech remains steadfast in its commitment to keeping the Trill fork project open source and embracing community contributions.

# Trill

Trill is rollback netcode emulator for Mega Man Battle Network.

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

## Major differences to upstream
- Fully localized Chinese and Korean message.
- No nonsense UI triggering (since v5.0.2).
- Customizable accent color that matches your favor.
- Trill provides regional exclusive server, for lower latency in certain countries or regions, like China mainland.
- Trill provides prebuilt matchmaking server binary, to allow you host yourself easily.
- Unlike upstream, Trill accepts cosmetical-only patches for entertainment purpose, like Doronetwork / Doroexe.
- [It can coexist with v4.x version released before](https://github.com/HikariCalyx/trill).

## Roadmap
- [x] Sync upstream changes
- [x] Restore ARM64 and x86 support (WIP)
- [ ] Add NX Homebrew support
- [ ] Mobile port, instead of Winlator wrapper

## Signaling Server for Trill
You can find the source code and prebuilt binary from this repository: https://github.com/HikariCalyx/trill-matchmaking-server

## Building
Details will be added later as soon as Trill v5.x finishes its major roadmap.