# Rendered by stkr project update from templates/python/ci/compiler.py; sha256:09d8afdd534c72249b44b9e92e868e0d021e20d8d20ff3d28a09f4638edc9466
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Run Compiler Commands With An Owned Foreground Sccache Server."""

# === Imports === #

# Standard Library Imports
import json
import os
import queue
import re
import subprocess
import sys
import tempfile
import threading
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def run(
    command: Sequence[str] | str,
    shell: str | None = None,
    *,
    statistics: dict | None = None,
) -> int:
    """Require The Declared Remote Backend And Clean Up Only Our Process."""
    backend = os.environ.get("STKR_SCCACHE_BACKEND")
    environment = {key: value for key, value in os.environ.items() if not key.startswith("SCCACHE_")}
    mode = os.environ.get(
        "STKR_SCCACHE_MODE",
        "READ_WRITE" if os.environ.get("STKR_CACHE_POLICY") == "pull-push" else "READ_ONLY",
    )
    if mode not in {"READ_ONLY", "READ_WRITE"}:
        raise RuntimeError("sccache: invalid backend access mode")
    if backend == "gha":
        if not os.environ.get("ACTIONS_RUNTIME_TOKEN") or not any(
            os.environ.get(name) for name in ("ACTIONS_RESULTS_URL", "ACTIONS_CACHE_URL")
        ):
            raise RuntimeError("sccache: Actions runtime storage credentials are unavailable")
        environment.update(
            SCCACHE_GHA_ENABLED="true",
            SCCACHE_GHA_RW_MODE=mode,
            SCCACHE_GHA_VERSION="stkr-v5",
        )
    elif backend == "s3":
        if not os.environ.get("STKR_SCCACHE_BUCKET") or not os.environ.get("STKR_SCCACHE_REGION"):
            raise RuntimeError("sccache: S3 bucket and region are required")
        environment.update(
            SCCACHE_BUCKET=os.environ["STKR_SCCACHE_BUCKET"],
            SCCACHE_REGION=os.environ["STKR_SCCACHE_REGION"],
            SCCACHE_S3_RW_MODE=mode,
            SCCACHE_S3_KEY_PREFIX="stkr-v5/" + os.environ.get("CI_PROJECT_PATH_SLUG", "project") + "/",
        )
        if endpoint := os.environ.get("STKR_SCCACHE_ENDPOINT"):
            environment["SCCACHE_ENDPOINT"] = endpoint
    else:
        raise RuntimeError("sccache: a supported remote backend is required")
    with tempfile.TemporaryDirectory(prefix="stkr-sccache-") as temporary:
        # Ignore Persistent Runner Configuration And Never Fall Back To Its Local Store.
        configuration = Path(temporary) / "config"
        configuration.write_text("", encoding="utf-8")
        environment.update(
            SCCACHE_CONF=str(configuration),
            SCCACHE_CACHED_CONF=str(Path(temporary) / "cached-config"),
        )
        return _execute(command, shell, environment, backend, statistics=statistics)


# === Private Functions === #


def _execute(
    command: Sequence[str] | str,
    shell: str | None,
    environment: dict[str, str],
    backend: str | None = None,
    *,
    statistics: dict | None = None,
) -> int:
    endpoint: queue.Queue[str] = queue.Queue(maxsize=1)
    server_environment = environment | {
        "SCCACHE_START_SERVER": "1",
        "SCCACHE_NO_DAEMON": "1",
        "SCCACHE_SERVER_PORT": "0",
        "SCCACHE_LOG": "info",
        "SCCACHE_IDLE_TIMEOUT": "0",
    }
    # Pinned 0.18.0 Logs This Only After Backend Readiness And Binding Port 0.
    with subprocess.Popen(
        ["sccache"],
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        env=server_environment,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    ) as server:

        def read_endpoint() -> None:
            assert server.stderr is not None
            for line in server.stderr:
                match = re.search(r"server started, listening on 127\.0\.0\.1:([1-9][0-9]*)\s*$", line)
                if match and endpoint.empty():
                    endpoint.put(match[1])
            if endpoint.empty():
                endpoint.put("")

        reader = threading.Thread(target=read_endpoint, daemon=True)
        reader.start()
        try:
            try:
                port = endpoint.get(timeout=60)
            except queue.Empty:
                raise RuntimeError("sccache: remote backend readiness timed out; check native CI credentials and storage access") from None
            if not port or server.poll() is not None:
                raise RuntimeError("sccache: remote backend startup failed; check native CI credentials and storage access")
            child_environment = environment | {
                "SCCACHE_SERVER_PORT": port,
                "RUSTC_WRAPPER": "sccache",
                "CARGO_INCREMENTAL": "0",
            }
            if backend is not None:
                try:
                    readiness = subprocess.run(
                        ["sccache", "--show-stats", "--stats-format", "json"],
                        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
                        env=child_environment,
                        capture_output=True,
                        text=True,
                        check=True,
                        timeout=10,
                    )
                    information = json.loads(readiness.stdout)
                    scheme = "ghac" if backend == "gha" else "s3"
                    if not information.get("cache_location", "").startswith(scheme + ",") or information.get("version") != "0.18.0":
                        raise ValueError
                except (OSError, ValueError, subprocess.SubprocessError):
                    raise RuntimeError("sccache: pinned remote backend verification failed; refusing local storage fallback") from None
            if isinstance(command, str):
                shell = shell or ("pwsh" if os.name == "nt" else "bash")
                command = (
                    [
                        "pwsh",
                        "-NoProfile",
                        "-NonInteractive",
                        "-Command",
                        "$ErrorActionPreference = 'Stop'\n" + command + "\nexit $LASTEXITCODE",
                    ]
                    if shell == "pwsh"
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
            with subprocess.Popen(
                command,
                stdin=sys.stdin,
                stdout=sys.stdout,
                stderr=sys.stderr,
                creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
                env=child_environment,
            ) as child:
                try:
                    while child.poll() is None:
                        if server.poll() is not None:
                            child.terminate()
                            raise RuntimeError("sccache: owned compiler cache server exited during the command")
                        try:
                            return child.wait(timeout=0.2)
                        except subprocess.TimeoutExpired:
                            continue
                    return child.returncode
                finally:
                    if server.poll() is None:
                        try:
                            result = subprocess.run(
                                ["sccache", "--show-stats", "--stats-format", "json"],
                                creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
                                env=child_environment,
                                stdin=subprocess.DEVNULL,
                                stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL,
                                text=True,
                                check=True,
                                timeout=10,
                            )
                            if statistics is not None:
                                statistics.update(_statistics(json.loads(result.stdout)))
                        except (
                            OSError,
                            ValueError,
                            TypeError,
                            subprocess.SubprocessError,
                        ):
                            pass
        finally:
            # The Process Handle Is Ours; Never Issue Stop-Server Against A Reused Endpoint.
            if server.poll() is None:
                server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait(timeout=10)
            reader.join(timeout=1)


def _statistics(information: dict) -> dict[str, int | None]:
    if not isinstance(information, dict):
        return {}
    stats = information.get("stats", {})
    if not isinstance(stats, dict):
        return {}
    result = {}
    for key in (
        "cache_hits",
        "cache_misses",
        "cache_errors",
        "requests_not_cacheable",
        "cache_read_errors",
        "cache_write_errors",
    ):
        value = stats.get(key)
        if key in {"cache_hits", "cache_misses", "cache_errors"}:
            counts = value.get("counts") if isinstance(value, dict) else None
            value = (
                sum(counts.values())
                if isinstance(counts, dict) and all(type(item) is int and item >= 0 for item in counts.values())
                else None
            )
        result[key] = value if type(value) is int and value >= 0 else None
    return result


# === Entry Point === #
