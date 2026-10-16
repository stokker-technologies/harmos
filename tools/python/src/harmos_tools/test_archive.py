# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build And Verify Compatible Nextest Archives For Native CI Jobs."""

# === Imports === #

# Standard Library Imports
import argparse
import hashlib
import json
import os
from contextlib import nullcontext
from dataclasses import asdict, replace
from pathlib import Path

# Third Party Imports
# Local Imports
from . import evidence, release

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ARCHIVES_VARIABLE = "STKR_TEST_ARCHIVES"
ENVIRONMENT_KEYS = (
    "RUSTC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "RUSTDOCFLAGS",
    "CARGO_BUILD_TARGET",
    "CARGO_BUILD_RUSTFLAGS",
)

# === Types === #

# === Classes === #

# === Public Functions === #


def build(
    root: Path,
    directory: Path,
    all_features: bool,
    coverage: bool,
    config_file: Path | None = None,
) -> None:
    policy = evidence.parse_policy(evidence._config(root))
    archive_configuration = _configuration(root, config_file)
    directory.mkdir(parents=True, exist_ok=True)
    measurement = (
        evidence.Lock(evidence._state(root) / "coverage-rust.lock", wait_seconds=30)
        if coverage
        else nullcontext()
    )
    with evidence.Lock(evidence._state(root) / "test-archive.lock"), measurement:
        built = set()
        for configuration in policy.rust:
            selection = configuration.selection
            if not isinstance(selection, evidence.RustSelection):
                continue
            if configuration.name == "default" and all_features:
                selection = replace(selection, all_features=True)
            # Category Filters Narrow Execution, Never The Compatible Archive Contents.
            selection = replace(selection, targets=(), filter=None)
            key = _key(selection, coverage)
            if key in built:
                continue
            built.add(key)
            path = directory / f"{key}.tar.zst"
            receipt = path.with_suffix(".json")
            receipt.unlink(missing_ok=True)
            identity = _identity(root, selection, coverage, archive_configuration)
            environment = dict(os.environ)
            if coverage:
                target = root / "target/validation/cargo/coverage"
                environment.update(
                    CARGO_TARGET_DIR=str(target),
                    CARGO_LLVM_COV_TARGET_DIR=str(target),
                    CARGO_LLVM_COV_BUILD_DIR=str(target),
                )
            command = [
                "cargo",
                *(
                    [
                        "llvm-cov",
                        "nextest-archive",
                        *(
                            ["--coverage-host-only"]
                            if selection.coverage_host_only
                            else []
                        ),
                    ]
                    if coverage
                    else ["nextest", "archive"]
                ),
                *evidence.cargo_selection(selection),
                *(
                    ["--config-file", str(root / archive_configuration["path"])]
                    if archive_configuration
                    else []
                ),
                "--archive-file",
                str(path),
            ]
            if not evidence._execute(command, root, environment):
                raise evidence.EvidenceError("nextest archive build failed")
            if (
                _identity(root, selection, coverage, _configuration(root, config_file))
                != identity
            ):
                raise evidence.EvidenceError(
                    "archive inputs changed during compilation"
                )
            receipt.write_text(
                json.dumps({"identity": identity, "sha256": _hash(path)}, indent=2)
            )


