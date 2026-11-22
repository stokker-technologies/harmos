# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Collect Packages Under Their Matching Profile And Native Target"""

# === Imports === #

# Standard Library Imports
import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import zipfile
from pathlib import Path

# Third Party Imports
# Local Imports
from .. import temporary
from ..build import paths
from ..process import spawn_flags
from ..release import policy
from . import changelog

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def definitions(directory: Path, profile: str, target: str) -> list[dict]:
    """Resolve Declared Products Without Looking At Native Compiler Directories."""
    config = policy.config(directory)
    version = config.get("version", {}).get("value")
    result = []
    for item in config.get("package", {}).get("executables", []):
        triple = item["targets"].get(target)
        if triple is None:
            continue
        if not version or not re.fullmatch(r"[a-zA-Z0-9_.-]+", version):
            raise policy.ToolError("executable packages require version.value")
        extension = ".exe" if "-windows-" in triple else ""
        archive = ".zip" if extension else ".tar.gz"
        name = f"{item['name']}-{version}-{triple}{archive}"
        result.append({**item, "language": "rust", "output": name, "triple": triple, "binary": item["command"] + extension})
    for item in config.get("package", {}).get("bundles", []):
        if profile in item["profiles"] and target in item["targets"]:
            fields = [item["name"], *(entry[key] for entry in item["inputs"] for key in ("path", "destination"))]
            if any("{version}" in field for field in fields) and (
                not isinstance(version, str) or re.fullmatch(r"[A-Za-z0-9_.-]+", version) is None or version in {".", ".."}
            ):
                raise policy.ToolError("versioned bundles require a safe version.value")
            result.append({**item, "output": item["name"].format(version=version, target=target)})
    names = [item["output"].casefold() for item in result]
    if len(names) != len(set(names)):
        raise policy.ToolError("package output names collide")
    for item in result:
        name = item["output"]
        if re.fullmatch(r"[A-Za-z0-9_.-]+", name) is None or name in {".", "..", "SHA256SUMS", "release.json", "platform.json"}:
            raise policy.ToolError("unsafe or reserved package output name")
    return result


def packaged(directory: Path, profile: str, target: str) -> list[dict]:
    """Only Explicitly Shipped Tools Enter The Release; Bundles Need release.assets."""
    root = paths.output(directory, "package", profile, target)
    result = [
        {"target": target, "path": (root / item["language"] / item["output"]).relative_to(directory).as_posix(), "name": item["output"]}
        for item in definitions(directory, profile, target)
        if "triple" in item and item["release"]
    ]
    return result


def regular(root: Path, relative: str) -> Path:
    parts = relative.split("/")
    if not relative or any(part in {"", ".", ".."} for part in parts) or "\\" in relative or ":" in relative:
        raise policy.ToolError(f"unsafe package input: {relative}")
    source = policy.inside(root, relative)
    if not source.is_file() or any(path.is_symlink() for path in (source, *source.parents) if path.is_relative_to(root)):
        raise policy.ToolError(f"missing or linked package input: {relative}")
    return source


