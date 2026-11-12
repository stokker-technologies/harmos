# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build And Measure Focused Storage Workloads."""

# === Imports === #

# Standard Library Imports
import json
import os
import subprocess
from pathlib import Path

# Third Party Imports
# Local Imports
from .build import cargo
from .process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def run(directory: Path, selected: str | None) -> int:
    output = directory / "target/benchmarks/storage"
    output.mkdir(parents=True, exist_ok=True)
    messages = output / "compiler.json"
    status = cargo.focused(
        directory,
        "build",
        ["--locked", "--profile", "bench", "-p", "harmos-storage", "--features", "testing", "--bench", "integration-multi-day"],
        messages=messages,
    )
    if status:
        for line in messages.read_text(encoding="utf-8").splitlines():
            event = json.loads(line)
            if rendered := event.get("message", {}).get("rendered"):
                print(rendered, end="")
        return status
    executables = [
        event["executable"]
        for line in messages.read_text(encoding="utf-8").splitlines()
        if (event := json.loads(line)).get("executable") and event.get("target", {}).get("name") == "integration-multi-day"
    ]
    environment = dict(os.environ, CRITERION_HOME=str(output / "criterion"))
    environment.pop("STKR_TEST_REPORT_DIR", None)
    if selected == "baseline":
        environment["HARMOS_STORAGE_BASELINE_ONCE"] = "1"
    command = [executables[-1], "--bench", "--sample-size", "10", "--warm-up-time", "1", "--measurement-time", "1"]
    if selected:
        command.append(selected)
    with (output / "baseline.txt" if selected == "baseline" else output / "candidate.txt").open("w", encoding="utf-8") as log:
        with subprocess.Popen(
            command,
            cwd=directory,
            env=environment,
            creationflags=spawn_flags(),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        ) as child:
            assert child.stdout is not None
            for line in child.stdout:
                print(line, end="", flush=True)
                log.write(line)
                log.flush()
            return child.wait()


# === Private Functions === #

# === Entry Point === #
