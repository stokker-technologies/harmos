# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Publish Existing Dev Packages To The Fixed Dev Channel."""

# === Imports === #

# Standard Library Imports
import json
import os
import re
import sys
import tomllib
import urllib.parse
from datetime import UTC, datetime
from pathlib import Path

# Third Party Imports

# Local Imports
from . import layout, policy, registry, run

# Type Checking Imports


# === Public Exports === #

# === Constants === #

TAG = "dev-channel"

# === Types === #

# === Classes === #

# === Public Functions === #


def publish(directory: Path) -> None:
    settings = policy.policy(directory, "release")
    if settings.get("dev_channel") is not True:
        raise policy.ToolError("release.dev_channel must opt in to channel publication")
    github = settings["provider"] == "github"
    if github:
        if os.environ.get("GITHUB_EVENT_NAME") != "workflow_run":
            raise policy.ToolError("dev channel requires a successful dev CI completion")
        event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8"))["workflow_run"]
        eligible = event["conclusion"] == "success" and event["event"] == "push" and event["head_branch"] == "dev"
        if not eligible or event["head_repository"]["full_name"] != os.environ["GITHUB_REPOSITORY"]:
            raise policy.ToolError("ineligible dev channel CI run")
        sha, identifier, url = event["head_sha"], str(event["id"]), event["html_url"]
    else:
        source = os.environ.get("PIPELINE_SOURCE") or os.environ.get("CI_PIPELINE_SOURCE")
        eligible = source == "push" and os.environ.get("CI_COMMIT_BRANCH") == "dev"
        if not eligible or os.environ.get("CI_DEFAULT_BRANCH") != "dev":
            raise policy.ToolError("GitLab dev channel requires a push to default dev")
        sha, identifier, url = (
            os.environ["CI_COMMIT_SHA"],
            os.environ["CI_PIPELINE_ID"],
            os.environ["CI_PIPELINE_URL"],
        )
    if not re.fullmatch(r"[0-9a-f]{40,64}", sha) or not identifier.isdecimal() or run._revision(directory) != sha:
        raise policy.ToolError("dev channel checkout must match the source CI commit")
    source_run = f"[{identifier}]({url})"
    endpoint = f"releases/tags/{TAG}" if github else f"releases/{TAG}"
    previous = registry.document(settings, endpoint, channel=True)
    previous_notes = (previous or {}).get("body" if github else "description", "")
    with policy.credential_free(settings, profile="dev"):
        run._refuse_overrides(directory)
        content = _notes(directory, settings, sha, source_run, previous_notes)
        run.dry(directory, settings, profile="dev", require_all=True, release_notes=content)
    files = layout.uploads(directory, staged=True)
    notes = (directory / run.ASSETS / run.SHARED / "CHANGELOG.md").read_text(encoding="utf-8")
    if github:
        _github(directory, settings, files, sha, identifier, notes)
    else:
        _gitlab(settings, files, sha, identifier, notes)


# === Private Functions === #


def _superseded(release: dict | None, identifier: str, field: str) -> bool:
    previous = re.search(r"(?m)^\| CI Run \| \[([0-9]+)\]", (release or {}).get(field, ""))
    return previous is not None and int(previous[1]) > int(identifier)


def _notes(directory: Path, settings: dict, sha: str, source_run: str, previous: str) -> str:
    github = settings["provider"] == "github"
    repository = (settings.get("repository") or os.environ.get("GITHUB_REPOSITORY")) if github else settings["project_id"]
    base = "https://github.com" if github else settings.get("api_url", "https://gitlab.com/api/v4").removesuffix("/api/v4")
    project = f"{base}/{repository}"
    commit = f"{project}/commit/{sha}" if github else f"{project}/-/commit/{sha}"
    subject = policy.execute(directory, ["git", "log", "-1", "--format=%s", sha], capture=True).strip().replace("|", "\\|")
    version = policy.policy(directory, "version")["value"]
    lines = [
        "Rolling build of the `dev` branch, replaced automatically after merges. Not a release: unsigned, may change or break at any time.",
        "",
        "| | |",
        "|---|---|",
        f"| Commit | [`{sha[:8]}`]({commit}) {subject} |",
        f"| Version | {version} (dev) |",
    ]
    manifest = directory / "Cargo.toml"
    if manifest.is_file():
        cargo = tomllib.loads(manifest.read_text(encoding="utf-8"))
        dependencies = {
            **cargo.get("workspace", {}).get("dependencies", {}),
            **cargo.get("dependencies", {}),
        }
        for name, dependency in sorted(dependencies.items()):
            if isinstance(dependency, dict) and "git" in dependency:
                url = dependency["git"].removesuffix(".git")
                revision = dependency.get("rev")
                pin = (
                    f"[`{revision[:8]}`]({url}/commit/{revision})" if revision else dependency.get("tag", dependency.get("branch", "HEAD"))
                )
                lines.append(f"| {name.replace('-', ' ').title()} | {pin} (git) |")
    lines.extend(
        [
            f"| Built | {datetime.now(UTC):%Y-%m-%d %H:%M UTC} |",
            f"| CI Run | {source_run} |",
            "",
            "### Use It",
            "",
        ]
    )
    if github:
        lines.extend(
            [
                "Tools through mise:",
                "",
                "```toml",
                f'[tools."github:{repository}"]',
                'version = "dev-channel"',
                "```",
                "",
            ]
        )
    else:
        lines.extend(
            [
                f"Tools are available from the [channel downloads]({project}/-/releases/{TAG}).",
                "",
            ]
        )
    dependency = settings["product"] + ' = { git = "' + project + '", branch = "dev" }'
    lines.extend(["Libraries through Cargo:", "", "```toml", dependency, "```", ""])
    previous_commit = re.search(r"/commit/([0-9a-f]{40,64})\)", previous)
    if previous_commit and previous_commit[1] != sha:
        revision = previous_commit[1]
        try:
            present = policy.execute(
                directory,
                ["git", "rev-parse", "--verify", "--quiet", revision + "^{commit}"],
                capture=True,
            )
        except policy.ToolError:
            present = ""
        if present.strip() == revision:
            changes = policy.execute(
                directory,
                ["git", "log", "--format=- %s", "-20", f"{revision}..{sha}"],
                capture=True,
            )
            lines.extend(["### Changes Since The Previous Build", "", changes.strip(), ""])
    lines.append("The *Source code* archives below belong to the tag, not to this build; use the commit link instead.")
    return "\n".join(lines) + "\n"


