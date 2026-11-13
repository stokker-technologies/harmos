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
import math
import os
import platform
import statistics
import subprocess
from pathlib import Path

# Third Party Imports
# Local Imports
from .build import cargo
from .process import spawn_flags
from .release import policy
from .test import archive

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def run(directory: Path, selected: str | None) -> int:
    output = directory / "target/benchmarks/storage"
    output.mkdir(parents=True, exist_ok=True)
    target = (
        "integration-file-transfer"
        if selected == "file_transfer"
        else "integration-durable-lag"
        if selected == "durable_lag"
        else "integration-multi-day"
    )
    messages = output / (
        "file-transfer-compiler.json"
        if selected == "file_transfer"
        else "durable-lag-compiler.json"
        if selected == "durable_lag"
        else "compiler.json"
    )
    status = cargo.focused(
        directory,
        "build",
        [
            "--locked",
            "--profile",
            "bench",
            "-p",
            "harmos-runtime" if selected == "file_transfer" else "harmos-storage",
            "--features",
            "storage-benchmarks" if selected == "file_transfer" else "testing,sqlite",
            "--bench",
            target,
        ],
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
        if (event := json.loads(line)).get("executable") and event.get("target", {}).get("name") == target
    ]
    environment = dict(os.environ, CRITERION_HOME=str(output / "criterion"))
    environment.pop("STKR_TEST_REPORT_DIR", None)
    if selected == "baseline":
        environment["HARMOS_STORAGE_BASELINE_ONCE"] = "1"
    if selected == "native":
        environment["HARMOS_STORAGE_NATIVE"] = "1"
    command = [executables[-1], "--bench", "--sample-size", "10", "--warm-up-time", "1", "--measurement-time", "1"]
    if selected:
        command.append(selected)
    suite = "storage-file-transfer" if selected == "file_transfer" else "storage-durable-lag" if selected == "durable_lag" else "storage"
    destination = directory / "target/benchmarks" / f"benchmarks-{suite}-{platform.system().lower()}.json"
    if selected in {"native", "durable_lag", "file_transfer"}:
        policy.write_json(destination, {"benchmarks": []})
    lines = []
    peak = 0
    log_name = (
        "file-transfer.txt"
        if selected == "file_transfer"
        else "durable-lag.txt"
        if selected == "durable_lag"
        else "baseline.txt"
        if selected == "baseline"
        else "candidate.txt"
    )
    with (output / log_name).open("w", encoding="utf-8") as log:
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
                if not line.startswith("STORAGE_SAMPLES "):
                    print(line, end="", flush=True)
                log.write(line)
                log.flush()
                lines.append(line)
                peak = max(peak, _peak_memory(child))
            status = child.wait()
    if selected not in {"native", "durable_lag", "file_transfer"}:
        return status
    revision = os.environ.get("GITHUB_SHA") or archive.revision(directory) or "unknown"
    document = measurements(lines, revision, platform.system().lower())
    if peak:
        name = (
            "integration/file_transfer/process_peak_bytes"
            if selected == "file_transfer"
            else "integration/durable_lag/process_peak_bytes"
            if selected == "durable_lag"
            else "storage.process.peak.bytes"
        )
        document["benchmarks"].append(_row(name, peak, "bytes", revision, platform.system().lower()))
    policy.write_json(destination, document)
    print(f"Storage measurements: {destination}; peak resident memory: {peak} bytes")
    if not document["benchmarks"]:
        return 1
    for row in document["benchmarks"]:
        if row["name"] in {"storage.window.read.2m", "storage.window.next.2m", "storage.projection.read.2m"} and row["value"] > 2:
            print(f"Storage read budget exceeded: {row['name']}: {row['value']} s > 2 s")
            status = 1
    return status


