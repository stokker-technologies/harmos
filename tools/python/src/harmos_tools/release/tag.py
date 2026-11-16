# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Release Tag."""

# === Imports === #

# Standard Library Imports
import os
import subprocess
import sys
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.release.source

from ..process import spawn_flags
from . import pins, policy, registry

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(directory: Path, settings: dict) -> None:
    """Tag An Exactly Rehearsed Merged Release Review, Then Fast-Forward main."""
    sha = harmos_tools.release.source.revision(directory)
    event = resolve_event(directory, settings, sha)
    pins.check(directory)
    review = registry.release_review(settings, sha, registry.RELEASE_BRANCH + harmos_tools.release.source.release_tag(directory))
    if review is None:
        raise policy.ToolError("the revision has no merged release review")
    if event is not None and (review["number"] != event["number"] or review["head"] != event["head"]["sha"]):
        raise policy.ToolError("the merged release review differs from the GitHub event")
    name = harmos_tools.release.source.release_tag(directory)
    existing = registry.remote_tag(settings, name)
    if existing and existing != sha:
        raise policy.ToolError(f"{name} already identifies another revision; never move a release tag")
    # Only products that render the rehearsal can have rehearsed their review; for them no tag exists without it.
    if harmos_tools.release.source.task_defined(directory, "release:rehearse"):
        registry.rehearsed(directory, settings, review, sha)
    else:
        registry._same_tree(directory, review["head"], sha)
        print("note: this product renders no rehearsal; Release / Dry verified the review")
    token = os.environ.get("RELEASE_TOKEN", "")
    if not token:
        raise policy.ToolError("tagging requires the workflow-triggering RELEASE_TOKEN secret")
    # Reject known divergence before any immutable tag or registry publication can occur.
    remote = harmos_tools.release.source.push_remote(settings, token)
    harmos_tools.release.source.execute(directory, ["git", "fetch", "--quiet", remote, "refs/heads/main"], token)
    if subprocess.run(
        ["git", "merge-base", "--is-ancestor", "FETCH_HEAD", sha],
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
        creationflags=spawn_flags(),
        cwd=directory,
        check=False,
    ).returncode:
        raise policy.ToolError(f"main is not an ancestor of {name}; it must only ever fast-forward to release tags")
    if existing is None:
        # The release token's tag push starts the tag pipeline; the native job token's would not.
        registry.create_tag(settings, name, sha)
    if registry.remote_tag(settings, name) != sha:
        raise policy.ToolError(f"{name} was not created at {sha}")
    if harmos_tools.release.source.task_defined(directory, "release:publish:rust"):
        policy.execute(directory, ["mise", "run", "release:publish:rust"])
    # main is the last released commit: a fast-forward to the tag, never a merge or a force.
    harmos_tools.release.source.execute(directory, ["git", "push", remote, f"{sha}:refs/heads/main"], token)
    print(f"{name}: tagged at {sha}; main fast-forwarded")
    branch = registry.RELEASE_BRANCH + name
    if registry.delete_branch(settings, branch):
        print(f"{branch}: deleted after its squash")


def resolve_event(directory: Path, settings: dict, sha: str) -> dict | None:
    if settings["provider"] == "gitlab":
        source = os.environ.get("PIPELINE_SOURCE", os.environ.get("CI_PIPELINE_SOURCE"))
        if source != "web" or os.environ.get("CI_COMMIT_BRANCH") != "dev" or os.environ.get("CI_COMMIT_SHA") != sha:
            raise policy.ToolError("GitLab tagging requires a web-started dev release pipeline at the checkout revision")
        return None
    if os.environ.get("GITHUB_EVENT_NAME") not in {"workflow_run", "workflow_dispatch"} or os.environ.get("GITHUB_REF") != "refs/heads/dev":
        raise policy.ToolError("GitHub tagging requires a dev validation completion or dispatch")
    event_sha = os.environ.get("GITHUB_SHA", "")
    if not event_sha:
        raise policy.ToolError("GitHub tagging requires the dev event revision")
    if sha != event_sha:
        policy.execute(directory, ["git", "merge-base", "--is-ancestor", sha, event_sha])
    workflow = f"{settings['repository']}/.github/workflows/{harmos_tools.release.source.RELEASE_WORKFLOW}@"
    if not os.environ.get("GITHUB_WORKFLOW_REF", "").startswith(workflow):
        raise policy.ToolError("GitHub tagging runs only in release.yml")
    merged = registry.release_review(settings, sha, registry.RELEASE_BRANCH + harmos_tools.release.source.release_tag(directory))
    if merged is None:
        raise policy.ToolError("the revision has no merged release review")
    review = registry.document(settings, f"pulls/{merged['number']}")
    if (
        not isinstance(review, dict)
        or review.get("merged") is not True
        or review.get("merge_commit_sha") != sha
        or type(review.get("number")) is not int
        or review.get("base", {}).get("ref") != "dev"
        or review.get("head", {}).get("ref") != registry.RELEASE_BRANCH + harmos_tools.release.source.release_tag(directory)
        or any(review.get(side, {}).get("repo", {}).get("full_name") != settings["repository"] for side in ("base", "head"))
    ):
        raise policy.ToolError("GitHub review is not the merged release review for this version and revision")
    return review


# === Private Functions === #

# === Entry Point === #
