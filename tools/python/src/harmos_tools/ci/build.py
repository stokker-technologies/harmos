# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Produce An Authored Build Output With Internal Compiler Cache Ownership"""

# === Imports === #

# Standard Library Imports
import json
import os
import subprocess
import sys
import tempfile
import time
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports
# Local Imports
from ..process import spawn_flags
from . import compiler, summary

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def run(command: Sequence[str], label: str) -> int:
    started = time.monotonic()
    statistics = {}
    backend = os.environ.get("COMPILER_CACHE_BACKEND")
    mode = os.environ.get(
        "COMPILER_CACHE_MODE",
        "READ_WRITE" if os.environ.get("CACHE_POLICY") == "pull-push" else "READ_ONLY",
    )
    phase_store = tempfile.TemporaryDirectory(prefix="project-build-phases-")
    phase_file = Path(phase_store.name) / "phases.jsonl"
    previous_phase_file = os.environ.get("BUILD_PHASE_FILE")
    os.environ["BUILD_PHASE_FILE"] = str(phase_file)
    status = 1
    print(f"{label}: starting", flush=True)
    try:
        if backend is not None:
            status = compiler.run(command, statistics=statistics)
        else:
            status = subprocess.run(
                command,
                stdin=sys.stdin,
                stdout=sys.stdout,
                stderr=sys.stderr,
                creationflags=spawn_flags(),
                check=False,
            ).returncode
        return status
    finally:
        elapsed = time.monotonic() - started
        try:
            measured = summary.phases(phase_file)
        finally:
            if previous_phase_file is None:
                os.environ.pop("BUILD_PHASE_FILE", None)
            else:
                os.environ["BUILD_PHASE_FILE"] = previous_phase_file
            try:
                phase_store.cleanup()
            except OSError:
                print("Phase telemetry cleanup unavailable", flush=True)
        print(
            "BUILD_STATISTICS "
            + json.dumps(
                {
                    "backend": backend if backend in {None, "disk", "s3"} else "unsupported",
                    "mode": mode if mode in {"READ_ONLY", "READ_WRITE"} else "unsupported",
                    "status": status,
                    "statistics": {key: value for key, value in statistics.items() if value is None or type(value) is int},
                    "phases": measured,
                },
                sort_keys=True,
            ),
            flush=True,
        )
        summary.render(label, elapsed, statistics, phases=measured)


# === Entry Point === #
