# Rendered by stkr project update from templates/python/build/run.py; sha256:c9d24130d6ef63b6c7cd1dfef390373ac4d7ffd092022b47ff43f9b42a94d7de
# ***
#
# __PRODUCT_NAME__
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build Packageable Products Without Test Harnesses Or Fixtures"""

# === Imports === #

# Standard Library Imports
import argparse
import json
import shutil
import subprocess
from pathlib import Path

# Third Party Imports
# Local Imports
from .. import temporary
from . import paths

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def rust(directory: Path, profile: str, native: str | None = None) -> None:
    destination = paths.output(directory, "build", profile, native)
    messages = destination / "product/compiler.json"
    messages.parent.mkdir(parents=True, exist_ok=True)
    command = ["cargo", "build", "--locked", "--workspace", "--profile", profile, "--message-format=json-render-diagnostics"]
    with messages.open("w", encoding="utf-8") as output:
        subprocess.run(command, cwd=directory, stdout=output, check=True)

    with temporary.directory() as staging:
        for name in ("bin", "lib"):
            (staging / name).mkdir()
        for line in messages.read_text(encoding="utf-8").splitlines():
            event = json.loads(line)
            if event.get("reason") != "compiler-artifact" or event.get("profile", {}).get("test"):
                continue
            manifest = Path(event["manifest_path"])
            if not manifest.resolve().is_relative_to(directory.resolve()) or "target" in manifest.relative_to(directory).parts:
                continue
            executable = event.get("executable")
            if executable and event.get("target", {}).get("kind") == ["bin"]:
                source = Path(executable)
                shutil.copy2(source, staging / "bin" / source.name)
            elif any(
                kind in event.get("target", {}).get("kind", []) for kind in ("lib", "rlib", "cdylib", "dylib", "staticlib", "proc-macro")
            ):
                for filename in event.get("filenames", []):
                    source = Path(filename)
                    shutil.copy2(source, staging / "lib" / source.name)
        for name in ("bin", "lib"):
            paths.replace(directory, staging / name, destination / name, origin=staging)


def main() -> None:
    parser = argparse.ArgumentParser(description="Produce Canonical Build Outputs")
    parser.add_argument("language", choices=("rust", "python", "typescript"))
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--project", default=".")
    parser.add_argument("--target")
    options = parser.parse_args()
    directory = Path.cwd()
    project = directory / options.project
    if not project.resolve().is_relative_to(directory.resolve()):
        raise ValueError("build project escapes the repository")
    destination = paths.output(directory, "build", options.profile, options.target)
    if options.language == "rust":
        rust(directory, options.profile, options.target)
    elif options.language == "python":
        subprocess.run(
            ["uv", "build", "--project", str(project), "--wheel", "--out-dir", str(destination / "python")],
            cwd=directory,
            check=True,
        )
    else:
        paths.replace(directory, project / "dist", destination / "web")


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
