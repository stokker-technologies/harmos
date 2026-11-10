# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Own Artifact Transport, Nightly Bookkeeping And The Published Ref Mirror."""

# === Imports === #

# Standard Library Imports
import json
import os
import shlex
import subprocess
import tempfile
from pathlib import Path

# Third Party Imports

# Local Imports
from ..process import spawn_flags
from ..release import policy, registry
from . import summary

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def run(operation: str, arguments: list[str], profile: str) -> int:
    arguments = arguments[1:] if arguments[:1] == ["--"] else arguments
    if operation in ("review", "release-checks"):
        from .tasks import _module

        module = "release.review" if operation == "review" else "release.checks"
        return _module(module, *arguments)
    if operation == "required":
        results = json.loads(os.environ["RESULTS"])
        if any(job["result"] not in ("success", "skipped") for job in results.values()):
            raise ValueError("selected CI jobs failed")
        return 0
    if operation in ("pack", "extract", "runners"):
        from .tasks import _module

        module = "test.runners" if operation == "runners" else "ci.artifacts"
        extra = [] if operation == "runners" else [operation, "--profile", profile]
        return _module(module, *extra, *arguments)
    if arguments:
        raise ValueError(f"{operation} takes its inputs from the workflow environment")
    if operation == "summary":
        caches = json.loads(os.environ.get("CACHE_RESTORES", "{}"))
        summary.render("Cache Summary", None, None, caches={name: {"Restore": value} for name, value in caches.items()})
        return 0
    if operation == "nightly-gate":
        build = True
        if os.environ["GITHUB_EVENT_NAME"] == "schedule":
            result = subprocess.run(
                ["gh", "api", f"repos/{os.environ['GITHUB_REPOSITORY']}/git/ref/tags/nightly", "--jq", ".object.sha"],
                capture_output=True,
                text=True,
                creationflags=spawn_flags(),
                check=False,
            )
            build = result.returncode != 0 or result.stdout.strip() != os.environ["GITHUB_SHA"]
        with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
            output.write(f"build={str(build).lower()}\n")
        return 0
    if operation == "nightly-report":
        root = policy.root()
        settings = policy.policy(root, "release")
        current = registry.document(settings, "releases/tags/nightly", channel=True)
        notes = current["body"]
        url = (
            os.environ.get("RUN_URL")
            or f"{os.environ.get('GITHUB_SERVER_URL', 'https://github.com')}/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{os.environ['GITHUB_RUN_ID']}"
        )
        if f"({url})" not in notes:
            print("A Newer Run Replaced Nightly; Leaving Its Notes Unchanged")
            return 0
        suite = os.environ["SUITE"]
        if suite not in ("passed", "failed"):
            raise ValueError("SUITE must be passed or failed")
        registry.document(settings, f"releases/{current['id']}", "PATCH", {"body": notes + f"\n**Suite:** {suite}, {url}\n"}, channel=True)
        return 0
    if operation == "mirror":
        branch = os.environ["GITHUB_REF_NAME"]
        if branch not in ("dev", "main") or os.environ["GITHUB_REPOSITORY"] != "stokker-technologies/harmos":
            raise ValueError("mirror requires Harmos dev or main")
        head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True, creationflags=spawn_flags()).strip()
        if head != os.environ["GITHUB_SHA"]:
            raise ValueError("mirror checkout differs from the published SHA")
        key, hosts = os.environ["GITLAB_MIRROR_SSH_KEY"], os.environ["GITLAB_MIRROR_KNOWN_HOSTS"]
        if not key or not hosts:
            raise ValueError("mirror requires its SSH key and known hosts")
        with tempfile.TemporaryDirectory(prefix="harmos-mirror-") as directory:
            private = Path(directory) / "key"
            known = Path(directory) / "known_hosts"
            private.touch(mode=0o600)
            private.write_text(key + "\n", encoding="utf-8")
            known.write_text(hosts + "\n", encoding="utf-8")
            environment = dict(os.environ)
            environment["GIT_SSH_COMMAND"] = (
                f"ssh -i {shlex.quote(str(private))} -o IdentitiesOnly=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile={shlex.quote(str(known))}"
            )
            subprocess.run(
                [
                    "git",
                    "push",
                    "--no-force",
                    "--no-follow-tags",
                    "git@gitlab.com:stokker-technologies/open-source/harmos.git",
                    f"HEAD:refs/heads/{branch}",
                ],
                env=environment,
                creationflags=spawn_flags(),
                check=True,
            )
        return 0
    raise ValueError(f"unknown workflow operation: {operation}")


# === Private Functions === #

# === Entry Point === #
