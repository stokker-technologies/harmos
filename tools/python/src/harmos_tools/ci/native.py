# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Prepare Native CI Inputs"""

# === Imports === #

# Standard Library Imports
import argparse
import os
import stat
import subprocess
from pathlib import Path

# Third Party Imports
# Local Imports
from .. import csharp
from ..process import spawn_flags
from ..task.execute import command, task

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "operation", choices=("native-dependencies", "restore-permissions", "sdk-tools", "csharp-runtime", "cpp-cache", "quick-check")
    )
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    options = parser.parse_args()
    match options.operation:
        case "native-dependencies":
            command(["sudo", "apt-get", "update"])
            command(["sudo", "apt-get", "install", "-y", "autoconf", "automake", "libtool", "pkg-config"])
        case "restore-permissions":
            root = Path.cwd()
            sdk = Path("sdk/csharp").resolve()
            executable = csharp.executable(root, options.profile).resolve()
            if not executable.is_relative_to(sdk):
                raise ValueError("Restore C# Permissions: correct the executable path in its build manifest")
            executable.chmod(executable.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        case "sdk-tools":
            for sdk in os.environ["SDK_TOOLS"].split():
                if sdk not in {"cpp", "csharp", "python", "rust"}:
                    raise ValueError("Install SDK Tools: select cpp, csharp, python or rust")
                command(["mise", "trust", f"sdk/{sdk}/mise.toml"])
                command(["mise", "-C", f"sdk/{sdk}", "install", "--locked"])
        case "csharp-runtime":
            result = subprocess.run(
                ["mise", "-C", "sdk/csharp", "where", "dotnet"], check=True, capture_output=True, text=True, creationflags=spawn_flags()
            )
            stream = Path(os.environ["GITHUB_ENV"]).open("a", encoding="utf-8", newline="\n")
            try:
                stream.write(f"DOTNET_ROOT={result.stdout.strip()}\n")
            finally:
                stream.close()
        case "cpp-cache":
            Path(os.environ["VCPKG_DEFAULT_BINARY_CACHE"]).mkdir(parents=True, exist_ok=True)
        case "quick-check":
            task("format", "--check")
            task("lint", "--language", "python")
            task("check:repository")


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
