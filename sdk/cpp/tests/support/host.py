# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Locate Built SDK and Host Artifacts Without Compiling in a Test."""

# === Imports === #

# Standard Library Imports
import os
import subprocess
from pathlib import Path

# Third Party Imports
# Local Imports
from harmos_tools.build.paths import output
from harmos_tools.process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[4]

# === Types === #

# === Classes === #

# === Public Functions === #


def run_managed() -> subprocess.CompletedProcess[str]:
    profile = os.environ.get("TEST_PROFILE", "dev")
    configuration = "Debug" if profile == "dev" else "Release"
    suffix = ".exe" if os.name == "nt" else ""
    exported = output(ROOT, "build", profile) / "bin" / f"harmos-conformance{suffix}"
    local = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "debug" / f"harmos-conformance{suffix}"
    host = Path(os.environ.get("HARMOS_CONFORMANCE_HOST", exported if exported.is_file() else local))
    directory = ROOT / "target/cpp" / profile
    plugin = directory / configuration / f"harmos-cpp-conformance{suffix}" if os.name == "nt" else directory / "harmos-cpp-conformance"
    if not host.is_file():
        raise FileNotFoundError("Build the Host: mise run build -- --package harmos-conformance")
    if not plugin.is_file():
        raise FileNotFoundError("Build the SDK: mise run build -- --language cpp")
    environment = dict(os.environ)
    environment.pop("HARMOS_ADDRESS", None)
    # The Bound Is a Transport Hang Guard; the Scenario Uses Protocol Events and Literal Ticks.
    result = subprocess.run(
        [str(host), "--plugin", str(plugin)],
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        check=False,
        timeout=540,
        creationflags=spawn_flags(),
    )
    print(result.stdout, end="")
    return result


# === Private Functions === #

# === Entry Point === #
