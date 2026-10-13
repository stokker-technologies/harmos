# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Exercise Real Cargo Watch Builds And Process-Tree Cancellation."""

# === Imports === #

# Standard Library Imports
import hashlib
import json
import os
import queue
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

# Third Party Imports
import pytest

# Local Imports
from harmos_tools import artifacts

# Type Checking Imports

# === Public Exports === #

# === Constants === #

REPOSITORY = Path(__file__).resolve().parents[4]
FIXTURE = REPOSITORY / "crates/harmos-macros/tests/fixtures/metadata"

# === Types === #

# === Classes === #


class Watching:
    def __init__(self, command: list[str], root: Path) -> None:
        self.events: queue.Queue[dict] = queue.Queue()
        self.log_path = root / "build.log"
        self.log = self.log_path.open("w", encoding="utf-8")
        self.process = subprocess.Popen(
            command,
            cwd=root,
            stdout=subprocess.PIPE,
            stderr=self.log,
            text=True,
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0,
        )
        self.reader = threading.Thread(target=self._read)
        self.reader.start()

    def _read(self) -> None:
        assert self.process.stdout is not None
        for line in self.process.stdout:
            self.events.put(json.loads(line))

    def event(self, expected: str, timeout: float = 120) -> dict:
        try:
            event = self.events.get(timeout=timeout)
        except queue.Empty:
            self.log.flush()
            pytest.fail(
                f"Missing {expected}; exit={self.process.poll()}: {self.log_path.read_text()}"
            )
        assert event["event"] == expected, event
        return event

    def quiet(self) -> None:
        with pytest.raises(queue.Empty):
            self.events.get(timeout=0.8)
        assert self.process.poll() is None

    def cancel(self) -> int:
        try:
            if self.process.poll() is None:
                try:
                    self.process.send_signal(
                        signal.CTRL_BREAK_EVENT if os.name == "nt" else signal.SIGINT
                    )
                except ProcessLookupError:
                    pass
            return self.process.wait(timeout=10)
        finally:
            if self.process.poll() is None:
                self.process.kill()
                self.process.wait(timeout=10)
            self.reader.join(timeout=10)
            assert not self.reader.is_alive()
            assert self.process.stdout is not None
            self.process.stdout.close()
            self.log.close()


# === Public Functions === #


