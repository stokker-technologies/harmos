# Rendered by stkr project update from templates/ci/extract-build; sha256:79ab145a1e1268f3da0181bfa0719fadef4f66decc2ce28dbfed5b3b093bc556
"""Transfer Declared Build Outputs Through The Repository Artifact Task."""

# === Imports === #

# Standard Library Imports
import contextlib
import glob
import json
import os
import shutil
import subprocess
import tarfile
from pathlib import Path

# Third Party Imports
# Local Imports
# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


@contextlib.contextmanager
def group(title):
    print(f"::group::{title}", flush=True)
    try:
        yield
    finally:
        print("::endgroup::", flush=True)


@group("Build / Archive")
def export(root: Path, outputs: dict, archive: Path) -> None:
    archive.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive, "w") as transport:
        for output, patterns in outputs.items():
            for pattern in patterns:
                selected = glob.glob(pattern.rstrip("/"), root_dir=root, recursive=True)
                if not selected:
                    raise ValueError(f"required output missing: {pattern}")
                for name in selected:
                    path = root / name
                    for file in [path] if path.is_file() else [path, *path.rglob("*")]:
                        if not file.resolve().is_relative_to(root.resolve()) or file.is_symlink():
                            raise ValueError("artifact escapes job workspace")
                        if file.is_file() or file.is_dir():
                            transport.add(file, arcname=f"{output}/{file.relative_to(root).as_posix()}", recursive=False)


@group("Build / Extract")
def transport(archive: Path, destination: Path) -> None:
    modes = {}
    directories = []
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive) as source:
        for member in source:
            path = destination / member.name
            if (
                not (member.isfile() or member.isdir())
                or not relative_path(member.name)
                or not path.resolve().is_relative_to(destination.resolve())
            ):
                raise ValueError("unsafe local artifact")
            modes[member.name] = {"type": "file" if member.isfile() else "directory", "mode": member.mode & 0o777}
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
                directories.append((path, member.mode & 0o777))
                continue
            path.parent.mkdir(parents=True, exist_ok=True)
            data = source.extractfile(member)
            if data is None:
                raise ValueError("artifact member has no file content")
            with data, path.open("wb") as target:
                shutil.copyfileobj(data, target)
            path.chmod(member.mode & 0o777)
    for path, mode in sorted(directories, key=lambda item: len(item[0].parts), reverse=True):
        path.chmod(mode)
    destination.with_name(destination.name + ".modes.json").write_text(json.dumps({"version": 1, "files": modes}), encoding="utf-8")

    artifact_modes(destination.with_name(destination.name + ".modes.json"))
    archive.unlink()


def relative_path(name: str) -> bool:
    return (
        bool(name)
        and not name.startswith("/")
        and not any(part in ("", ".", "..") for part in name.split("/"))
        and "\\" not in name
        and ":" not in name
    )


def artifact_modes(path: Path) -> dict:
    document = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(document, dict) or document.get("version") != 1 or not isinstance(document.get("files"), dict):
        raise ValueError("invalid artifact mode metadata")
    result = {}
    for name, item in document["files"].items():
        if (
            not relative_path(name)
            or not isinstance(item, dict)
            or item.get("type") not in ("file", "directory")
            or type(item.get("mode")) is not int
            or not 0 <= item["mode"] <= 0o777
        ):
            raise ValueError("unsafe artifact mode metadata")
        root = path.with_name(path.name.removesuffix(".modes.json"))
        entry = root / name
        if (
            entry.is_symlink()
            or not entry.resolve().is_relative_to(root.resolve())
            or not (entry.is_file() if item["type"] == "file" else entry.is_dir())
        ):
            raise ValueError("artifact mode metadata does not match the retained artifact")
        result[name] = item["mode"]
    return result


def main():
    operation = os.environ["TRANSFER_OPERATION"]
    if operation not in ("pack", "extract"):
        raise ValueError("unknown artifact operation")
    print(f"::group::Build / {operation.title()}", flush=True)
    try:
        subprocess.run(json.loads(os.environ["TRANSFER_COMMAND"]), check=True)
    finally:
        print("::endgroup::", flush=True)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