def _github(
    directory: Path,
    settings: dict,
    files: list[dict],
    sha: str,
    identifier: str,
    notes: str,
) -> None:
    latest = registry.document(
        settings,
        "actions/workflows/ci.yml/runs?branch=dev&event=push&status=success&per_page=1",
        channel=True,
    )
    if latest and any(int(item["id"]) > int(identifier) for item in latest["workflow_runs"]):
        print("dev channel superseded by a newer successful CI run")
        return
    release = registry.document(settings, f"releases/tags/{TAG}", channel=True)
    if _superseded(release, identifier, "body"):
        return
    if release and release.get("immutable"):
        raise policy.ToolError("dev-channel assets are immutable; a rolling channel requires an existing mutable release")
    if registry.document(settings, f"git/ref/tags/{TAG}", channel=True) is None:
        registry.document(
            settings,
            "git/refs",
            "POST",
            {"ref": f"refs/tags/{TAG}", "sha": sha},
            channel=True,
        )
    if release is None:
        release = registry.document(
            settings,
            "releases",
            "POST",
            {
                "tag_name": TAG,
                "name": "Dev Channel",
                "prerelease": True,
                "draft": True,
                "make_latest": "false",
            },
            channel=True,
        )
    endpoint = f"releases/{release['id']}"
    # Remove Every Old Download, Including Assets No Longer Declared.
    assets = registry.pages(settings, f"{endpoint}/assets", channel=True)
    for asset in assets:
        registry.request(settings, f"releases/assets/{asset['id']}", "DELETE", channel=True)
    for item in files:
        path = policy.inside(directory, item["path"])
        name = urllib.parse.quote(item["name"], safe="")
        registry.request(
            settings,
            f"{endpoint}/assets?name={name}",
            "POST",
            path.read_bytes(),
            binary=True,
            upload=True,
            channel=True,
        )
    registry.document(
        settings,
        endpoint,
        "PATCH",
        {
            "body": notes,
            "name": "Dev Channel",
            "prerelease": True,
            "draft": False,
            "make_latest": "false",
        },
        channel=True,
    )


def _gitlab(settings: dict, files: list[dict], sha: str, identifier: str, notes: str) -> None:
    endpoint = f"releases/{TAG}"
    release = registry.document(settings, endpoint, channel=True)
    if _superseded(release, identifier, "description"):
        return
    if release is None:
        registry.document(
            settings,
            "releases",
            "POST",
            {"tag_name": TAG, "ref": sha, "name": "Dev Channel", "description": notes},
            channel=True,
        )
    # Links Address This Job's Unexpiring Staged Artifacts, Never A Moving Branch.
    links = registry.pages(settings, f"{endpoint}/assets/links", channel=True)
    for link in links:
        registry.document(settings, f"{endpoint}/assets/links/{link['id']}", "DELETE", channel=True)
    for item in files:
        path = urllib.parse.quote(item["path"], safe="/")
        url = os.environ["CI_JOB_URL"] + "/artifacts/raw/" + path
        registry.document(
            settings,
            f"{endpoint}/assets/links",
            "POST",
            {
                "name": item["name"],
                "url": url,
                "direct_asset_path": "/" + item["name"],
                "link_type": "package",
            },
            channel=True,
        )
    registry.document(
        settings,
        endpoint,
        "PUT",
        {"name": "Dev Channel", "description": notes},
        channel=True,
    )


# === Entry Point === #

if __name__ == "__main__":
    try:
        publish(policy.root())
    except (policy.ToolError, OSError, ValueError, KeyError, TypeError) as error:
        print(f"dev-channel: {error}", file=sys.stderr)
        raise SystemExit(1) from error
