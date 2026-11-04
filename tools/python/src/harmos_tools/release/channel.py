# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Publish Existing Packages To A Mutable Channel Prerelease."""

# === Imports === #

# Standard Library Imports
import hashlib
import json
import os
import re
import shutil
import tomllib
import urllib.parse
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path

# Third Party Imports
# Local Imports
from . import layout, policy, registry, run

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# GitHub Release Immutability Freezes Tags And Assets, Which A Mutable Channel Cannot Survive.
IMMUTABLE = (
    "the {tag} release is immutable; the repository owner must disable "
    "Settings > General > Releases > Enable release immutability before {tag} can publish"
)

# === Types === #

# === Classes === #


@dataclass(frozen=True)
class Channel:
    """One Mutable Prerelease: Its Tag, Title, Package Profile And Eligible Sources."""

    tag: str
    title: str
    profile: str
    summary: str
    github_events: frozenset[str]
    gitlab_sources: frozenset[str]
    # The Workflow Whose Newer Successful Dev Push Run Supersedes This Publication, If Any.
    superseding_workflow: str | None = None


EDGE = Channel(
    tag="edge",
    title="Edge",
    profile="dev",
    summary="Rolling build of the `dev` branch, replaced automatically after merges. Not a release: unsigned, may change or break at any time.",
    github_events=frozenset({"workflow_run"}),
    gitlab_sources=frozenset({"push"}),
    superseding_workflow="ci.yml",
)
NIGHTLY = Channel(
    tag="nightly",
    title="Nightly",
    profile="release",
    summary=(
        "Nightly release-profile build of the `dev` branch, replaced by each nightly run. "
        "Not a stable release: unsigned, published before the nightly suite finishes, and may be broken."
    ),
    github_events=frozenset({"schedule", "workflow_dispatch"}),
    gitlab_sources=frozenset({"schedule", "web", "api"}),
)
CHANNELS = {channel.tag: channel for channel in (EDGE, NIGHTLY)}

# === Public Functions === #


def publish(directory: Path, channel: Channel) -> None:
    settings = policy.policy(directory, "release")
    if settings.get(channel.tag) is not True:
        raise policy.ToolError(f"release.{channel.tag} must opt in to channel publication")
    github = settings["provider"] == "github"
    sha, identifier, url = _github_source(channel) if github else _gitlab_source(channel)
    matching = re.fullmatch(r"[0-9a-f]{40,64}", sha) and identifier.isdecimal() and run._revision(directory) == sha
    if not matching:
        raise policy.ToolError(f"{channel.tag} checkout must match the source CI commit")

    source_run = f"[{identifier}]({url})"
    endpoint = f"releases/tags/{channel.tag}" if github else f"releases/{channel.tag}"
    previous = _github_release(settings, channel) if github else registry.document(settings, endpoint, channel=True)
    previous_notes = (previous or {}).get("body" if github else "description", "")
    with policy.credential_free(settings, profile=channel.profile):
        run._refuse_overrides(directory)
        content = _notes(directory, settings, channel, sha, source_run, previous_notes)
        run.dry(directory, settings, profile=channel.profile, require_all=True, release_notes=content)
        _include_packages(directory, settings["targets"], channel.profile)

    files = layout.uploads(directory, staged=True)
    notes = (directory / run.ASSETS / run.SHARED / "CHANGELOG.md").read_text(encoding="utf-8")
    if github:
        _github(directory, settings, files, sha, identifier, notes, channel)
    else:
        _gitlab(settings, files, sha, identifier, notes, channel)


# === Private Functions === #


def _github_source(channel: Channel) -> tuple[str, str, str]:
    """The Commit, Run Identifier And Run URL Of An Eligible GitHub Source."""
    event_name = os.environ.get("GITHUB_EVENT_NAME")
    if event_name not in channel.github_events:
        raise policy.ToolError(f"{channel.tag} cannot publish from a {event_name} event")
    if event_name == "workflow_run":
        event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8"))["workflow_run"]
        eligible = event["conclusion"] == "success" and event["event"] == "push" and event["head_branch"] == "dev"
        if not eligible or event["head_repository"]["full_name"] != os.environ["GITHUB_REPOSITORY"]:
            raise policy.ToolError(f"ineligible {channel.tag} CI run")
        return event["head_sha"], str(event["id"]), event["html_url"]
    if os.environ.get("GITHUB_REF") != "refs/heads/dev":
        raise policy.ToolError(f"{channel.tag} publishes only from dev")
    identifier = os.environ["GITHUB_RUN_ID"]
    server = os.environ.get("GITHUB_SERVER_URL", "https://github.com")
    return os.environ["GITHUB_SHA"], identifier, f"{server}/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{identifier}"


