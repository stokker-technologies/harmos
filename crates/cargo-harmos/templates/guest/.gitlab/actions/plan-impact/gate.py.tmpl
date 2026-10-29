# Rendered by stkr project update from templates/ci/gate.py; sha256:6d687a7baad2b6c5d5a2901ced13f118e0e0f6ab93d53a6c43b1893dff64b6a3
"""Require Every Selected Validation Job To Succeed."""

# === Imports === #

# Standard Library Imports
import json
import os

# Third Party Imports
# Local Imports
# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def validate(results):
    if "impact" in results and results["impact"]["result"] != "success":
        raise ValueError("impact planning failed")
    if any(job["result"] not in ("success", "skipped") for job in results.values()):
        raise ValueError("selected CI jobs failed")


def main():
    print("::group::CI / Required", flush=True)
    try:
        validate(json.loads(os.environ["RESULTS"]))
    finally:
        print("::endgroup::", flush=True)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
