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
