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
    subprocess.run([native_examples["schematic-cli"]], cwd=ROOT, check=True)


def test_guest_schematic(built_examples: dict[str, Path]) -> None:
    subprocess.run(
        [built_examples["schematic-cli"], "--guest"],
        cwd=ROOT,
        check=True,
    )


def test_matching_backends(built_examples: dict[str, Path], tmp_path: Path) -> None:
    (tmp_path / "zeta").mkdir()
    (tmp_path / "alpha").mkdir()
    (tmp_path / "file.txt").write_text("not a directory")
    outputs = []
    for backend, artifact in [
        ("--sidecar", "schematic-sidecar"),
        ("--guest", "schematic-tools"),
    ]:
        result = subprocess.run(
            [
                built_examples["schematic-cli"],
                backend,
                built_examples[artifact],
                "--advanced",
            ],
            cwd=tmp_path,
            check=True,
            capture_output=True,
            text=True,
        )
        outputs.append(result.stdout.splitlines())
    native, guest = outputs
    directories = [line for line in native if line.startswith("Native directories:")]
    assert directories == ['Native directories: ["alpha", "zeta"]']
    assert [
        line for line in native if not line.startswith("Native directories:")
    ] == guest
    assert "Analysis settled: 3 findings" in guest
    assert "Cancelled analysis and waited for settlement" in guest
    assert "Reopened: 2 symbols, 1 net, 3 route points" in guest


def test_bundled_backends(built_examples: dict[str, Path], tmp_path: Path) -> None:
    (tmp_path / "only-caller-directory").mkdir()
    for arguments in ([], ["--guest"]):
        result = subprocess.run(
            [built_examples["schematic-cli"], *arguments, "--advanced"],
            cwd=tmp_path,
            check=True,
            capture_output=True,
            text=True,
        )
        assert "Reopened: 2 symbols, 1 net, 3 route points" in result.stdout
        if not arguments:
            assert 'Native directories: ["only-caller-directory"]' in result.stdout
        else:
            assert "Native directories:" not in result.stdout


def test_cargo_tool_installation(
    native_examples: dict[str, Path], tmp_path: Path
) -> None:
    # Cargo Discovers The Exact Extracted Release Executable Through PATH.
    environment = os.environ | {
        "PATH": str(native_examples["cargo-harmos"].parent)
        + os.pathsep
        + os.environ["PATH"]
    }
    help_result = subprocess.run(
        ["cargo", "harmos", "--help"],
        cwd=tmp_path,
        env=environment,
        check=True,
        capture_output=True,
        text=True,
    )
    assert "sidecar" in help_result.stdout
    assert "guest" in help_result.stdout
    assert "componentize" not in help_result.stdout
    assert "inspect" in help_result.stdout
    for artifact, facility in [
        ("schematic-tools", "guest"),
        ("schematic-sidecar", "sidecar"),
    ]:
        result = subprocess.run(
            ["cargo", "harmos", "inspect", str(native_examples[artifact]), "--json"],
            cwd=tmp_path,
            env=environment,
            check=True,
            capture_output=True,
            text=True,
        )
        record = json.loads(result.stdout)
        assert record["facility"] == facility
        assert record["id"] == "schematic-tools"
        assert len(record["sha256"]) == 64
        assert record["operations"]
    missing = subprocess.run(
        ["cargo", "harmos", "inspect", "missing-plugin"],
        cwd=tmp_path,
        env=environment,
        capture_output=True,
        text=True,
    )
    assert missing.returncode == 3
    malformed = tmp_path / "invalid.wasm"
    malformed.write_bytes(b"invalid input")
    invalid = subprocess.run(
        ["cargo", "harmos", "inspect", str(malformed)],
        cwd=tmp_path,
        env=environment,
        capture_output=True,
        text=True,
    )
    assert invalid.returncode == 2


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
    executable = native_examples["harmos-lifecycle-fixture"]
    with (tmp_path / "stderr.log").open("wb") as diagnostics:
        process = subprocess.Popen(
            [
                executable,
                tmp_path / "document",
                "--json",
                "--exercise-shutdown",
                "--worker",
                native_examples["harmos-lifecycle-sidecar"],
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
            for dispatched in [
                timings["operation_stop_ns"],
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
