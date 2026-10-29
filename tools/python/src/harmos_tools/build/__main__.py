# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Build Canonical Shipping Products"""

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path


from .. import temporary
from . import cargo, fixtures, paths
from .fixtures import ROOT, TARGET, SCHEMATIC


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    options = parser.parse_args()
    with temporary.directory() as scratch:
        product(options.profile, scratch)


def product(profile: str, scratch: Path) -> None:
    os.environ["CARGO_TARGET_DIR"] = str(TARGET)
    export = paths.output(ROOT, "build", profile)
    messages = export / "product/compiler.json"
    messages.parent.mkdir(parents=True, exist_ok=True)
    arguments = ["--locked", "--workspace", "--profile", profile]
    cargo.clippy(ROOT, arguments)
    with messages.open("w", encoding="utf-8") as output:
        subprocess.run(
            ["cargo", "build", *arguments, "--message-format=json-render-diagnostics"],
            cwd=ROOT,
            stdout=output,
            stderr=sys.stderr,
            stdin=sys.stdin,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
            check=True,
        )
    with temporary.directory() as staging:
        for name in ("bin", "lib"):
            (staging / name).mkdir()
        for event in cargo.artifacts(ROOT, messages):
            executable = event.get("executable")
            kinds = event.get("target", {}).get("kind", [])
            if executable and kinds == ["bin"]:
                source = Path(executable)
                shutil.copy2(source, staging / "bin" / source.name)
            elif any(kind in kinds for kind in ("lib", "rlib", "cdylib", "dylib", "staticlib", "proc-macro")):
                for filename in event.get("filenames", []):
                    source = Path(filename)
                    shutil.copy2(source, staging / "lib" / source.name)
        for name in ("bin", "lib"):
            paths.replace(ROOT, staging / name, export / name, origin=staging)
    cargo.record_target(ROOT, messages, profile)

    examples = export / "product/examples"
    shutil.rmtree(examples, ignore_errors=True)
    examples.mkdir(parents=True)
    tool = export / "bin" / ("cargo-harmos.exe" if os.name == "nt" else "cargo-harmos")
    native = fixtures.capture("schematic", SCHEMATIC.parent, profile, ["-p", "schematic-cli", "-p", "schematic-sidecar"], scratch)
    files: dict[str, str] = {}
    # Export The Actual Shipping Libraries While Producer-Local Compiler Paths Still Exist.
    libraries = fixtures.libraries(fixtures.messages(native), files, examples)
    artifacts = {"cargo-harmos": tool.relative_to(export).as_posix()}
    files[tool.relative_to(ROOT).as_posix()] = fixtures.digest(tool)
    cli = fixtures.executable(fixtures.messages(native), "schematic-cli")
    fixtures.copy(cli, export / "bin" / cli.name, files)
    artifacts["schematic-cli"] = f"bin/{cli.name}"

    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--locked", "--format-version", "1"],
            cwd=ROOT,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
            stdin=sys.stdin,
            stderr=sys.stderr,
        )
    )
    platform = {"linux": "linux-x86_64", "darwin": "macos-x86_64", "win32": "win32"}[sys.platform]
    package = next(package for package in metadata["packages"] if package["name"] == f"protoc-bin-vendored-{platform}")
    protoc = Path(package["manifest_path"]).parent / "bin" / ("protoc.exe" if os.name == "nt" else "protoc")

    builds = {}
    for package in ("schematic-sidecar", "schematic-guest"):
        capture = (
            fixtures.capture(package, SCHEMATIC.parent, profile, ["-p", package, "--target", "wasm32-unknown-unknown"], scratch)
            if package == "schematic-guest"
            else native
        )
        backend = "sidecar" if package == "schematic-sidecar" else "guest"
        built, receipt = fixtures.collect(tool, SCHEMATIC, package, backend, profile, capture, scratch)
        artifact, staged = fixtures.stage_build(built, receipt, package, files, examples)
        source = examples / artifact
        filename = source.name if package == "schematic-sidecar" else "schematic-guest.wasm"
        destination = export / "bin/plugins" / filename
        fixtures.copy(source, destination, files)
        artifacts[package] = destination.relative_to(export).as_posix()
        builds[package] = f"product/examples/{staged}"
        if package == "schematic-sidecar":
            # Consume Producer-Local Schemas Before The Guest Compiler Preflight.
            fixtures.generate_python(tool, export / builds[package], protoc, files)

    catalog = {
        "artifacts": artifacts,
        "builds": builds,
        "build_roots": {name: str(ROOT) for name in builds},
        "libraries": {name: f"product/examples/{path}" for name, path in libraries.items()},
        "files": files,
    }
    (examples / "catalog.json").write_text(json.dumps(catalog, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
