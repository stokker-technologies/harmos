# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Run CI Mirror."""

# === Imports === #

# Standard Library Imports
import os
import shlex
import subprocess
import tempfile
from pathlib import Path

# Third Party Imports
# Local Imports
from ..process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def run() -> int:
    branch = os.environ["GITHUB_REF_NAME"]
    if branch not in ("dev", "main") or os.environ["GITHUB_REPOSITORY"] != "stokker-technologies/harmos":
        raise ValueError(
            "mirror requires Harmos dev or main; Run the mirror from the published dev or main revision with its configured SSH credentials"
        )
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True, creationflags=spawn_flags()).strip()
    if head != os.environ["GITHUB_SHA"]:
        raise ValueError(
            "mirror checkout differs from the published SHA; "
            "Run the mirror from the published dev or main revision with its configured SSH credentials"
        )
    key, hosts = os.environ["GITLAB_MIRROR_SSH_KEY"], os.environ["GITLAB_MIRROR_KNOWN_HOSTS"]
    if not key or not hosts:
        raise ValueError(
            "mirror requires its SSH key and known hosts; "
            "Run the mirror from the published dev or main revision with its configured SSH credentials"
        )
    directory_directory = tempfile.TemporaryDirectory(prefix="harmos-mirror-")
    directory = directory_directory.name
    try:
        private = Path(directory) / "key"
        known = Path(directory) / "known_hosts"
        private.touch(mode=0o600)
        private.write_text(key + "\n", encoding="utf-8")
        known.write_text(hosts + "\n", encoding="utf-8")
        environment = dict(os.environ)
        environment["GIT_SSH_COMMAND"] = (
            f"ssh -i {shlex.quote(str(private))} -o IdentitiesOnly=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile={shlex.quote(str(known))}"
        )
        # Run the Native Command
        command = [
            "git",
            "push",
            "--no-force",
            "--no-follow-tags",
            "git@gitlab.com:stokker-technologies/open-source/harmos.git",
            f"HEAD:refs/heads/{branch}",
        ]
        subprocess.run(
            command,
            env=environment,
            creationflags=spawn_flags(),
            check=True,
        )
    finally:
        directory_directory.cleanup()
    return 0


# === Private Functions === #

# === Entry Point === #
