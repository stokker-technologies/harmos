# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Run Native Tasks With The Shared Compiler Cache"""

# === Imports === #

# Standard Library Imports
import os
import subprocess
import sys

# Third Party Imports
# Local Imports
from ..process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #

# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    arguments = sys.argv[1:]
    if arguments[:1] == ["--"]:
        arguments = arguments[1:]
    if not arguments:
        raise SystemExit("pass an existing mise task invocation")
    # Reuse The Tooling Interpreter In Nested Python Commands.
    if arguments[0] == "python":
        arguments[0] = sys.executable
    environment = dict(os.environ)
    environment.pop("UV_PROJECT_ENVIRONMENT", None)
    if not environment.get("CI") and environment.get("RUSTC_WRAPPER") == "sccache" and not environment.get("STKR_LOCAL_COMPILER_CACHE"):
        from ..ci import compiler

        os.environ.update(environment, COMPILER_CACHE_BACKEND="disk", COMPILER_CACHE_MODE="READ_WRITE", STKR_LOCAL_COMPILER_CACHE="1")
        statistics = {}
        try:
            status = compiler.run(arguments, statistics=statistics)
        except compiler.CacheError:
            raise SystemExit(1) from None
        print(f"Local Compiler Cache: {statistics.get('cache_hits')} Hits, {statistics.get('cache_misses')} Misses", flush=True)
        raise SystemExit(status)
    result = subprocess.run(
        arguments,
        env=environment,
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
        check=False,
        creationflags=spawn_flags(),
    )
    raise SystemExit(result.returncode)