def _gitlab_source(channel: Channel) -> tuple[str, str, str]:
    """The Commit, Pipeline Identifier And Pipeline URL Of An Eligible GitLab Source."""
    source = os.environ.get("PIPELINE_SOURCE") or os.environ.get("CI_PIPELINE_SOURCE")
    eligible = source in channel.gitlab_sources and os.environ.get("CI_COMMIT_BRANCH") == "dev"
    if not eligible or os.environ.get("CI_DEFAULT_BRANCH") != "dev":
        raise policy.ToolError(f"GitLab {channel.tag} requires a {'/'.join(sorted(channel.gitlab_sources))} pipeline on default dev")
    return os.environ["CI_COMMIT_SHA"], os.environ["CI_PIPELINE_ID"], os.environ["CI_PIPELINE_URL"]


def _include_packages(directory: Path, targets: list[str], profile: str) -> None:
    """Include Every Cataloged Distribution From Both Native Package Jobs."""
    root = directory / run.ASSETS
    files = [
        {**item, "path": Path(item["path"]).relative_to(run.ASSETS).as_posix()}
        for item in layout.uploads(directory, staged=True)
        if item["name"] != run.SUMS
    ]
    for target in targets:
        packaged = policy.inside(directory, f"target/package/{profile}/{target}")
        catalog = json.loads((packaged / "catalog.json").read_text(encoding="utf-8"))
        if catalog["profile"] != profile or not catalog["assets"]:
            raise policy.ToolError(f"channel publication requires a complete {profile} package catalog")
        for relative, expected in sorted(catalog["assets"].items()):
            source = run.packages.regular(packaged, relative)
            data = source.read_bytes()
            digest = hashlib.sha256(data).hexdigest()
            if digest != expected["sha256"] or len(data) != expected["bytes"]:
                raise policy.ToolError(f"{profile} package differs from its catalog: {relative}")
            # Native Plugin Bundles Share Names; Qualify Them By Their Producer Target.
            name = f"{target}-{source.name}" if source.suffix == ".harmos" else source.name
            existing = next((item for item in files if item["name"].casefold() == name.casefold()), None)
            if existing:
                if existing["sha256"] != digest or existing["size"] != len(data):
                    raise policy.ToolError(f"channel download names collide: {name}")
                continue
            destination = policy.inside(root, f"{target}/{name}")
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)
            files.append(run._describe(root, destination))
    sums = root / run.SHARED / run.SUMS
    sums.write_text("".join(f"{item['sha256']}  {item['name']}\n" for item in files), encoding="utf-8")
    files.append(run._describe(root, sums))
    layout.upload_manifest(directory, root, files)
    layout.uploads(directory, staged=True)


def _github_release(settings: dict, channel: Channel) -> dict | None:
    release = registry.document(settings, f"releases/tags/{channel.tag}", channel=True)
    if release is not None:
        return release
    # The Tag Lookup Can Omit Drafts; Recover Interrupted Publication By Listing.
    drafts = [item for item in registry.pages(settings, "releases", channel=True) if item.get("tag_name") == channel.tag]
    if len(drafts) > 1:
        raise policy.ToolError(f"multiple {channel.tag} releases require owner reconciliation")
    return drafts[0] if drafts else None


def _superseded(release: dict | None, identifier: str, field: str) -> bool:
    previous = re.search(r"(?m)^\| CI Run \| \[([0-9]+)\]", (release or {}).get(field, ""))
    return previous is not None and int(previous[1]) > int(identifier)


