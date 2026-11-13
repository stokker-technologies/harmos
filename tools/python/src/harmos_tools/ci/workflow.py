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
import re
import shlex
import subprocess
import tempfile
from pathlib import Path

# Third Party Imports
# Local Imports
from ..process import spawn_flags
from ..release import policy
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
    if operation == "release-mode":
        from ..release import review

        if arguments:
            raise ValueError("release-mode takes its inputs from the workflow environment")
        result = review.github_release_mode()
        with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
            output.write("".join(f"{key}={value}\n" for key, value in result.items()))
        return 0
    if operation == "required":
        results = json.loads(os.environ["RESULTS"])
        optional = set(json.loads(os.environ.get("OPTIONAL_JOBS", "[]")))
        if os.environ.get("RELEASE_REVIEW") != "true":
            optional.add("release_rehearsal")
        if any(job["result"] != "success" and not (name in optional and job["result"] == "skipped") for name, job in results.items()):
            raise ValueError("required CI jobs failed or were skipped")
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
    if operation == "nightly-freshness":
        from ..release.review import pages, request

        if os.environ["GITHUB_REF"] != "refs/heads/dev":
            raise ValueError("nightly requires dev")
        build = True
        if os.environ["GITHUB_EVENT_NAME"] == "schedule":
            # A Missing Tag Is The First Nightly; Other API Failures Must Fail Closed.
            result = subprocess.run(
                ["gh", "api", "--include", f"repos/{os.environ['GITHUB_REPOSITORY']}/git/ref/tags/nightly"],
                check=False,
                capture_output=True,
                text=True,
                creationflags=spawn_flags(),
            )
            if result.returncode and not re.search(r"HTTP/\S+ 404", result.stdout) and "HTTP 404" not in result.stderr:
                raise RuntimeError("nightly tag lookup failed")
            if not result.returncode:
                published = request("git/ref/tags/nightly")
                build = published["object"]["sha"] != os.environ["GITHUB_SHA"]
        for status in ("in_progress", "queued"):
            runs = pages(f"actions/workflows/nightly.yml/runs?head_sha={os.environ['GITHUB_SHA']}&status={status}", "workflow_runs")
            if any(int(run["id"]) < int(os.environ["GITHUB_RUN_ID"]) for run in runs):
                build = False
                break
        with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
            output.write(f"build={str(build).lower()}\n")
        return 0
    if operation == "nightly-report":
        from ..release import nightly

        published = nightly.report(policy.root(), json.loads(os.environ["NEEDS"]), os.environ["RUN_URL"])
        with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
            output.write(f"published={str(published).lower()}\n")
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
