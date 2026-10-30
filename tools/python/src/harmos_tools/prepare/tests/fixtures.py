# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Join Product And Tests Fixture Manifests Without Compilation"""

# === Imports === #

# Standard Library Imports
import argparse
import json
import os
from pathlib import Path
from typing import Any

# Third Party Imports

# Local Imports
from ...build import fixtures as staging
from ...build import paths
from ...build.fixtures import ROOT
from ...test import archive

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)

    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--focused-messages", type=Path)
    parser.add_argument("--libraries", help="comma-separated library target names represented in focused Cargo messages")
    parser.add_argument("--inspection-messages", help="comma-separated Cargo JSON captures that produced inspected artifacts")
    parser.add_argument("--inspections", help="comma-separated native executable target names to inspect")

    options = parser.parse_args()
    if options.focused_messages is None:
        prepare(options.profile)
        return

    if not options.libraries:
        parser.error("--focused-messages requires --libraries")
    inspection_messages = [Path(path) for path in options.inspection_messages.split(",")] if options.inspection_messages else []
    inspections = options.inspections.split(",") if options.inspections else []
    if bool(inspection_messages) != bool(inspections):
        parser.error("--inspection-messages and --inspections must be supplied together")
    prepare_focused(options.focused_messages, options.libraries.split(","), inspection_messages, inspections)


def prepare_focused(
    messages: Path, requested: list[str], inspection_messages: list[Path] | None = None, inspections: list[str] | None = None
) -> Path:
    """Export Hash-Stamped Focused Libraries And Optional Inspection Artifacts."""
    required = set(requested)
    invalid_names = any(not _valid_target_name(name) for name in required)
    if not required or invalid_names:
        raise ValueError("focused library names must be nonempty Cargo target names")
    libraries: dict[str, dict[str, str]] = {}
    dependencies: set[Path] = set()
    events = _compiler_artifacts(_cargo_events([messages]))
    for event in events:
        target = event.get("target", {})
        name = target.get("name")
        if name not in required or "lib" not in target.get("kind", []):
            continue
        filenames = [Path(path) for path in event.get("filenames", [])]
        emitted = [path for path in filenames if path.suffix == ".rlib"]
        if not emitted:
            continue
        library = emitted[0].resolve()
        dependencies.add(library.parent if library.parent.name == "deps" else library.parent / "deps")
        libraries[name] = {"path": str(library), "sha256": staging.digest(library)}
    if set(libraries) != required or len(dependencies) != 1:
        raise RuntimeError(f"focused Cargo messages did not produce the requested libraries unambiguously: {sorted(libraries)}")

    dependency = next(iter(dependencies))
    dependency_directory = str(dependency)
    metadata = {"directory": dependency_directory, "dependencies": dependency_directory, "libraries": libraries}
    inspection_messages = inspection_messages or []
    inspections = inspections or []
    inspection_sources = _inspection_artifacts(inspection_messages, inspections)
    inspection_paths = _stage_inspections(inspection_sources)
    metadata["inspections"] = {name: str(path) for name, path in inspection_paths.items()}
    metadata["inspection_sha256"] = {name: staging.digest(path) for name, path in inspection_paths.items()}
    for name, library in libraries.items():
        library_path = Path(library["path"])
        digest = staging.digest(library_path)
        staging.verify(digest, library["sha256"], f"focused library changed while stamping: {name}")
    for name, path in inspection_paths.items():
        staging.verify(staging.digest(path), metadata["inspection_sha256"][name], f"focused inspection changed while stamping: {name}")
    manifest = ROOT / "target" / "build" / "focused-fixtures" / "fixtures.json"
    manifest.parent.mkdir(parents=True, exist_ok=True)
    manifest.write_text(json.dumps({"schema": 1, "metadata": metadata}, indent=2) + "\n", encoding="utf-8")
    print(manifest)
    return manifest


def _inspection_artifacts(messages: list[Path], requested: list[str]) -> dict[str, Path]:
    """Resolve Native Executable Inspections Across Target Captures."""
    if not requested:
        return {}
    if len(set(requested)) != len(requested):
        raise ValueError("focused inspection names must be unique")
    if any(not _valid_target_name(name) for name in requested):
        raise ValueError("focused inspection names must be Cargo target names")
    events = _cargo_events(messages)

    resolved: dict[str, Path] = {}
    for name in requested:
        target_name = name.replace("-", "_")
        matches = _inspection_matches(events, target_name)
        if len(matches) != 1:
            raise RuntimeError(f"focused Cargo messages did not produce one inspection artifact for {name}: {matches}")
        resolved[name] = matches[0]
    return resolved


