# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Publish Existing Release Packages To The Mutable Nightly Prerelease."""

# === Imports === #

# Standard Library Imports
import hashlib
import json
import os
import re
import shutil
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

TAG = "nightly"
TITLE = "Nightly"
PROFILE = "release"
SUMMARY = (
    "Nightly release-profile build of the `dev` branch, replaced by each nightly run. "
    "Not a stable release: unsigned, published before the nightly suite finishes, and may be broken."
)
GITHUB_EVENTS = frozenset({"schedule", "workflow_dispatch"})
GITLAB_SOURCES = frozenset({"schedule", "web", "api"})
# GitHub Release Immutability Freezes Tags And Assets, Which A Mutable Prerelease Cannot Survive.
IMMUTABLE = (
    "the nightly release is immutable; the repository owner must disable "
    "Settings > General > Releases > Enable release immutability before nightly can publish"
)

# === Types === #

# === Classes === #

# === Public Functions === #


def publish(directory: Path) -> None:
    settings = policy.policy(directory, "release")
    if settings.get("nightly") is not True:
        raise policy.ToolError("release.nightly must opt in to nightly publication")
    github = settings["provider"] == "github"
    sha, identifier, url = _github_source() if github else _gitlab_source()
    matching = re.fullmatch(r"[0-9a-f]{40,64}", sha) and identifier.isdecimal() and run._revision(directory) == sha
    if not matching:
        raise policy.ToolError(f"{TAG} checkout must match the source CI commit")

    source_run = f"[{identifier}]({url})"
    endpoint = f"releases/tags/{TAG}" if github else f"releases/{TAG}"
    previous = _github_release(settings) if github else registry.document(settings, endpoint, channel=True)
    previous_notes = (previous or {}).get("body" if github else "description", "")
    with policy.credential_free(settings, profile=PROFILE):
        run._refuse_overrides(directory)
        content = _notes(directory, settings, sha, source_run, previous_notes)
        run.dry(directory, settings, profile=PROFILE, require_all=True, release_notes=content)
        _include_packages(directory, settings["targets"])

    files = layout.uploads(directory, staged=True)
    notes = (directory / run.ASSETS / run.SHARED / "CHANGELOG.md").read_text(encoding="utf-8")
    if github:
        _github(directory, settings, files, sha, identifier, notes)
    else:
        _gitlab(settings, files, sha, identifier, notes)


# === Private Functions === #


def _github_source() -> tuple[str, str, str]:
    """The Commit, Run Identifier And Run URL Of A Scheduled Or Dispatched Dev Run."""
    event_name = os.environ.get("GITHUB_EVENT_NAME")
    if event_name not in GITHUB_EVENTS:
        raise policy.ToolError(f"nightly cannot publish from a {event_name} event")
    if os.environ.get("GITHUB_REF") != "refs/heads/dev":
        raise policy.ToolError("nightly publishes only from dev")
    identifier = os.environ["GITHUB_RUN_ID"]
    server = os.environ.get("GITHUB_SERVER_URL", "https://github.com")
    return os.environ["GITHUB_SHA"], identifier, f"{server}/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{identifier}"


def _gitlab_source() -> tuple[str, str, str]:
    """The Commit, Pipeline Identifier And Pipeline URL Of An Eligible GitLab Source."""
    source = os.environ.get("PIPELINE_SOURCE") or os.environ.get("CI_PIPELINE_SOURCE")
    eligible = source in GITLAB_SOURCES and os.environ.get("CI_COMMIT_BRANCH") == "dev"
    if not eligible or os.environ.get("CI_DEFAULT_BRANCH") != "dev":
        raise policy.ToolError(f"GitLab nightly requires a {'/'.join(sorted(GITLAB_SOURCES))} pipeline on default dev")
    return os.environ["CI_COMMIT_SHA"], os.environ["CI_PIPELINE_ID"], os.environ["CI_PIPELINE_URL"]


def _include_packages(directory: Path, targets: list[str]) -> None:
    """Include Every Cataloged Distribution From Both Native Package Jobs."""
    root = directory / run.ASSETS
    files = [
        {**item, "path": Path(item["path"]).relative_to(run.ASSETS).as_posix()}
        for item in layout.uploads(directory, staged=True)
        if item["name"] != run.SUMS
    ]
    for target in targets:
        packaged = policy.inside(directory, f"target/package/{PROFILE}/{target}")
        catalog = json.loads((packaged / "catalog.json").read_text(encoding="utf-8"))
        if catalog["profile"] != PROFILE or not catalog["assets"]:
            raise policy.ToolError(f"nightly requires a complete {PROFILE} package catalog")
        for relative, expected in sorted(catalog["assets"].items()):
            source = run.packages.regular(packaged, relative)
            data = source.read_bytes()
            digest = hashlib.sha256(data).hexdigest()
            if digest != expected["sha256"] or len(data) != expected["bytes"]:
                raise policy.ToolError(f"{PROFILE} package differs from its catalog: {relative}")
            # Native Plugin Bundles Share Names; Qualify Them By Their Producer Target.
            name = f"{target}-{source.name}" if source.suffix == ".harmos" else source.name
            existing = next((item for item in files if item["name"].casefold() == name.casefold()), None)
            if existing:
                if existing["sha256"] != digest or existing["size"] != len(data):
                    raise policy.ToolError(f"nightly download names collide: {name}")
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