def consume(
    root: Path, selection: evidence.RustSelection, coverage: bool
) -> Path | None:
    directory = os.environ.get(ARCHIVES_VARIABLE)
    if directory is None:
        return None
    path = Path(directory) / f"{_key(selection, coverage)}.tar.zst"
    if not path.is_absolute():
        path = root / path
    receipt = path.with_suffix(".json")
    if (
        not path.is_file()
        or path.is_symlink()
        or not receipt.is_file()
        or receipt.is_symlink()
    ):
        raise evidence.EvidenceError("required compatible nextest archive is absent")
    try:
        value = json.loads(receipt.read_text())
    except (ValueError, OSError) as error:
        raise evidence.EvidenceError(
            f"invalid nextest archive receipt: {error}"
        ) from error
    if not isinstance(value, dict) or not isinstance(value.get("identity"), dict):
        raise evidence.EvidenceError("invalid nextest archive receipt")
    configuration = value["identity"].get("archive_configuration")
    if configuration is not None:
        if not isinstance(configuration, dict) or not isinstance(
            configuration.get("path"), str
        ):
            raise evidence.EvidenceError("invalid nextest archive configuration")
        configuration = _configuration(root, Path(configuration["path"]))
    if value.get("identity") != _identity(root, selection, coverage, configuration):
        raise evidence.EvidenceError("nextest archive inputs differ")
    if value.get("sha256") != _hash(path):
        raise evidence.EvidenceError("nextest archive bytes differ")
    return path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path)
    parser.add_argument(
        "--output",
        type=Path,
        default=Path(os.environ.get(ARCHIVES_VARIABLE, "target/test-archives")),
    )
    parser.add_argument("--all-features", action="store_true")
    parser.add_argument("--coverage", action="store_true")
    parser.add_argument(
        "--config-file",
        type=Path,
        help="product-owned Nextest configuration transferred with the archive",
    )
    options = parser.parse_args()
    root = (options.root or evidence._project_root(Path.cwd())).resolve()
    directory = (
        options.output if options.output.is_absolute() else root / options.output
    )
    build(
        root,
        directory,
        options.all_features,
        options.coverage or os.environ.get(evidence.COVERAGE_VARIABLE) == "1",
        options.config_file,
    )
    return 0


# === Private Functions === #


def _selection(selection: evidence.RustSelection) -> dict:
    value = asdict(selection)
    for key in ("categories", "targets", "filter"):
        value.pop(key, None)
    return json.loads(json.dumps(value))


def _key(selection: evidence.RustSelection, coverage: bool) -> str:
    value = json.dumps([_selection(selection), coverage], sort_keys=True).encode()
    return hashlib.sha256(value).hexdigest()[:24]


def _identity(
    root: Path,
    selection: evidence.RustSelection,
    coverage: bool,
    archive_configuration: dict | None = None,
) -> dict:
    inputs = release.inputs(root)
    revision = evidence.revision(root)
    if inputs.get("source_tree") is None or revision is None:
        raise evidence.EvidenceError("nextest archives require clean committed source")
    commands = {
        "rustc": ["rustc", "-vV"],
        "cargo": ["cargo", "--version"],
        "nextest": ["cargo", "nextest", "--version"],
    }
    if coverage:
        commands["llvm-cov"] = ["cargo", "llvm-cov", "--version"]
    tools = {name: evidence._query(root, command) for name, command in commands.items()}
    if any(value is None for value in tools.values()):
        raise evidence.EvidenceError("nextest archive tool identity is unavailable")
    return {
        "revision": revision,
        "inputs": inputs,
        "selection": _selection(selection),
        "coverage": coverage,
        "build_profile": selection.native_profile,
        "archive_configuration": archive_configuration,
        "host": [evidence._platform(), evidence._architecture()],
        "tools": tools,
        "environment": {
            name: os.environ.get(name)
            for name in sorted(
                set(ENVIRONMENT_KEYS)
                | {
                    name
                    for name in os.environ
                    if name.startswith(("CARGO_PROFILE_", "CARGO_TARGET_"))
                    and name != "CARGO_TARGET_DIR"
                }
            )
        },
        "configuration": {
            name: _hash(path) if path.is_file() else None
            for name, path in (
                ("project-cargo", root / ".cargo/config.toml"),
                ("project-cargo-legacy", root / ".cargo/config"),
                (
                    "home-cargo",
                    Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
                    / "config.toml",
                ),
                (
                    "home-cargo-legacy",
                    Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
                    / "config",
                ),
            )
        },
    }


def _configuration(root: Path, path: Path | None) -> dict | None:
    if path is None:
        return None
    root = root.resolve()
    path = path if path.is_absolute() else root / path
    if ".." in path.parts or not path.is_relative_to(root):
        raise evidence.EvidenceError("archive configuration must be inside the project")
    for component in (path, *path.parents):
        if component == root:
            break
        if component.is_symlink():
            raise evidence.EvidenceError("archive configuration must not use symlinks")
    if not path.is_file():
        raise evidence.EvidenceError("archive configuration must be a regular file")
    return {"path": path.relative_to(root).as_posix(), "sha256": _hash(path)}


def _hash(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