def measurements(lines: list[str], revision: str, system: str, backend: str = "sqlite") -> dict:
    """Translate The Shared Workload's Native Events Without Backend-Dependent Case Names."""
    cases: dict[str, list[dict]] = {}
    counters = []
    work = []
    lag: dict[str, list[dict]] = {}
    transfers: dict[str, list[dict]] = {}
    for line in lines:
        if line.startswith("STORAGE_FILE_TRANSFER "):
            value = json.loads(line.removeprefix("STORAGE_FILE_TRANSFER "))
            transfers.setdefault(value["case"], []).append(value["measurement"])
        if line.startswith("STORAGE_DURABLE_LAG "):
            value = json.loads(line.removeprefix("STORAGE_DURABLE_LAG "))
            lag.setdefault(value["case"], []).append(value)
        if line.startswith("STORAGE_MEASUREMENT "):
            value = json.loads(line.removeprefix("STORAGE_MEASUREMENT "))
            cases.setdefault(value["name"], []).append(value)
        if line.startswith("STORAGE_COUNTER "):
            counters.append(json.loads(line.removeprefix("STORAGE_COUNTER ")))
        if line.startswith("STORAGE_WORK "):
            work.append(json.loads(line.removeprefix("STORAGE_WORK ")))
    rows = []
    for name, samples in sorted(cases.items()):
        for field, suffix, unit in (
            ("seconds", "", "s"),
            ("allocations", ".allocations", "count"),
            ("allocated_bytes", ".allocated.bytes", "bytes"),
            ("peak_heap_bytes", ".heap.peak.bytes", "bytes"),
        ):
            value = statistics.median(sample[field] for sample in samples)
            rows.append(_row(name + suffix, value, unit, revision, system, backend))
        seconds = statistics.median(sample["seconds"] for sample in samples)
        count = (
            2_000_000
            if name in {"storage.ingest.2m", "storage.scan.read.2m", "storage.projection.advance.2m", "storage.bucket.advance.2m"}
            else None
        )
        if name == "storage.object.contend.8":
            count = 8
        if count and seconds > 0:
            rows.append(_row(name + ".throughput", count / seconds, "rows/s" if count != 8 else "attempts/s", revision, system, backend))
    for counter in counters:
        rows.append(_row(counter["name"] + ".read.bytes", counter["bytes_read"], "bytes", revision, system, backend))
        rows.append(_row(counter["name"] + ".payload.rows", counter["payload_rows"], "rows", revision, system, backend))
        if "source_rows" in counter:
            rows.append(_row(counter["name"] + ".source.rows", counter["source_rows"], "rows", revision, system, backend))
    for counter in work:
        for field, unit in (
            ("source_rows", "rows"),
            ("source_bytes", "bytes"),
            ("decoded_chunks", "count"),
            ("bucket_rows", "rows"),
            ("bucket_steps", "count"),
            ("buckets", "count"),
            ("index_entries", "rows"),
            ("index_steps", "count"),
            ("index_prepares", "count"),
        ):
            rows.append(_row(counter["name"] + ".work." + field.replace("_", "."), counter[field], unit, revision, system, backend))
    for name, samples in sorted(lag.items()):
        for field, unit in (
            ("p50_ms", "ms"),
            ("p95_ms", "ms"),
            ("maximum_ms", "ms"),
            ("maximum_backlog_bytes", "bytes"),
            ("maximum_backlog_positions", "count"),
            ("admission_wait_ms", "ms"),
            ("batches", "count"),
            ("bytes", "bytes"),
        ):
            values = [case["measurement"][field] for case in samples]
            rows.append(_row(name + "/" + field, statistics.median(values), unit, revision, system, backend))
            if field == "p95_ms":
                rows.append(_row(name + "/p95_min_ms", min(values), unit, revision, system, backend))
                rows.append(_row(name + "/p95_max_ms", max(values), unit, revision, system, backend))
        rows.append(_row(name + "/repetitions", len(samples), "count", revision, system, backend))
        for field, unit in (
            ("synchronous_commits", "count"),
            ("synchronous_commit_ms", "ms"),
            ("maximum_synchronous_commit_ms", "ms"),
            ("writing_ms", "ms"),
            ("maximum_writing_ms", "ms"),
        ):
            values = [case[field] for case in samples if field in case]
            if values:
                rows.append(_row(name + "/" + field, statistics.median(values), unit, revision, system, backend))
        for stage in ("cadence_wait", "ordered_wait", "ordered_wait_small", "ordered_wait_large", "batch_build", "backend", "publication"):
            values = [case["measurement"].get("stages", {}).get(stage) for case in samples]
            values = [value for value in values if value is not None]
            for field, unit in (("count", "count"), ("total_ns", "ns"), ("maximum_ns", "ns")):
                if values:
                    rows.append(
                        _row(
                            name + "/stages/" + stage + "/" + field,
                            statistics.median(value[field] for value in values),
                            unit,
                            revision,
                            system,
                            backend,
                        )
                    )
        for field, unit in (
            ("committed_pieces", "count"),
            ("committed_rows", "rows"),
            ("committed_bytes", "bytes"),
            ("maximum_commit_bytes", "bytes"),
        ):
            values = [case["measurement"]["stages"][field] for case in samples if "stages" in case["measurement"]]
            if values:
                rows.append(_row(name + "/" + field, statistics.median(values), unit, revision, system, backend))
    for name, samples in sorted(transfers.items()):
        for field, unit in (
            ("bytes", "bytes"),
            ("chunk_bytes", "bytes"),
            ("calls", "count"),
            ("seconds", "s"),
            ("bytes_per_second", "bytes/s"),
        ):
            rows.append(
                _row(name + "/" + field, statistics.median(sample[field] for sample in samples), unit, revision, system, "reference-memory")
            )
    return {"benchmarks": rows}


# === Private Functions === #


def _row(name: str, value: float, unit: str, revision: str, system: str, backend: str = "sqlite") -> dict:
    if not math.isfinite(value) or value < 0:
        raise policy.ToolError(f"invalid storage measurement: {name}")
    return {
        "name": name,
        "suite": f"storage-{backend}-{system}",
        "backend": backend,
        "unit": unit,
        "value": value,
        "baseline_value": None,
        "revision": revision,
        "threshold_percent": 20,
    }


def _peak_memory(child: subprocess.Popen) -> int:
    if os.name != "nt":
        path = Path(f"/proc/{child.pid}/status")
        if not path.exists():
            return 0
        for line in path.read_text().splitlines():
            if line.startswith("VmHWM:"):
                return int(line.split()[1]) * 1024
        return 0
    import ctypes
    from ctypes import wintypes

    class Counters(ctypes.Structure):
        _fields_ = [("cb", wintypes.DWORD), ("faults", wintypes.DWORD)] + [
            (name, ctypes.c_size_t)
            for name in ("peak_working", "working", "peak_pool", "pool", "peak_nonpaged", "nonpaged", "pagefile", "peak_pagefile")
        ]

    counters = Counters()
    counters.cb = ctypes.sizeof(counters)
    process = ctypes.windll.kernel32.OpenProcess(0x0410, False, child.pid)
    if not process:
        return 0
    try:
        success = ctypes.windll.psapi.GetProcessMemoryInfo(wintypes.HANDLE(process), ctypes.byref(counters), counters.cb)
        return counters.peak_working if success else 0
    finally:
        ctypes.windll.kernel32.CloseHandle(wintypes.HANDLE(process))


# === Entry Point === #
