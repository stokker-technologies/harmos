# Rendered by stkr project update from templates/python/ci/build.py; sha256:cc59b569ea12f65bcff47ce47c69943c3c8a5170803230f88802405227bdd735
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
import os
import subprocess
import sys
import time
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports
# Local Imports
from . import compiler

# Type Checking Imports

# === Public Exports === #

# === Constants === #

OUTPUTS = {"product": "Product Build", "tests": "Tests Build", "bench": "Bench Build"}
METRICS = {
    "cache_hits": "Hits",
    "cache_misses": "Misses",
    "requests_not_cacheable": "Noncacheable Requests",
    "cache_errors": "Cache Errors",
    "cache_read_errors": "Read Errors",
    "cache_write_errors": "Write Errors",
}

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
    return _run(command, OUTPUTS[output])


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
            f"build: {type(error).__name__}; inspect the command diagnostics and native cache configuration\n",
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
        selected_backend = backend if backend in {"gha", "s3"} else "disabled" if backend is None else "unavailable"
        selected_mode = mode if backend is not None and mode in {"READ_ONLY", "READ_WRITE"} else "unavailable"
        rows = [
            ("Backend", selected_backend),
            ("Access Mode", selected_mode),
            ("Elapsed", f"{elapsed:.2f}s"),
            ("Exit", str(status)),
        ]
        rows.extend(
            (
                title,
                str(statistics[key]) if statistics.get(key) is not None else "unavailable",
            )
            for key, title in METRICS.items()
        )
        print(
            f"{label}: " + "; ".join(f"{name}={value}" for name, value in rows),
            flush=True,
        )
        if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
            try:
                with Path(summary).open("a", encoding="utf-8") as stream:
                    stream.write(f"## {label}\n\n| Metric | Value |\n| --- | --- |\n")
                    stream.writelines(f"| {name} | {value} |\n" for name, value in rows)
                    stream.write("\n")
            except OSError:
                print(f"{label}: GitHub summary unavailable", flush=True)


# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