def _notes(directory: Path, settings: dict, channel: Channel, sha: str, source_run: str, previous: str) -> str:
    github = settings["provider"] == "github"
    repository = (settings.get("repository") or os.environ.get("GITHUB_REPOSITORY")) if github else settings["project_id"]
    base = "https://github.com" if github else settings.get("api_url", "https://gitlab.com/api/v4").removesuffix("/api/v4")
    project = f"{base}/{repository}"
    commit = f"{project}/commit/{sha}" if github else f"{project}/-/commit/{sha}"
    subject = policy.execute(directory, ["git", "log", "-1", "--format=%s", sha], capture=True).strip().replace("|", "\\|")
    version = policy.policy(directory, "version")["value"]
    lines = [
        channel.summary,
        "",
        "| | |",
        "|---|---|",
        f"| Commit | [`{sha[:8]}`]({commit}) {subject} |",
        f"| Version | {version} ({channel.tag}) |",
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
                f'version = "{channel.tag}"',
                "```",
                "",
            ]
        )
    else:
        lines.extend(
            [
                f"Tools are available from the [channel downloads]({project}/-/releases/{channel.tag}).",
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
    lines.append("The tag and source archives identify this build; use the commit link for its permanent revision.")
    return "\n".join(lines) + "\n"


def _github(
    directory: Path,
    settings: dict,
    files: list[dict],
    sha: str,
    identifier: str,
    notes: str,
    channel: Channel = EDGE,
) -> None:
    tag = channel.tag
    if channel.superseding_workflow:
        latest = registry.document(
            settings,
            f"actions/workflows/{channel.superseding_workflow}/runs?branch=dev&event=push&status=success&per_page=1",
            channel=True,
        )
        if latest and any(int(item["id"]) > int(identifier) for item in latest["workflow_runs"]):
            print(f"{tag} superseded by a newer successful CI run")
            return
    release = _github_release(settings, channel)
    if _superseded(release, identifier, "body"):
        print(f"{tag} already carries a newer run")
        return
    if release and release.get("immutable"):
        raise policy.ToolError(IMMUTABLE.format(tag=tag))
    if release is None:
        release = registry.document(
            settings,
            "releases",
            "POST",
            {
                "tag_name": tag,
                "target_commitish": sha,
                "name": channel.title,
                "prerelease": True,
                "draft": True,
                "make_latest": "false",
            },
            channel=True,
        )
    endpoint = f"releases/{release['id']}"
    # Hide The Channel Before Replacing Any Download; Failed Uploads Leave A Draft.
    registry.document(settings, endpoint, "PATCH", {"draft": True}, channel=True)
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
    uploaded = registry.pages(settings, f"{endpoint}/assets", channel=True)
    actual = {asset["name"]: asset["size"] for asset in uploaded}
    expected = {item["name"]: item["size"] for item in files}
    if (
        len(uploaded) != len(files)
        or actual != expected
        or "SHA256SUMS" not in actual
        or any(asset.get("state") != "uploaded" for asset in uploaded)
    ):
        raise policy.ToolError(f"{tag} upload inventory is incomplete; leaving it drafted")
    for asset in uploaded:
        expected_digest = next(item["sha256"] for item in files if item["name"] == asset["name"])
        digest = asset.get("digest")
        if digest is None:
            downloaded = registry.request(settings, f"releases/assets/{asset['id']}", binary=True, channel=True)
            if downloaded is None:
                raise policy.ToolError(f"{tag} upload could not be verified; leaving it drafted")
            digest = "sha256:" + hashlib.sha256(downloaded).hexdigest()
        if digest != "sha256:" + expected_digest:
            raise policy.ToolError(f"{tag} upload digest differs; leaving it drafted")

    # Move The Channel Once, Only After Every Platform And Checksum Is Uploaded.
    reference = registry.document(settings, f"git/ref/tags/{tag}", channel=True)
    if reference is None:
        registry.document(settings, "git/refs", "POST", {"ref": f"refs/tags/{tag}", "sha": sha}, channel=True)
    else:
        registry.document(settings, f"git/refs/tags/{tag}", "PATCH", {"sha": sha, "force": True}, channel=True)
    registry.document(
        settings,
        endpoint,
        "PATCH",
        {
            "body": notes,
            "tag_name": tag,
            "target_commitish": sha,
            "name": channel.title,
            "prerelease": True,
            "draft": False,
            "make_latest": "false",
        },
        channel=True,
    )

    published = registry.document(settings, f"releases/tags/{tag}", channel=True)
    if published and published.get("immutable"):
        raise policy.ToolError(IMMUTABLE.format(tag=tag))
    if (
        not published
        or published.get("id") != release["id"]
        or published.get("tag_name") != tag
        or published.get("target_commitish") != sha
        or published.get("draft") is not False
        or published.get("prerelease") is not True
    ):
        raise policy.ToolError(f"{tag} publication could not be verified at releases/tags/{tag}")
    published_assets = registry.pages(settings, f"releases/{published['id']}/assets", channel=True)
    expected = {item["name"]: item["size"] for item in files}
    actual = {asset["name"]: asset["size"] for asset in published_assets}
    if len(published_assets) != len(files) or actual != expected or any(asset.get("state") != "uploaded" for asset in published_assets):
        raise policy.ToolError(f"published {tag} asset inventory differs from the staged downloads")


def _gitlab(settings: dict, files: list[dict], sha: str, identifier: str, notes: str, channel: Channel) -> None:
    endpoint = f"releases/{channel.tag}"
    release = registry.document(settings, endpoint, channel=True)
    if _superseded(release, identifier, "description"):
        return
    if release is None:
        registry.document(
            settings,
            "releases",
            "POST",
            {"tag_name": channel.tag, "ref": sha, "name": channel.title, "description": notes},
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
        {"name": channel.title, "description": notes},
        channel=True,
    )


# === Entry Point === #


def _publish_named(directory: Path, tag: str) -> None:
    publish(directory, CHANNELS[tag])


if __name__ == "__main__":
    raise SystemExit(policy.main("channel", sorted(CHANNELS), _publish_named))
