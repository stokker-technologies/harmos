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
import os
import subprocess
from pathlib import Path

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
    metadata = subprocess.run(
        [
            "cargo",
            "metadata",
            "--locked",
            "--offline",
            "--no-deps",
            "--format-version",
            "1",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    target = Path(json.loads(metadata.stdout)["target_directory"])

    if args.example == "guest":
        subprocess.run(
            [
                "cargo",
                "build",
                "--locked",
                "-p",
                "schematic-guest",
                "--target",
                "wasm32-unknown-unknown",
            ],
            check=True,
        )
        (target / "artifacts").mkdir(parents=True, exist_ok=True)
        subprocess.run(
            [
                "cargo",
                "run",
                "--locked",
                "-p",
                "harmos-guest",
                "--features",
                "componentize",
                "--bin",
                "harmos-component",
                "--",
                str(target / "wasm32-unknown-unknown/debug/schematic_guest.wasm"),
                str(target / "artifacts/schematic-tools.wasm"),
            ],
            check=True,
        )
        subprocess.run(
            [
                "cargo",
                "run",
                "--locked",
                "-p",
                "schematic-cli",
                "--",
                "--guest",
                str(target / "artifacts/schematic-tools.wasm"),
            ],
            check=True,
        )
    else:
        subprocess.run(
            ["cargo", "build", "--locked", "-p", "schematic-sidecar"], check=True
        )
        suffix = ".exe" if os.name == "nt" else ""
        subprocess.run(
            [
                "cargo",
                "run",
                "--locked",
                "-p",
                "harmos-sidecar",
                "--no-default-features",
                "--features",
                "packaging",
                "--bin",
                "harmos-sidecar-export",
                "--",
                str(target / f"debug/schematic-sidecar{suffix}"),
                "examples/schematic/sidecar/Cargo.toml",
                "schematic-sidecar",
            ],
            check=True,
        )
        subprocess.run(
            [
                "cargo",
                "run",
                "--locked",
                "-p",
                "schematic-cli",
                "--",
                "--sidecar",
                str(target / f"debug/schematic-sidecar{suffix}"),
            ],
            check=True,
        )


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
