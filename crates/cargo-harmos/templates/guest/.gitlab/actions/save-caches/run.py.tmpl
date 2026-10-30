"""Report Observed Cache Restores Through The Repository Summary Task."""

# === Imports === #

# Standard Library Imports
import os
import runpy
import subprocess
from pathlib import Path

# Third Party Imports
# Local Imports
# Type Checking Imports

# === Public Exports === #

# === Constants === #

spawn_flags = runpy.run_path(str(Path(__file__).resolve().parents[1] / "process.py"))["spawn_flags"]

# === Types === #

# === Classes === #

# === Public Functions === #


def main():
    print("::group::Cache / Summary", flush=True)
    try:
        if module := os.environ.get("SUMMARY_MODULE"):
            subprocess.run(
                ["mise", "exec", "--", "uv", "run", "--locked", "--project", "tools/python", "python", "-m", module],
                check=True,
                creationflags=spawn_flags(),
            )
    finally:
        print("::endgroup::", flush=True)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