def assemble(directory: Path, language: str, profile: str, target: str, staging: Path) -> None:
    build = paths.output(directory, "build", profile, target)
    version = policy.config(directory).get("version", {}).get("value")
    if not isinstance(version, str):
        raise policy.ToolError("package version must be a string")
    substitutions = {"profile": profile, "target": target, "version": version}
    if language in {"python", "typescript"}:
        source = build / {"python": "python", "typescript": "web"}[language]
        canonical = policy.config(directory).get("languages", {}).get("python", {}).get("build_target") == "any"
        if (target != "any" or language == "python" and canonical) and not source.is_dir():
            raise policy.ToolError(f"missing Product Build input: {source}")
        for entry in source.rglob("*"):
            if entry.is_symlink():
                raise policy.ToolError("linked package input")
            if not entry.is_file():
                continue
            relative = entry.relative_to(source)
            original = regular(source, relative.as_posix())
            destination = staging / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(original, destination)
    for item in definitions(directory, profile, target):
        if item["language"] != language:
            continue
        output = staging / item["output"]
        if "triple" in item:
            actual = regular(build, "product/target.txt").read_text().strip()
            if actual != item["triple"]:
                raise policy.ToolError(f"Build Rust target {actual} differs from package target {item['triple']}")
            source = regular(build, "bin/" + item["binary"])
            documentation = [(regular(directory, "LICENSE"), "LICENSE"), (regular(directory, "README.md"), "README.md")]
            value = substitutions["version"]
            changelog_file = staging / "CHANGELOG.md"
            changelog_file.write_text(changelog.notes(directory, "v" + value), encoding="utf-8", newline="\n")
            documentation.append((changelog_file, "CHANGELOG.md"))
            contents = [(source, item["binary"]), *documentation]
            if output.name.endswith(".zip"):
                archive = zipfile.ZipFile(output, "x", compression=zipfile.ZIP_DEFLATED)
                try:
                    for original, name in contents:
                        metadata = zipfile.ZipInfo.from_file(original, name)
                        metadata.create_system = 3
                        metadata.external_attr = (stat.S_IFREG | (0o755 if original == source else 0o644)) << 16
                        metadata.compress_type = zipfile.ZIP_DEFLATED
                        archive.writestr(metadata, original.read_bytes())
                finally:
                    archive.close()
            else:
                archive = tarfile.open(output, "x:gz")
                try:
                    for original, name in contents:
                        metadata = archive.gettarinfo(str(original), arcname=name)
                        metadata.mode = 0o755 if original == source else 0o644
                        stream = original.open("rb")
                        try:
                            archive.addfile(metadata, stream)
                        finally:
                            stream.close()
                finally:
                    archive.close()
            changelog_file.unlink()
            continue
        destinations = set()
        archive = zipfile.ZipFile(output, "x", compression=zipfile.ZIP_DEFLATED)
        try:
            for entry in item["inputs"]:
                prefix = "target/build/{profile}/{target}/"
                if not entry["path"].startswith(prefix):
                    raise policy.ToolError("bundle inputs must come from the matching Product Build")
                relative = entry["path"][len(prefix) :].format(**substitutions)
                source = regular(build, relative)
                name = entry["destination"].format(**substitutions)
                parts = name.split("/")
                unsafe = (
                    not name
                    or any(part in {"", ".", ".."} for part in parts)
                    or "\\" in name
                    or ":" in name
                    or any(ord(char) < 32 for char in name)
                )
                overlaps = any(
                    name.casefold() == old or name.casefold().startswith(old + "/") or old.startswith(name.casefold() + "/")
                    for old in destinations
                )
                if unsafe or overlaps:
                    raise policy.ToolError("unsafe or duplicate bundle destination")
                destinations.add(name.casefold())
                mode = entry.get("mode")
                if mode is None:
                    archive.write(source, name)
                    continue
                if type(mode) is not int or not 0 <= mode <= 0o777:
                    raise policy.ToolError("bundle mode must contain only ordinary permission bits")
                metadata = zipfile.ZipInfo.from_file(source, name)
                metadata.create_system = 3
                metadata.external_attr = (stat.S_IFREG | mode) << 16
                metadata.compress_type = zipfile.ZIP_DEFLATED
                original = source.open("rb")
                try:
                    destination = archive.open(metadata, "w")
                    try:
                        shutil.copyfileobj(original, destination)
                    finally:
                        destination.close()
                finally:
                    original.close()
        finally:
            archive.close()


def binary_bytes(source: Path, name: str) -> bytes:
    """Read Only The Declared Root Binary, Without Extracting Archive Paths."""
    if source.name.endswith(".zip"):
        archive = zipfile.ZipFile(source)
        try:
            matches = [entry for entry in archive.infolist() if entry.filename == name]
            if len(matches) != 1 or matches[0].is_dir():
                raise policy.ToolError("tool archive must contain one declared binary")
            return archive.read(matches[0])
        finally:
            archive.close()
    archive = tarfile.open(source, "r:gz")
    try:
        matches = [entry for entry in archive.getmembers() if entry.name == name]
        if len(matches) != 1 or not matches[0].isfile() or not matches[0].mode & 0o111:
            raise policy.ToolError("tool archive must contain one executable regular binary")
        stream = archive.extractfile(matches[0])
        if stream is None:
            raise policy.ToolError("tool archive binary has no file content")
        try:
            return stream.read()
        finally:
            stream.close()
    finally:
        archive.close()


def verify(directory: Path, profile: str, target: str, root: Path | None = None) -> None:
    """Execute Packaged Native Bytes In An Isolated Installation Directory."""
    for item in definitions(directory, profile, target):
        # Staged release downloads hold only the executables that ship.
        if "triple" not in item or root is not None and not item["release"]:
            continue
        source = (root / item["output"]) if root else paths.output(directory, "package", profile, target) / "rust" / item["output"]
        if not source.is_file() or source.is_symlink():
            raise policy.ToolError("missing packaged executable")
        install_directory = temporary.Directory()
        install = install_directory.path
        try:
            executable = install / item["binary"]
            executable.write_bytes(binary_bytes(source, item["binary"]))
            executable.chmod(0o755)
            # Run the Native Command
            command = [str(executable), *item["verify"]]
            subprocess.run(
                command,
                stdin=sys.stdin,
                stdout=sys.stdout,
                stderr=sys.stderr,
                creationflags=spawn_flags(),
                cwd=install,
                check=True,
            )
        finally:
            install_directory.close()


