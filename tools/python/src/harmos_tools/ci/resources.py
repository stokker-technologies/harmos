# Rendered by stkr project update from templates/python/ci/resources.py; sha256:81c31b3a4bfc99ae49b0d52a1f472f54a23d12c45839d7e4b04b3e71cb299832
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Own Native Process Trees And Redistribute The Shared Execution Budget."""

# === Imports === #

# Standard Library Imports
import ctypes
import hashlib
import os
import subprocess
import sys
import tempfile
import threading
from pathlib import Path

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

GIB = 1024**3

# === Types === #

# === Classes === #


class SimulationError(RuntimeError):
    pass


class NativeLimit:
    def __init__(self, cpu: int | None, memory: int | None):
        if sys.platform != "win32":
            raise SimulationError("native process resource limits require Windows")

        class Basic(ctypes.Structure):
            _fields_ = [
                ("process_time", ctypes.c_longlong),
                ("job_time", ctypes.c_longlong),
                ("flags", ctypes.c_ulong),
                ("minimum", ctypes.c_size_t),
                ("maximum", ctypes.c_size_t),
                ("active", ctypes.c_ulong),
                ("affinity", ctypes.c_size_t),
                ("priority", ctypes.c_ulong),
                ("scheduling", ctypes.c_ulong),
            ]

        class Counters(ctypes.Structure):
            _fields_ = [(name, ctypes.c_ulonglong) for name in ("read", "write", "other", "read_bytes", "write_bytes", "other_bytes")]

        class Limits(ctypes.Structure):
            _fields_ = [
                ("basic", Basic),
                ("io", Counters),
                ("process_memory", ctypes.c_size_t),
                ("job_memory", ctypes.c_size_t),
                ("peak_process", ctypes.c_size_t),
                ("peak_job", ctypes.c_size_t),
            ]

        class Rate(ctypes.Structure):
            _fields_ = [("flags", ctypes.c_ulong), ("rate", ctypes.c_ulong)]

        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.CreateJobObjectW.restype = ctypes.c_void_p
        kernel.SetInformationJobObject.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_ulong]
        kernel.AssignProcessToJobObject.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        kernel.CloseHandle.argtypes = [ctypes.c_void_p]
        self.kernel = kernel
        self.handle = kernel.CreateJobObjectW(None, None)
        if not self.handle:
            raise SimulationError("cannot create aggregate native resource limit")
        limits = Limits()
        limits.basic.flags = 0x2000 | (0x200 if memory is not None else 0)
        limits.job_memory = memory or 0
        self.limits = limits
        self.rate_type = Rate
        rate = Rate(5, max(1, min(10000, (cpu or 1) * 10000 // (os.cpu_count() or 1))))
        if not kernel.SetInformationJobObject(self.handle, 9, ctypes.byref(limits), ctypes.sizeof(limits)) or (
            cpu is not None and not kernel.SetInformationJobObject(self.handle, 15, ctypes.byref(rate), ctypes.sizeof(rate))
        ):
            self.close()
            raise SimulationError("cannot enforce the aggregate native CPU/RAM budget")

    def resize(self, cpu: float, memory: int) -> None:
        if sys.platform != "win32":
            raise SimulationError("native process resource limits require Windows")

        self.limits.job_memory = memory
        rate = self.rate_type(5, max(1, min(10000, int(cpu * 10000 / (os.cpu_count() or 1)))))
        if not self.kernel.SetInformationJobObject(
            self.handle, 9, ctypes.byref(self.limits), ctypes.sizeof(self.limits)
        ) or not self.kernel.SetInformationJobObject(self.handle, 15, ctypes.byref(rate), ctypes.sizeof(rate)):
            raise SimulationError("cannot redistribute the aggregate native budget")

    def attach(self, process, *, resume: bool = True) -> None:
        if sys.platform != "win32":
            raise SimulationError("native process resource limits require Windows")

        if not self.kernel.AssignProcessToJobObject(self.handle, int(process._handle)):
            process.kill()
            process.wait()
            raise SimulationError("cannot attach the owned command to its aggregate resource limit")
        if not resume:
            return

        class Thread(ctypes.Structure):
            _fields_ = [
                ("size", ctypes.c_ulong),
                ("usage", ctypes.c_ulong),
                ("identity", ctypes.c_ulong),
                ("owner", ctypes.c_ulong),
                ("priority", ctypes.c_long),
                ("delta", ctypes.c_long),
                ("flags", ctypes.c_ulong),
            ]

        self.kernel.CreateToolhelp32Snapshot.restype = ctypes.c_void_p
        self.kernel.Thread32First.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        self.kernel.Thread32Next.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        self.kernel.OpenThread.restype = ctypes.c_void_p
        self.kernel.ResumeThread.argtypes = [ctypes.c_void_p]
        self.kernel.ResumeThread.restype = ctypes.c_ulong
        snapshot = self.kernel.CreateToolhelp32Snapshot(4, 0)
        resumed = False
        try:
            entry = Thread()
            entry.size = ctypes.sizeof(entry)
            present = self.kernel.Thread32First(snapshot, ctypes.byref(entry))
            while present:
                if entry.owner == process.pid:
                    thread = self.kernel.OpenThread(2, False, entry.identity)
                    if thread:
                        try:
                            resumed = self.kernel.ResumeThread(thread) != 0xFFFFFFFF
                        finally:
                            self.kernel.CloseHandle(thread)
                        break
                present = self.kernel.Thread32Next(snapshot, ctypes.byref(entry))
        finally:
            self.kernel.CloseHandle(snapshot)
        if not resumed:
            process.kill()
            process.wait()
            raise SimulationError("cannot resume the owned resource-bounded command")

    def close(self) -> None:
        if sys.platform != "win32":
            return

        if self.handle:
            self.kernel.CloseHandle(self.handle)
            self.handle = None

    def peak(self) -> int | None:
        if sys.platform != "win32":
            return None

        self.kernel.QueryInformationJobObject.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_ulong, ctypes.c_void_p]
        if self.handle and self.kernel.QueryInformationJobObject(
            self.handle, 9, ctypes.byref(self.limits), ctypes.sizeof(self.limits), None
        ):
            return self.limits.peak_job
        return None


class Budget:
    def __init__(self, cpu: int, memory: int, limits: dict, report=None):
        self.cpu, self.memory, self.limits = cpu, memory, limits
        self.report = report
        self.native = None
        self.lock = threading.Lock()
        self.containers = {}
        self.active = []
        self.pending = []
        self.cancelled = threading.Event()
        self.last = None

    def update(self, active: list[dict], pending: list[dict]) -> None:
        with self.lock:
            if self.cancelled.is_set():
                return
            self.active, self.pending = active, pending
            self._resize()

    def register(self, key: str, container: str) -> None:
        with self.lock:
            if self.cancelled.is_set():
                raise SimulationError("simulation cancelled before container start")
            self.containers[key] = container
            self._resize()

    def unregister(self, key: str) -> None:
        with self.lock:
            self.containers.pop(key, None)

    def cancel(self) -> None:
        with self.lock:
            self.cancelled.set()
            if self.native:
                self.native.close()
            for name in self.containers.values():
                subprocess.run(["docker", "rm", "--force", name], capture_output=True, check=False)

    def _resize(self) -> None:
        remaining = self.active + self.pending
        native_remaining = any(job["executor"] == "native" for job in remaining)
        docker_remaining = any(job["executor"] == "docker" for job in remaining)
        native_active = any(job["executor"] == "native" for job in self.active)
        docker_active = [job for job in self.active if job["executor"] == "docker"]
        docker_cpu = (
            min(self.limits.get("docker", (0, 0))[0], self.cpu / 2 if native_active and docker_active else self.cpu) if docker_active else 0
        )
        docker_memory = (
            min(self.limits.get("docker", (0, 0))[1], self.memory // 2 if native_remaining else self.memory) if docker_remaining else 0
        )
        if self.native and native_remaining:
            self.native.resize(self.cpu - docker_cpu, self.memory - docker_memory)
        count = max(1, len(docker_active))
        slots = 2 if any(job["executor"] == "docker" for job in self.pending) else count
        allocation = (self.cpu - docker_cpu, self.memory - docker_memory, docker_cpu, docker_memory, len(docker_active))
        if allocation != self.last:
            if self.report:
                self.report(
                    f"native aggregate CPU={allocation[0]:g}, RAM={allocation[1] / GIB:.2f} GiB; Docker aggregate CPU={docker_cpu:g}, RAM={docker_memory / GIB:.2f} GiB; active Linux jobs={len(docker_active)}",
                )
            self.last = allocation
        for name in self.containers.values():
            subprocess.run(
                [
                    "docker",
                    "update",
                    "--cpus",
                    str(max(0.01, docker_cpu / count)),
                    "--memory",
                    str(max(1, docker_memory // slots)),
                    "--memory-swap",
                    str(max(1, docker_memory // slots)),
                    name,
                ],
                capture_output=True,
                check=True,
            )


# === Public Functions === #


def managed_image(mise_image: str, setup: str) -> str:
    recipe = f"""FROM {mise_image} AS mise
