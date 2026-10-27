# Rendered by stkr project update from templates/python/ci/build.py; sha256:5d2d1ef3175fb4cf1d51bf5b7fcdbefe451f749d5bb4e1212d5f1923274a70f0
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
import argparse
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
from . import compiler, summary

# Type Checking Imports

# === Public Exports === #

# === Constants === #

OUTPUTS = {"product": "Product Build", "tests": "Tests Build", "bench": "Bench Build"}

# === Types === #

# === Classes === #

# === Public Functions === #


def run(task: str, output: str, profile: str) -> int:
    if output not in OUTPUTS or profile not in ({"bench"} if output == "bench" else {"dev", "release"}):
        raise ValueError("build: output and profile do not match")
    if not task or task.startswith("-") or any(character.isspace() for character in task):
        raise ValueError("build: require one authored mise task name")
    command = ["mise", "run", task]
    if profile != "bench":
        command += ["--profile", profile]
    if output == "bench":
        return _run(command, OUTPUTS[output])
    # Check / Rust Runs lint:rust Over The Product And Tests Selections; These Producers Only Compile.
    previous = os.environ.get("STKR_CI_CHECK_CLIPPY")
    os.environ["STKR_CI_CHECK_CLIPPY"] = "1"
    try:
        return _run(command, OUTPUTS[output])
    finally:
        if previous is None:
            os.environ.pop("STKR_CI_CHECK_CLIPPY", None)
        else:
            os.environ["STKR_CI_CHECK_CLIPPY"] = previous


def main() -> int:
    parser = argparse.ArgumentParser(description="Produce A Repository-Owned Build Output")
    parser.add_argument("--profile", choices=("dev", "release", "bench"))
    parser.add_argument("--output", choices=OUTPUTS)
    parser.add_argument("--task")
    parser.add_argument("--internal-script", help=argparse.SUPPRESS)
    parser.add_argument("--shell", choices=("bash", "pwsh"), help=argparse.SUPPRESS)
    options = parser.parse_args()
    try:
        if options.task is not None:
            if options.profile is None or options.output is None or options.internal_script is not None:
                parser.error("--task requires --output and --profile")
            return run(options.task, options.output, options.profile)
        script = (
            bytes.fromhex(options.internal_script).decode("utf-8")
            if options.internal_script is not None
            else os.environ["_STKR_CI_BUILD_SCRIPT"]
        )
        return _run(script, "Task Build", options.shell)
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        parser.exit(
            1,
            f"build: {type(error).__name__}: {error}; the native command diagnostics precede this line\n",
        )


# === Private Functions === #


def _run(command: Sequence[str] | str, label: str, shell: str | None = None) -> int:
    started = time.monotonic()
    statistics = {}
    backend = os.environ.get("STKR_SCCACHE_BACKEND")
    mode = os.environ.get(
        "STKR_SCCACHE_MODE",
        "READ_WRITE" if os.environ.get("STKR_CACHE_POLICY") == "pull-push" else "READ_ONLY",
    )
    phase_store = tempfile.TemporaryDirectory(prefix="stkr-build-phases-")
    phase_file = Path(phase_store.name) / "phases.jsonl"
    previous_phase_file = os.environ.get("STKR_CI_PHASE_FILE")
    os.environ["STKR_CI_PHASE_FILE"] = str(phase_file)
    status = 1
    print(f"{label}: starting", flush=True)
    try:
        if backend is not None:
            status = compiler.run(command, shell, statistics=statistics)
        else:
            if isinstance(command, str):
                selected = shell or ("pwsh" if os.name == "nt" else "bash")
                command = (
                    [
                        "pwsh",
                        "-NoProfile",
                        "-NonInteractive",
                        "-Command",
                        "$ErrorActionPreference = 'Stop'\n" + command + "\nexit $LASTEXITCODE",
                    ]
                    if selected == "pwsh"
                    else [
                        "bash",
                        "--noprofile",
                        "--norc",
                        "-e",
                        "-o",
                        "pipefail",
                        "-c",
                        command,
                    ]
                )
            status = subprocess.run(
                command,
                stdin=sys.stdin,
                stdout=sys.stdout,
                stderr=sys.stderr,
                creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
                check=False,
            ).returncode
        return status
    finally:
        elapsed = time.monotonic() - started
        try:
            measured = summary.phases(phase_file)
        finally:
            if previous_phase_file is None:
                os.environ.pop("STKR_CI_PHASE_FILE", None)
            else:
                os.environ["STKR_CI_PHASE_FILE"] = previous_phase_file
            try:
                phase_store.cleanup()
            except OSError:
                print("Phase telemetry cleanup unavailable", flush=True)
        print(
            "STKR_BUILD_STATISTICS "
            + json.dumps(
                {
                    "backend": backend,
                    "mode": mode,
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

if __name__ == "__main__":
    raise SystemExit(main())