def installed(directory: Path, settings: dict, root: Path, target: str) -> None:
    """Install From Published Assets And Prove No Source Or Third-Party Fallback."""
    for item in definitions(directory, "release", target):
        if not item.get("installers"):
            continue
        version = settings["tag"].removeprefix("v")
        provider = settings["provider"]
        repository = settings.get("repository") if provider == "github" else settings["project_id"]
        for installer in item["installers"]:
            install_directory = temporary.Directory()
            install = install_directory.path
            try:
                if installer == "mise":
                    tool = f"{provider}:{repository}[asset_pattern={item['output']},bin={item['binary']}]@{version}"
                    environment = os.environ.copy()
                    environment.pop("MISE_ENABLE_TOOLS", None)
                    # Run the Native Command
                    command = ["mise", "install-into", tool, str(install)]
                    subprocess.run(
                        command,
                        stdin=sys.stdin,
                        stdout=sys.stdout,
                        stderr=sys.stderr,
                        creationflags=spawn_flags(),
                        cwd=install.parent,
                        env=environment,
                        check=True,
                    )
                    matches = list(install.rglob(item["binary"]))
                else:
                    command = [
                        "cargo",
                        "binstall",
                        item["package"],
                        "--version",
                        "=" + version,
                        "--target",
                        item["triple"],
                        "--root",
                        str(install),
                        "--no-confirm",
                        "--disable-strategies",
                        "quick-install,compile",
                        "--strategies",
                        "crate-meta-data",
                    ]
                    subprocess.run(
                        command,
                        stdin=sys.stdin,
                        stdout=sys.stdout,
                        stderr=sys.stderr,
                        creationflags=spawn_flags(),
                        cwd=install,
                        check=True,
                    )
                    matches = [install / "bin" / item["binary"]]
                if len(matches) != 1 or not matches[0].is_file():
                    raise policy.ToolError(f"{installer} did not install the declared executable")
                expected = hashlib.sha256(binary_bytes(root / item["output"], item["binary"])).digest()
                if hashlib.sha256(matches[0].read_bytes()).digest() != expected:
                    raise policy.ToolError(f"{installer} did not install the exact published asset")
                # Run the Native Command
                command = [str(matches[0]), *item["verify"]]
                subprocess.run(
                    command,
                    stdin=sys.stdin,
                    stdout=sys.stdout,
                    stderr=sys.stderr,
                    creationflags=spawn_flags(),
                    cwd=install,
                    check=True,
                )
            finally:
                install_directory.close()


def main() -> None:
    parser = argparse.ArgumentParser(description="Assemble Existing Build Outputs")
    parser.add_argument("language", choices=("rust", "python", "typescript", "native"))
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--target")
    options = parser.parse_args()
    if options.target == "native":
        options.target = None
    directory = Path.cwd()
    destination = paths.output(directory, "package", options.profile, options.target)
    target = options.target or paths.target()
    language = "rust" if options.language == "native" else options.language
    products = definitions(directory, options.profile, target)
    if target == "any" and not any(item["language"] == language for item in products) and language != "python":
        staging_directory = temporary.Directory()
        staging = staging_directory.path
        try:
            paths.replace(directory, staging, destination / language, origin=staging)
        finally:
            staging_directory.close()
        return
    if any(item["language"] == language for item in products):
        staging_directory = temporary.Directory()
        staging = staging_directory.path
        try:
            assemble(directory, language, options.profile, target, staging)
            existing = paths.output(directory, "package", options.profile, target)
            for item in products:
                if item["language"] != language:
                    continue
                foreign = [path for path in existing.glob("*/" + item["output"]) if path.parent.name != language]
                if foreign:
                    raise policy.ToolError("package output collides with another language")
            paths.replace(directory, staging, destination / language, origin=staging)
        finally:
            staging_directory.close()
        if language == "rust":
            verify(directory, options.profile, target)
        return
    if options.language == "rust":
        # Read Structured Input
        text = policy.execute(directory, ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"], capture=True)
        metadata = json.loads(text)
        members = set(metadata["workspace_members"])
        crates = {item["name"]: item for item in metadata["packages"] if item["id"] in members and item.get("publish") != []}
        scratch_directory = temporary.Directory()
        scratch = scratch_directory.path
        try:
            staged_notes = changelog.StagedNotes(directory, crates)
            try:
                # Run the Native Command
                command = [
                    "cargo",
                    "package",
                    "--workspace",
                    "--locked",
                    "--no-verify",
                    "--allow-dirty",
                    "--target-dir",
                    str(scratch),
                ]
                subprocess.run(
                    command,
                    stdin=sys.stdin,
                    stdout=sys.stdout,
                    stderr=sys.stderr,
                    creationflags=spawn_flags(),
                    cwd=directory,
                    check=True,
                )
                paths.replace(
                    directory,
                    scratch / "package",
                    destination / options.language,
                    origin=scratch,
                )
            finally:
                staged_notes.close()
        finally:
            scratch_directory.close()
        return
    else:
        kind = {"python": "python", "typescript": "web", "native": "bin"}[options.language]
        source = paths.output(directory, "build", options.profile, options.target) / kind
    if options.language == "python" and not any(source.glob("*.whl")):
        raise policy.ToolError("missing Python wheel; Run mise run build -- --package harmos-sdk before packaging")
    paths.replace(directory, source, destination / options.language)


# === Private Functions === #


# === Entry Point === #

if __name__ == "__main__":
    main()