FROM ubuntu:24.04@sha256:68ee22004a9109b44973313b02ff7b1de250f1de0f5ecb628d73f579aaff33ac
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends python3 git curl ca-certificates sudo && rm -rf /var/lib/apt/lists/*
COPY --from=mise /usr/local/bin/mise /usr/local/bin/mise
COPY native-setup.sh /opt/stkr-ci/native-setup.sh
RUN bash /opt/stkr-ci/native-setup.sh && rm -rf /var/lib/apt/lists/*
ENTRYPOINT []
"""
    fingerprint = hashlib.sha256((recipe + setup).encode()).hexdigest()
    image = f"stkr-ci-ubuntu24:{fingerprint}"
    if subprocess.run(["docker", "image", "inspect", image], capture_output=True, check=False).returncode == 0:
        return image
    print(f"Preparing managed Ubuntu 24.04 executor image {image}", flush=True)
    with tempfile.TemporaryDirectory(prefix="stkr-ci-image-") as temporary:
        context = Path(temporary)
        (context / "Dockerfile").write_text(recipe, encoding="utf-8", newline="\n")
        (context / "native-setup.sh").write_text(setup, encoding="utf-8", newline="\n")
        subprocess.run(["docker", "build", "--tag", image, str(context)], check=True)
    return image


# === Private Functions === #

# === Entry Point === #
