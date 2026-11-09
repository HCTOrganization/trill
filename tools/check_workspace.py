#!/usr/bin/env python3
"""Check shared dependency and game-feature conventions (Python 3.11+)."""

from collections import defaultdict
from pathlib import Path
import re
import sys
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")


def check(root: Path) -> list[str]:
    workspace = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]
    shared = workspace["dependencies"]
    external = defaultdict(set)
    errors = []
    game_sets = {}

    for path in sorted(root.glob("*/Cargo.toml")):
        manifest = tomllib.loads(path.read_text())
        name = manifest["package"]["name"]
        label = str(path.relative_to(root))
        if not manifest.get("lints", {}).get("workspace"):
            errors.append(f"{label}: inherit workspace lints")

        sections = [manifest, *manifest.get("target", {}).values()]
        for section in sections:
            for table in DEPENDENCY_TABLES:
                for dependency, spec in section.get(table, {}).items():
                    if isinstance(spec, str):
                        spec = {"version": spec}
                    if "path" in spec:
                        continue
                    external[dependency].add(label)
                    if dependency in shared and not spec.get("workspace"):
                        errors.append(f"{label}: {dependency} must inherit its workspace dependency")

        features = manifest.get("features", {})
        if "gamesupport-all" in features:
            games = {feature for feature in features if feature.startswith("gamesupport-")} - {"gamesupport-all"}
            enabled = features["gamesupport-all"]
            if len(enabled) != len(set(enabled)) or set(enabled) != games:
                errors.append(f"{label}: gamesupport-all must list every game feature exactly once")
            game_sets[name] = games
            if name == "trill-library":
                expected_ui = {"trill-gamesupport/ui", *(f"trill-{game}?/ui" for game in games)}
                if set(features.get("ui", [])) != expected_ui:
                    errors.append(f"{label}: ui must enable editors weakly for every optional game")
                registered = set(re.findall(
                    r'"(gamesupport-[a-z0-9]+)"\s*=>',
                    (root / "trill-library/src/game.rs").read_text(),
                ))
                if registered != games:
                    errors.append(f"{label}: game features must match the registry")
            else:
                for game in games:
                    if features[game] != [f"trill-library/{game}"]:
                        errors.append(f"{label}: {game} must forward only to trill-library")

    for dependency, manifests in sorted(external.items()):
        if len(manifests) > 1 and dependency not in shared:
            errors.append(f"{dependency}: used by multiple crates; declare its version in workspace.dependencies")
    for dependency in sorted(shared.keys() - external.keys()):
        errors.append(f"workspace.dependencies: {dependency} is unused")
    if game_sets and any(games != next(iter(game_sets.values())) for games in game_sets.values()):
        errors.append("desktop, browser, and library game features must agree")
    return errors


# A game's registration crate (`trill-gamesupport-bn6`), as opposed to its
# `-dataview`/`-ui` halves or the shared `-common*` crates.
GAME_REGISTRATION = re.compile(r"trill-gamesupport-(?!common$)[a-z0-9]+")


def check_boundaries(root: Path) -> list[str]:
    errors = []
    allowed = {
        "trill-match": set(),
        "trill-net-protocol": set(),
        "trill-platform": set(),
        "trill-replay": set(),
        "trill-ui": set(),
        "trill-net": {"trill-platform", "trill-net-protocol"},
        "trill-lobby": {"trill-net", "trill-platform", "trill-net-protocol"},
        "trill-backend-mgba": {"trill-match"},
        "trill-backend-melonds": {"trill-match"},
        "trill-replay-renderer": {"trill-match"},
        "trill-gamesupport": {"trill-match"},
        "trill-gamesupport-common-dataview": {"trill-gamesupport"},
        "trill-session": {
            "trill-gamesupport", "trill-match", "trill-net", "trill-net-protocol", "trill-platform", "trill-replay",
        },
        "trill-library": {
            "trill-gamesupport", "trill-gamesupport-common-dataview", "trill-match", "trill-net-protocol",
            "trill-replay", GAME_REGISTRATION,
        },
    }
    for crate, permitted in allowed.items():
        manifest = tomllib.loads((root / crate / "Cargo.toml").read_text())
        for section in [manifest, *manifest.get("target", {}).values()]:
            for dependency, spec in section.get("dependencies", {}).items():
                if not (isinstance(spec, dict) and "path" in spec):
                    continue
                if not any(dependency == p if isinstance(p, str) else p.fullmatch(dependency) for p in permitted):
                    errors.append(f"{crate}: forbidden layer dependency {dependency}")
    for path in (root / "trill-session/src").rglob("*.rs"):
        if "std::fs::" in path.read_text():
            errors.append(f"{path.relative_to(root)}: session persistence must use a host adapter")
    for name in ("library", "engine", "link"):
        path = root / f"trill-lite-web/src/{name}.rs"
        if "thread_local!" in path.read_text():
            errors.append(f"{path.relative_to(root)}: state must be owned by an explicit host handle")
    return errors


def check_resolved_boundaries(root: Path) -> list[str]:
    errors = []
    checks = {
        "trill-gamesupport-common-dataview": {"iced", "trill-ui"},
        "trill-library": {"iced", "trill-ui", "trill-session", "trill-lobby", "trill-net"},
        "trill-session": {"iced", "trill-net", "datachannel-wrapper", "trill-signaling"},
        "trill-lobby": {"iced", "trill-library", "trill-session", "trill-match", "mgba", "melonds"},
    }
    for crate, forbidden in checks.items():
        result = subprocess.run(["cargo", "tree", "--locked", "--offline", "--no-default-features", "-p", crate,
                                 "--edges", "normal", "--prefix", "none", "--format", "{p}"],
                                cwd=root, text=True, capture_output=True)
        if result.returncode:
            errors.append(f"{crate}: cannot inspect dependency graph: {result.stderr.strip()}")
            continue
        packages = {line.split()[0] for line in result.stdout.splitlines() if line.strip()}
        for dependency in sorted(packages & forbidden):
            errors.append(f"{crate}: headless dependency graph includes {dependency}")
    return errors


def main() -> int:
    errors = check(ROOT) + check_boundaries(ROOT)
    if "--resolved" in sys.argv:
        errors.extend(check_resolved_boundaries(ROOT))
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print("Workspace dependencies, features, and architecture boundaries are consistent.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
