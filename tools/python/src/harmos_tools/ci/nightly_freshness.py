# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Run CI Nightly Freshness."""

# === Imports === #

# Standard Library Imports
import os
import re
import subprocess
from pathlib import Path

# Third Party Imports
# Local Imports
from ..process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def run() -> int:
    from ..release.review import pages, request

    if os.environ["GITHUB_REF"] != "refs/heads/dev":
        raise ValueError("nightly requires dev")
    build = True
    if os.environ["GITHUB_EVENT_NAME"] == "schedule":
        # A Missing Tag Is The First Nightly; Other API Failures Must Fail Closed.
        # Run the Native Command
        command = ["gh", "api", "--include", f"repos/{os.environ['GITHUB_REPOSITORY']}/git/ref/tags/nightly"]
        result = subprocess.run(
            command,
            check=False,
            capture_output=True,
            text=True,
            creationflags=spawn_flags(),
        )
        if result.returncode and not re.search(r"HTTP/\S+ 404", result.stdout) and "HTTP 404" not in result.stderr:
            raise RuntimeError("nightly tag lookup failed")
        if not result.returncode:
            published = request("git/ref/tags/nightly")
            build = published["object"]["sha"] != os.environ["GITHUB_SHA"]
    for status in ("in_progress", "queued"):
        runs = pages(f"actions/workflows/nightly.yml/runs?head_sha={os.environ['GITHUB_SHA']}&status={status}", "workflow_runs")
        if any(int(run["id"]) < int(os.environ["GITHUB_RUN_ID"]) for run in runs):
            build = False
            break
    output = Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8")
    try:
        output.write(f"build={str(build).lower()}\n")
    finally:
        output.close()
    return 0


# === Private Functions === #

# === Entry Point === #
