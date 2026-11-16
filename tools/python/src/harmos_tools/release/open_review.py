# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Release Open Review."""

# === Imports === #

# Standard Library Imports
import base64
import json
import os
import subprocess
import sys
import urllib.parse
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.release.source

from ..package.changelog import notes
from ..process import spawn_flags
from . import policy, registry, version

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(directory: Path, settings: dict) -> None:
    """Commit The Prepared Tree To release/v<version> And Open Its Review Into dev With The Release Token."""
    token = os.environ.get("RELEASE_TOKEN", "")
    if not harmos_tools.release.source.continuous_integration() or not token:
        raise policy.ToolError("the release review opens from Release / Prepare with the RELEASE_TOKEN secret")
    tag = harmos_tools.release.source.release_tag(directory)
    version.run(directory, "check")
    branch = registry.RELEASE_BRANCH + tag
    if settings["provider"] == "github":
        os.environ["GH_TOKEN"] = token
    endpoint = "git/ref/heads/" if settings["provider"] == "github" else "repository/branches/"
    if registry.document(settings, endpoint + urllib.parse.quote(branch, safe="")):
        raise policy.ToolError(f"{branch} already exists; merge or close its review before preparing again")
    title = f"chore(release): prepare {tag}"
    policy.execute(directory, ["git", "add", "--all"])
    commit(directory, settings, branch, title)
    body = policy.inside(directory, "target/release-notes.md")
    body.parent.mkdir(parents=True, exist_ok=True)
    body.write_text(notes(directory, tag, tagged=False), encoding="utf-8")
    if settings["provider"] == "github":
        command = ["gh", "pr", "create", "--base", "dev", "--head", branch, "--title", title, "--body-file", str(body)]
        environment = {**os.environ, "GH_TOKEN": token}
    else:
        command = [
            "glab",
            "mr",
            "create",
            "--source-branch",
            branch,
            "--target-branch",
            "dev",
            "--title",
            title,
            "--description",
            body.read_text(encoding="utf-8"),
            "--squash-before-merge",
            "--remove-source-branch",
            "--yes",
        ]
        environment = {**os.environ, "GITLAB_TOKEN": token}
    if subprocess.run(
        command,
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
        creationflags=spawn_flags(),
        cwd=directory,
        env=environment,
        check=False,
    ).returncode:
        raise policy.ToolError(f"opening the release review failed; {branch} is pushed and can be reviewed by hand")
    registry.arm_auto_merge(settings, branch, title)


def commit(directory: Path, settings: dict, branch: str, title: str) -> None:
    """Create The Preparation Commit Through The Provider API, Signed By The Provider."""
    head = harmos_tools.release.source.revision(directory)
    changed = policy.execute(directory, ["git", "diff", "--cached", "--name-status", "--no-renames", "-z", "HEAD"], capture=True)
    entries = changed.rstrip("\0").split("\0") if changed else []
    additions, deletions, actions = [], [], []
    for status, name in zip(entries[::2], entries[1::2], strict=True):
        if status == "D":
            deletions.append({"path": name})
            actions.append({"action": "delete", "file_path": name})
            continue
        path = policy.inside(directory, name)
        if status not in {"A", "M"} or path.is_symlink() or not path.is_file():
            raise policy.ToolError(f"unsupported release file change: {status} {name}")
        content = base64.b64encode(path.read_bytes()).decode("ascii")
        additions.append({"path": name, "contents": content})
        actions.append({"action": "create" if status == "A" else "update", "file_path": name, "content": content, "encoding": "base64"})
    if settings["provider"] == "github":
        registry.document(settings, "git/refs", "POST", {"ref": "refs/heads/" + branch, "sha": head})
        payload = {
            "query": "mutation($input: CreateCommitOnBranchInput!) { createCommitOnBranch(input: $input) { commit { oid signature { isValid } } } }",
            "variables": {
                "input": {
                    "branch": {
                        "repositoryNameWithOwner": settings.get("repository", os.environ.get("GITHUB_REPOSITORY")),
                        "branchName": branch,
                    },
                    "expectedHeadOid": head,
                    "message": {"headline": title},
                    "fileChanges": {"additions": additions, "deletions": deletions},
                }
            },
        }
        response = json.loads(registry.request(settings, "graphql", "POST", json.dumps(payload).encode(), graphql=True) or b"{}")
        if response.get("errors"):
            raise policy.ToolError("createCommitOnBranch failed: " + json.dumps(response["errors"]))
        commit = response.get("data", {}).get("createCommitOnBranch", {}).get("commit", {})
        verified = (commit.get("signature") or {}).get("isValid")
    else:
        # start_sha creates the new branch and its commit together; never force an existing branch.
        commit = registry.document(
            settings,
            "repository/commits",
            "POST",
            {
                "branch": branch,
                "start_sha": head,
                "commit_message": title,
                "actions": actions,
                "allow_empty": True,
            },
        )
        if commit is None:
            raise policy.ToolError("preparation commit returned no document")
        signature = registry.document(settings, "repository/commits/" + urllib.parse.quote(commit["id"], safe="") + "/signature")
        verified = signature and signature.get("verification_status") in {"verified", "verified_system"}
        if not verified:
            registry.request(settings, "repository/branches/" + urllib.parse.quote(branch, safe=""), "DELETE")
    if not verified:
        raise policy.ToolError("preparation API commit is not verified; GitLab requires web-based commit signing enabled")


# === Private Functions === #

# === Entry Point === #