def _github_release(settings: dict) -> dict | None:
    release = registry.document(settings, f"releases/tags/{TAG}", channel=True)
    if release is not None:
        return release
    # The Tag Lookup Can Omit Drafts; Recover Interrupted Publication By Listing.
    drafts = [item for item in registry.pages(settings, "releases", channel=True) if item.get("tag_name") == TAG]
    if len(drafts) > 1:
        raise policy.ToolError(f"multiple {TAG} releases require owner reconciliation")
    return drafts[0] if drafts else None


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
        SUMMARY,
        "",
        "| | |",
        "|---|---|",
        f"| Commit | [`{sha[:8]}`]({commit}) {subject} |",
        f"| Version | {version} ({TAG}) |",
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
                f'version = "{TAG}"',
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
    lines.append("The tag and source archives identify this build; use the commit link for its permanent revision.")
    return "\n".join(lines) + "\n"


def _github(
    directory: Path,
    settings: dict,
    files: list[dict],
    sha: str,
    identifier: str,
    notes: str,
) -> None:
    release = _github_release(settings)
    if _superseded(release, identifier, "body"):
        print("nightly already carries a newer run")
        return
    if release and release.get("immutable"):
        raise policy.ToolError(IMMUTABLE)
    if release is None:
        release = registry.document(
            settings,
            "releases",
            "POST",
            {
                "tag_name": TAG,
                "target_commitish": sha,
                "name": TITLE,
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
        raise policy.ToolError(f"{TAG} upload inventory is incomplete; leaving it drafted")
    for asset in uploaded:
        expected_digest = next(item["sha256"] for item in files if item["name"] == asset["name"])
        digest = asset.get("digest")
        if digest is None:
            downloaded = registry.request(settings, f"releases/assets/{asset['id']}", binary=True, channel=True)
            if downloaded is None:
                raise policy.ToolError(f"{TAG} upload could not be verified; leaving it drafted")
            digest = "sha256:" + hashlib.sha256(downloaded).hexdigest()
        if digest != "sha256:" + expected_digest:
            raise policy.ToolError(f"{TAG} upload digest differs; leaving it drafted")

    # Move The Channel Once, Only After Every Platform And Checksum Is Uploaded.
    # Updating A Ref Across Workflow Changes Needs The Workflows Permission The Job Token Cannot Hold;
    # Recreating The Tag While The Release Is Still Drafted Does Not.
    reference = registry.document(settings, f"git/ref/tags/{TAG}", channel=True)
    if (reference or {}).get("object", {}).get("sha") != sha:
        if reference is not None:
            registry.document(settings, f"git/refs/tags/{TAG}", "DELETE", channel=True)
        registry.document(settings, "git/refs", "POST", {"ref": f"refs/tags/{TAG}", "sha": sha}, channel=True)
    registry.document(
        settings,
        endpoint,
        "PATCH",
        {
            "body": notes,
            "tag_name": TAG,
            "target_commitish": sha,
            "name": TITLE,
            "prerelease": True,
            "draft": False,
            "make_latest": "false",
        },
        channel=True,
    )

    published = registry.document(settings, f"releases/tags/{TAG}", channel=True)
    if published and published.get("immutable"):
        raise policy.ToolError(IMMUTABLE)
    if (
        not published
        or published.get("id") != release["id"]
        or published.get("tag_name") != TAG
        or published.get("target_commitish") != sha
        or published.get("draft") is not False
        or published.get("prerelease") is not True
    ):
        raise policy.ToolError(f"{TAG} publication could not be verified at releases/tags/{TAG}")
    published_assets = registry.pages(settings, f"releases/{published['id']}/assets", channel=True)
    expected = {item["name"]: item["size"] for item in files}
    actual = {asset["name"]: asset["size"] for asset in published_assets}
    if len(published_assets) != len(files) or actual != expected or any(asset.get("state") != "uploaded" for asset in published_assets):
        raise policy.ToolError(f"published {TAG} asset inventory differs from the staged downloads")


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
            {"tag_name": TAG, "ref": sha, "name": TITLE, "description": notes},
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
        {"name": TITLE, "description": notes},
        channel=True,
    )


# === Entry Point === #


if __name__ == "__main__":
    try:
        publish(policy.root())
    except (policy.ToolError, OSError, ValueError, KeyError, TypeError) as error:
        print(f"nightly: {error}", file=sys.stderr)
        raise SystemExit(1) from error
