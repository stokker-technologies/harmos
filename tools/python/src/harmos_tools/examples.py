# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build and run the guest and native sidecar examples."""

# === Imports === #

# Standard Library Imports
import argparse
import json
import subprocess
import sys

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("example", choices=["guest", "sidecar"])
    args = parser.parse_args()
    result = subprocess.run(
        [
            "cargo",
            "run",
            "--locked",
            "-p",
            "cargo-harmos",
            "--",
            args.example,
            "build",
            "-p",
            f"schematic-{args.example}",
            "--json",
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    sys.stderr.write(result.stderr)
    result.check_returncode()
    prepared = json.loads(result.stdout)
    artifacts = [
        artifact["path"]
        for artifact in prepared["artifacts"]
        if artifact["producer"] == f"schematic-{args.example}"
    ]
    if len(artifacts) != 1:
        raise RuntimeError(
            "schematic build did not select exactly one prepared artifact"
        )
    subprocess.run(
        [
            "cargo",
            "run",
            "--locked",
            "-p",
            "schematic-cli",
            "--",
            f"--{args.example}",
            artifacts[0],
        ],
        check=True,
    )


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
