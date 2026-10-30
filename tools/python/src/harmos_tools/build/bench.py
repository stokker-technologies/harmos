# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build Benchmark Executables And Matching Native Hosts"""

# === Imports === #

# Standard Library Imports
import argparse
import json
import shutil
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports
# Local Imports
from .. import temporary
from ..process import spawn_flags
from ..test import archive
from . import cargo, paths

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def build(directory: Path, extra: Sequence[str] = ()) -> Path:
    output = paths.output(directory, "build", "bench")
    messages = output / "bench/compiler.json"
    messages.parent.mkdir(parents=True, exist_ok=True)
    messages.unlink(missing_ok=True)
    cargo.clippy(
        directory,
        [
            "--locked",
            "--workspace",
            "--bins",
            "--benches",
            "--profile",
            "bench",
            *extra,
        ],
    )
    command = [
        "cargo",
        "build",
        "--locked",
        "--workspace",
        "--bins",
        "--benches",
        "--profile",
        "bench",
        "--message-format=json-render-diagnostics",
        *extra,
    ]
    with temporary.directory() as staging:
        raw = staging / "compiler.json"
        with raw.open("w", encoding="utf-8") as written:
            subprocess.run(
                command,
                stdin=sys.stdin,
                stderr=sys.stderr,
                creationflags=spawn_flags(),
                cwd=directory,
                stdout=written,
                check=True,
            )
        for kind in ("bin", "bench"):
            (staging / kind).mkdir()

        events = []
        files = []
        benchmarks = False
        for line in raw.read_text(encoding="utf-8").splitlines():
            event = json.loads(line)
            executable = event.get("executable")
            kinds = event.get("target", {}).get("kind", [])
            kind = "bench" if "bench" in kinds else "bin" if kinds == ["bin"] and not event.get("profile", {}).get("test") else None
            if executable and kind:
                benchmarks = benchmarks or kind == "bench"
                source = Path(executable)
                destination = output / kind / source.name
                shutil.copy2(source, staging / kind / source.name)
                event["executable"] = destination.relative_to(directory).as_posix()
                event["manifest_path"] = Path(event["manifest_path"]).relative_to(directory).as_posix()
                files.append(event["executable"])
            events.append(json.dumps(event))

        if not benchmarks:
            raise ValueError("no benchmark target was built")
        for kind in ("bin", "bench"):
            paths.replace(directory, staging / kind, output / kind, origin=staging)
        messages.write_text("\n".join(events) + "\n", encoding="utf-8")
        files.append(messages.relative_to(directory).as_posix())
        archive.stamp(directory, "bench", files)
    return messages


def main() -> None:
    parser = argparse.ArgumentParser(description="Build Benchmarks And Matching Hosts In The Bench Profile")
    parser.add_argument("extra", nargs=argparse.REMAINDER)
    options = parser.parse_args()
    extra = options.extra[1:] if options.extra[:1] == ["--"] else options.extra
    print(build(Path.cwd(), extra))


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