def _stage_inspections(sources: dict[str, Path]) -> dict[str, Path]:
    """Copy Inspections Under Preserved Build Outputs Before Later Compiler Preflights."""
    directory = ROOT / "target" / "build" / "focused-fixtures" / "inspection"
    staged = {}
    copied_files = {}
    for name, source in sources.items():
        destination = directory / f"{name}{source.suffix}"
        source_digest = staging.digest(source)
        copied_digest = staging.copy(source, destination, copied_files)
        staging.verify(copied_digest, source_digest, f"focused inspection changed while staging: {name}")
        staged[name] = destination.resolve()
    return staged


def _cargo_events(messages: list[Path]) -> list[dict[str, Any]]:
    """Read Cargo JSON Captures Only After Every Invocation Reports Success."""
    events = []
    for message_path in messages:
        events.extend(_successful_capture(message_path))
    return events


def _successful_capture(message_path: Path) -> list[dict[str, Any]]:
    lines = message_path.read_text(encoding="utf-8").splitlines()
    captured = [json.loads(line) for line in lines if line.startswith("{")]
    completions = [event for event in captured if event.get("reason") == "build-finished"]
    if not completions:
        raise RuntimeError(f"Cargo capture has no build-finished event: {message_path}")
    if any(event.get("success") is not True for event in completions):
        raise RuntimeError(f"Cargo capture reports an unsuccessful build: {message_path}")
    return captured


def _compiler_artifacts(events: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [event for event in events if _is_build_artifact(event)]


def _is_build_artifact(event: dict[str, Any]) -> bool:
    is_artifact = event.get("reason") == "compiler-artifact"
    is_test = event.get("profile", {}).get("test") is True
    return is_artifact and not is_test


def _inspection_matches(events: list[dict[str, Any]], target_name: str) -> list[Path]:
    matches = []
    matching_events = filter(lambda event: _is_shipping_target(event, target_name), events)
    for event in matching_events:
        matches.extend(_inspection_paths(event))
    return matches


def _is_shipping_target(event: dict[str, Any], target_name: str) -> bool:
    target = event.get("target", {})
    name = target.get("name", "")
    is_test = event.get("profile", {}).get("test") is True
    return name.replace("-", "_") == target_name and not is_test


def _inspection_paths(event: dict[str, Any]) -> list[Path]:
    executable = event.get("executable")
    if executable:
        return [Path(executable).resolve()]
    return []


def _valid_target_name(name: str) -> bool:
    normalized = name.replace("_", "").replace("-", "")
    return normalized.isalnum()


def prepare(profile: str) -> None:
    """Join Independently Exported Product And Tests Inputs Without Compilation."""
    export = paths.output(ROOT, "build", profile)
    fixtures = export / "fixtures"
    stamp = json.loads((fixtures / "stamp.json").read_text(encoding="utf-8"))
    staging.verify(stamp["profile"], profile, "test fixtures belong to another profile")
    staging.verify(stamp["target"], paths.target(), "test fixtures belong to another native target")
    catalog = json.loads((export / "product/examples/catalog.json").read_text(encoding="utf-8"))
    files = {**stamp["files"], **catalog["files"]}
    for relative, expected in files.items():
        staging.verify(staging.digest(ROOT / relative), expected, f"fixture bytes differ from their producer: {relative}")

    metadata = stamp["metadata"]
    for key in ("artifacts", "builds", "libraries"):
        metadata[key].update({name: os.path.relpath(export / path, fixtures) for name, path in catalog[key].items()})
    metadata["build_roots"].update(catalog["build_roots"])
    metadata["dependencies"] = "../product/examples/libraries"
    metadata["executables"] = [metadata["artifacts"][name] for name in ("cargo-harmos", "schematic-cli", "schematic-sidecar")]
    # Integration Inspects Immutable Copies Without Running Product Hosts.
    for name in ("schematic-sidecar",):
        source = export / catalog["artifacts"][name]
        destination = fixtures / "inspection" / source.name
        staging.copy(source, destination, files)
        metadata["inspections"][name] = destination.relative_to(fixtures).as_posix()
    archive.stamp(ROOT, profile, sorted(files), metadata)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
