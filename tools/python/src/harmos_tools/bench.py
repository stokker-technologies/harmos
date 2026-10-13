# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Measure the deterministic simulation oracle on every platform."""

# === Imports === #

# Standard Library Imports
import argparse
import subprocess

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
    parser.add_argument("--prepare", action="store_true")
    args = parser.parse_args()
    command = [
        "cargo",
        "nextest",
        "run",
        "--release",
        "--locked",
        "-p",
        "harmos",
        "--features",
        "simulation,plugin-sidecar",
        "--test",
        "sim",
        "sim_ac001_double_run_identity",
    ]
    if args.prepare:
        command.append("--no-run")
    result = subprocess.run(command)
    raise SystemExit(result.returncode)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