@pytest.mark.parametrize("backend", ["sidecar", "guest"])
def test_watch_build_cancel(backend: str) -> None:
    prepared = artifacts.artifacts()
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        source = root / "plugin"
        (source / "src").mkdir(parents=True)
        dependency = root / "dependency"
        (dependency / "src").mkdir(parents=True)
        (dependency / "Cargo.toml").write_text(
            '[package]\nname="watch-dependency"\nversion="0.0.0"\nedition="2024"\n',
            encoding="utf-8",
        )
        dependent = dependency / "src/lib.rs"
        dependent.write_text("pub fn value() -> usize { 10 }\n", encoding="utf-8")
        for name in ("main.rs", "lib.rs", "streams.rs", "operations.rs"):
            shutil.copyfile(FIXTURE / name, source / "src" / name)
        selected = source / "src" / ("main.rs" if backend == "sidecar" else "lib.rs")
        content = selected.read_text(encoding="utf-8")
        if backend == "sidecar":
            content = content.replace(
                "fn main() {",
                'fn main() {\n    if std::env::args().nth(1).as_deref() == Some("--watch-value") { println!("{}", WATCH_SOURCE + watch_dependency::value()); return; }',
            )
        else:
            content = content.replace(
                "markers.iter().map(|marker| marker.len()).sum()",
                "markers.iter().map(|marker| marker.len()).sum::<usize>() + WATCH_SOURCE + watch_dependency::value()",
            )
        content += "\nconst WATCH_SOURCE: usize = 1;\n"
        selected.write_text(content, encoding="utf-8")
        sdk = "harmos-sidecar" if backend == "sidecar" else "harmos-guest"
        manifest = (FIXTURE / "manifest.toml").read_text(encoding="utf-8")
        manifest = (
            manifest.replace('"@VERSION@"', '"0.0.0"')
            .replace('"@SDK_PACKAGE@"', json.dumps(sdk))
            .replace('"@SDK_PATH@"', json.dumps(str(REPOSITORY / "crates" / sdk)))
        )
        manifest = manifest.replace(
            "[workspace]",
            '[dependencies.watch-dependency]\npath="../dependency"\n[workspace]',
        )
        (source / "Cargo.toml").write_text(manifest, encoding="utf-8")
        shutil.copyfile(REPOSITORY / "Cargo.lock", source / "Cargo.lock")
        subprocess.run(
            [
                "cargo",
                "generate-lockfile",
                "--offline",
                "--manifest-path",
                str(source / "Cargo.toml"),
            ],
            check=True,
            capture_output=True,
            text=True,
            timeout=60,
        )
        target = REPOSITORY / "target/fixtures/harmos/build"
        command = [
            str(prepared["cargo-harmos-both"]),
            backend,
            "dev",
            "--manifest-path",
            str(source / "Cargo.toml"),
            "--target-dir",
            str(target),
            "--json",
            "--debounce-ms",
            "100",
        ]
        if backend == "guest":
            command.extend(["--no-default-features", "--features", "guest"])
        watcher = Watching(command, root)
        try:
            assert watcher.event("started")["attempt"] == 1
            built = watcher.event("built")
            package = Path(built["build"]["packages"][0]["path"])
            artifact = Path(built["build"]["artifacts"][0]["path"])
            initial = _hash(package)
            watcher.quiet()

            # Source And Local Dependency Changes Reach Real Compiled Outputs
            for _ in range(10):
                selected.write_text(
                    content.replace(
                        "WATCH_SOURCE: usize = 1", "WATCH_SOURCE: usize = 2"
                    ),
                    encoding="utf-8",
                )
            assert watcher.event("started")["attempt"] == 2
            assert watcher.event("built")["attempt"] == 2
            watcher.quiet()
            after_source = _hash(package)
            assert after_source != initial
            dependent.write_text("pub fn value() -> usize { 20 }\n", encoding="utf-8")
            watcher.event("started")
            watcher.event("built")
            after_dependency = _hash(package)
            assert after_dependency != after_source
            if backend == "sidecar":
                answered = subprocess.run(
                    [artifact, "--watch-value"],
                    check=True,
                    capture_output=True,
                    text=True,
                    timeout=10,
                )
                assert answered.stdout.strip() == "22"
            watcher.quiet()

            # Failed Preparation Keeps The Last Usable Package Until Recovery
            dependent.write_text('compile_error!("watch failure");\n', encoding="utf-8")
            watcher.event("started")
            watcher.event("failed")
            assert _hash(package) == after_dependency
            dependent.write_text("pub fn value() -> usize { 30 }\n", encoding="utf-8")
            watcher.event("started")
            watcher.event("built")
            assert _hash(package) != after_dependency
            watcher.quiet()

            # Generated Output Activity Does Not Trigger A Rebuild
            marker = package.parent / "watch-output.txt"
            marker.write_text("ignored output", encoding="utf-8")
            watcher.quiet()
            marker.unlink()
            watcher.quiet()

            # Cancel While Cargo Owns A Blocking Build Script And Its Child
            pids = root / "pids.json"
            program = (
                "import json,os,pathlib,time; pathlib.Path("
                + repr(str(pids))
                + ").write_text(json.dumps([os.getppid(),os.getpid()])); time.sleep(600)"
            )
            script = (
                "fn main() { std::process::Command::new("
                + json.dumps(sys.executable, ensure_ascii=False)
                + ').args(["-c", '
                + json.dumps(program, ensure_ascii=False)
                + "]).status().unwrap(); }\n"
            )
            (source / "build.rs").write_text(script, encoding="utf-8")
            watcher.event("started")
            deadline = time.monotonic() + 60
            while not pids.exists() and time.monotonic() < deadline:
                assert watcher.process.poll() is None
                time.sleep(0.02)
            assert pids.exists(), watcher.log_path.read_text()
            children = json.loads(pids.read_text())
            assert all(_running(pid) for pid in children)
            assert watcher.cancel() == 0
            deadline = time.monotonic() + 5
            while (
                any(_running(pid) for pid in children) and time.monotonic() < deadline
            ):
                time.sleep(0.02)
            assert not any(_running(pid) for pid in children), children
        finally:
            watcher.cancel()


# === Private Functions === #


def _hash(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _running(pid: int) -> bool:
    if os.name == "nt":
        import ctypes

        from ctypes import wintypes

        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        kernel.OpenProcess.restype = wintypes.HANDLE
        kernel.GetExitCodeProcess.argtypes = [
            wintypes.HANDLE,
            ctypes.POINTER(wintypes.DWORD),
        ]
        kernel.GetExitCodeProcess.restype = wintypes.BOOL
        kernel.CloseHandle.argtypes = [wintypes.HANDLE]
        kernel.CloseHandle.restype = wintypes.BOOL
        handle = kernel.OpenProcess(0x1000, False, pid)
        if not handle:
            error = ctypes.get_last_error()
            if error != 87:
                raise ctypes.WinError(error)
            return False
        try:
            code = wintypes.DWORD()
            assert kernel.GetExitCodeProcess(handle, ctypes.byref(code))
            return code.value == 259
        finally:
            kernel.CloseHandle(handle)
    state = Path(f"/proc/{pid}/stat")
    if state.exists():
        try:
            return state.read_text().split(") ", 1)[1].split()[0] != "Z"
        except FileNotFoundError:
            return False
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    return True


# === Entry Point === #
