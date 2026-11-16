# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Publish Rust Crates."""

# === Imports === #

# Standard Library Imports
import email.utils
import json
import os
import re
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
from collections.abc import Callable
from pathlib import Path

from ..package import changelog

# Third Party Imports
# Local Imports
from ..process import spawn_flags
from . import pins, policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

CRATES_INDEX = "https://index.crates.io"
RATE_INTERVAL = 600
RATE_BUDGET = 5400


# === Types === #

# === Classes === #

# === Public Functions === #


def publish(directory: Path, *, sleep: Callable[[float], None] = time.sleep, clock: Callable[[], float] = time.monotonic) -> None:
    """Publish Workspace Crates One At A Time In Dependency Order, Resuming Past Published Versions."""
    if policy.execute(directory, ["git", "status", "--porcelain"], capture=True).strip():
        raise policy.ToolError("crate publication requires clean source before generated CHANGELOG staging")
    pins.check(directory)
    packages = publishable(directory)
    deadline = clock() + RATE_BUDGET
    staged_notes = changelog.StagedNotes(directory, packages)
    try:
        for name in publication_order(packages):
            version = packages[name]["version"]
            if published(name, version):
                print(f"{name} {version}: already published")
                continue
            while True:
                # Run the Native Command
                command = ["cargo", "publish", "--locked", "--allow-dirty", "--package", name]
                result = subprocess.run(
                    command,
                    creationflags=spawn_flags(),
                    cwd=directory,
                    capture_output=True,
                    text=True,
                    encoding="utf-8",
                    check=False,
                )
                output = (result.stdout or "") + (result.stderr or "")
                if result.returncode == 0:
                    print(f"{name} {version}: published")
                    break
                if "already exists" in output:
                    print(f"{name} {version}: already published")
                    break
                if "429" not in output and "Too Many Requests" not in output:
                    raise policy.ToolError(f"cargo publish {name} failed with exit code {result.returncode}: {output}")
                wait = retry_after(output)
                if clock() + wait > deadline:
                    raise policy.ToolError(f"{name} {version}: crates.io rate limit outlasted the publication budget")
                print(f"{name} {version}: rate limited; retrying in {wait}s")
                sleep(wait)
    finally:
        staged_notes.close()


def rehearse(directory: Path, *, skip_published: bool = True) -> None:
    """Dry-Run Crates In Publication Order Without Publishing Credentials."""
    nightly = os.environ.get("GITHUB_WORKFLOW_REF", "").split("/workflows/")[-1].startswith("nightly.yml@")
    nightly = nightly or os.environ.get("CI_SIMULATE") == "1" and os.environ.get("CI_PIPELINE_KIND") == "nightly"
    if not pins.check(directory, warn=nightly):
        return
    credentials = policy.CredentialScope(policy.policy(directory, "release"))
    try:
        packages = publishable(directory)
        metadata = policy.execute(directory, ["cargo", "metadata", "--format-version", "1", "--no-deps"], capture=True)
        target = Path(json.loads(metadata)["target_directory"])
        lock = directory / "Cargo.lock"
        original = lock.read_bytes() if lock.is_file() else None
        staged = []
        staged_notes = changelog.StagedNotes(directory, packages)
        try:
            try:
                for name in publication_order(packages):
                    version = packages[name]["version"]
                    if skip_published and published(name, version):
                        print(f"{name} {version}: already published; the real leaf skips it")
                        continue
                    # A sibling resolves from its packaged sources only once the real leaf would have published it, so a
                    # crate ordered before something its published manifest requires fails here, before any tag exists.
                    command = ["cargo", "publish", "--dry-run", "--allow-dirty", "--package", name]
                    for sibling, release in staged:
                        command += [
                            "--config",
                            f'patch.crates-io.{sibling}.path="{(target / "package" / f"{sibling}-{release}").as_posix()}"',
                        ]
                    policy.execute(directory, command)
                    staged.append((name, version))
            finally:
                # The patches record themselves in the lock file; the rehearsal leaves the committed lock untouched.
                if original is not None:
                    lock.write_bytes(original)
        finally:
            staged_notes.close()
    finally:
        credentials.close()


def publishable(directory: Path) -> dict:
    """Publishable Workspace Members By Name."""
    # Read Structured Input
    text = policy.execute(directory, ["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"], capture=True)
    metadata = json.loads(text)
    return policy.publishable(directory, metadata)


def published(name: str, version: str | None = None) -> bool:
    """Whether The Sparse Index Lists This Crate, Or This Exact Crate Version."""
    lower = name.lower()
    prefix = {1: "1", 2: "2", 3: "3/" + lower[0]}.get(len(lower), f"{lower[0:2]}/{lower[2:4]}")
    try:
        response = urllib.request.urlopen(f"{CRATES_INDEX}/{prefix}/{lower}", timeout=60)
        try:
            lines = response.read().decode("utf-8").splitlines()
        finally:
            response.close()
    except urllib.error.HTTPError as error:
        error.close()
        if error.code == 404:
            return False
        raise policy.ToolError(f"crates.io index read failed: HTTP {error.code}") from error
    except urllib.error.URLError as error:
        if isinstance(error.reason, FileNotFoundError):
            return False
        raise policy.ToolError(f"crates.io index read failed: {error.reason}") from error
    entries = [json.loads(line) for line in lines if line.strip()]
    return bool(entries) if version is None else any(entry.get("vers") == version for entry in entries)


def publication_order(packages: dict) -> list[str]:
    """Publishable Crates After Every Publishable Crate Their Published Manifest Requires, Ties By Name.

    A dev-dependency that carries a version stays in the published manifest and must resolve from the registry; a
    path-only one is stripped on packaging.
    """
    requires = {
        name: sorted(
            {
                dependency["name"]
                for dependency in package["dependencies"]
                if (dependency.get("kind") != "dev" or dependency.get("req", "*") != "*")
                and dependency["name"] in packages
                and dependency["name"] != name
            }
        )
        for name, package in packages.items()
    }
    order = []
    visiting = set()

    def visit(name: str) -> None:
        if name in order:
            return
        if name in visiting:
            cycle = ", ".join(sorted(visiting))
            raise policy.ToolError(
                f"publishable crates {cycle} require each other at {name}; a dev-dependency on a sibling that carries a version stays in the "
                'published manifest, so declare it path-only ({ path = "../sibling" }, no version) and cargo strips it on packaging'
            )
        visiting.add(name)
        for dependency in requires[name]:
            visit(dependency)
        visiting.discard(name)
        order.append(name)

    for name in sorted(packages):
        visit(name)
    return order


def retry_after(output: str) -> int:
    """Seconds Until The Registry-Indicated Retry Time, Or The Published Interval."""
    match = re.search(r"try again after ([A-Z][a-z]{2}, \d{1,2} [A-Z][a-z]{2} \d{4} \d{2}:\d{2}:\d{2} GMT)", output)
    if match is None:
        return RATE_INTERVAL
    retry = email.utils.parsedate_to_datetime(match.group(1)).timestamp()
    return max(1, min(RATE_INTERVAL * 2, int(retry - time.time()) + 1))


# === Private Functions === #

# === Entry Point === #
