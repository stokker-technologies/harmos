# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Exercise native and guest example applications through built interfaces."""

# === Imports === #

# Standard Library Imports
import json
import os
import re
import selectors
import signal
import subprocess
import sys
import time
from collections.abc import Callable
from pathlib import Path

# Third Party Imports
import pytest

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[4]

# === Types === #

# === Classes === #

# === Public Functions === #


def test_native_schematic(native_examples: dict[str, Path]) -> None:
    subprocess.run([native_examples["minischematic-cli"]], cwd=ROOT, check=True)


def test_guest_schematic(built_examples: dict[str, Path]) -> None:
    subprocess.run(
        [built_examples["minischematic-cli"], "--guest", built_examples["autoroute"]],
        cwd=ROOT,
        check=True,
    )


def test_sidecar_bench(native_examples: dict[str, Path]) -> None:
    subprocess.run(
        [native_examples["minibench-cli"], native_examples["minibench-probe-sidecar"]],
        cwd=ROOT,
        check=True,
    )


@pytest.mark.skipif(os.name != "posix", reason="Unix process signal contract")
@pytest.mark.parametrize("shutdown_signal", [signal.SIGINT, signal.SIGTERM])
def test_schematic_signal(
    native_examples: dict[str, Path],
    tmp_path: Path,
    shutdown_signal: signal.Signals,
    record_testsuite_property: Callable[[str, object], None],
) -> None:
    if sys.platform == "win32":
        pytest.skip("Unix process signal contract")
    # Build Preparation Is Outside Both Measured Intervals
    executable = native_examples["minischematic-cli"]
    with (tmp_path / "stderr.log").open("wb") as diagnostics:
        process = subprocess.Popen(
            [
                executable,
                "serve",
                tmp_path / "document",
                "--json",
                "--exercise-shutdown",
                "--worker",
                native_examples["minischematic-worker"],
            ],
            cwd=ROOT,
            stdout=subprocess.PIPE,
            stderr=diagnostics,
            start_new_session=True,
        )
        try:
            assert process.stdout is not None
            ready = _ready(process.stdout.fileno())
            assert ready == {"event": "ready"}

            started = time.perf_counter()
            process.send_signal(shutdown_signal)
            output, _ = process.communicate(timeout=5)
            elapsed = time.perf_counter() - started

            assert process.returncode == 0, (tmp_path / "stderr.log").read_text()
            events = [json.loads(line) for line in output.splitlines()]
            assert len(events) == 3
            assert events[0] == {"event": "stopped"}
            assert events[2] == {"event": "recovered"}
            timings = events[1]
            assert timings["event"] == "shutdown-timings"
            record_testsuite_property(
                f"shutdown_{shutdown_signal.name}", json.dumps(timings, sort_keys=True)
            )
            assert timings["worker_exchanges"] > 0
            with pytest.raises(ProcessLookupError):
                os.killpg(process.pid, 0)
            cleanup = re.findall(
                r"worker-cleaned constructed=(\d+) completed=(\d+) finalized=(\d+)",
                (tmp_path / "stderr.log").read_text(),
            )
            assert len(cleanup) == 1, "the original worker must clean up exactly once"
            constructed, completed, finalized = map(int, cleanup[0])
            assert constructed == finalized
            assert completed >= timings["worker_exchanges"] > 0
            assert 0 < timings["pending_entries"] <= 64
            assert (
                timings["accepted_position"] - timings["stored_position"]
                == timings["pending_entries"]
            )
            settlement = timings["settlement_ns"]
            assert 0 <= settlement < 1_000_000_000
            assert len(timings["operation_stop_ns"]) == 2
            for dispatched in [
                *timings["operation_stop_ns"],
                timings["observer_stop_ns"],
            ]:
                assert 0 <= dispatched <= settlement
            assert elapsed < 1.0, f"settlement and recovery took {elapsed:.3f}s"
        finally:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait(timeout=5)
            if process.stdout is not None:
                process.stdout.close()


@pytest.mark.skipif(os.name != "posix", reason="Unix process group and signal contract")
@pytest.mark.parametrize("shutdown_signal", [signal.SIGINT, signal.SIGTERM])
def test_sidecar_signal(
    native_examples: dict[str, Path], tmp_path: Path, shutdown_signal: signal.Signals
) -> None:
    if sys.platform == "win32":
        pytest.skip("Unix process group and signal contract")
    with (tmp_path / "stderr.log").open("wb") as diagnostics:
        process = subprocess.Popen(
            [
                native_examples["minibench-cli"],
                "serve",
                native_examples["minibench-probe-sidecar"],
                "--json",
            ],
            cwd=ROOT,
            stdout=subprocess.PIPE,
            stderr=diagnostics,
            start_new_session=True,
        )
        try:
            assert process.stdout is not None
            assert _ready(process.stdout.fileno()) == {"event": "ready"}
            started = time.perf_counter()
            # Only Signal The Host; It Must Stop And Reap The Native Child.
            process.send_signal(shutdown_signal)
            output, _ = process.communicate(timeout=5)
            elapsed = time.perf_counter() - started
            assert process.returncode == 0, (tmp_path / "stderr.log").read_text()
            assert [json.loads(line) for line in output.splitlines()] == [
                {"event": "stopped"}
            ]
            with pytest.raises(ProcessLookupError):
                os.killpg(process.pid, 0)
            assert elapsed < 1.0, f"sidecar settlement took {elapsed:.3f}s"
        finally:
            # Clean Up A Failed Host And Any Child Left In Its Isolated Group.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait(timeout=5)
            if process.stdout is not None:
                process.stdout.close()


# === Private Functions === #


def _ready(descriptor: int) -> object:
    deadline = time.monotonic() + 5
    line = bytearray()
    with selectors.DefaultSelector() as selector:
        selector.register(descriptor, selectors.EVENT_READ)
        while b"\n" not in line:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not selector.select(remaining):
                pytest.fail("schematic did not report readiness within 5s")
            chunk = os.read(descriptor, 4096)
            if not chunk:
                pytest.fail("schematic exited before readiness")
            line.extend(chunk)
            assert len(line) <= 4096, "unexpectedly large readiness event"
    return json.loads(line)


# === Entry Point === #
