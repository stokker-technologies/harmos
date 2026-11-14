# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Prepare, Rehearse, Tag And Publish Releases From The Conventional Commit History"""

# === Imports === #

# Standard Library Imports
import argparse
import base64
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tomllib
import urllib.parse
import zipfile
from datetime import UTC, datetime
from functools import cache
from pathlib import Path

# Third Party Imports
# Local Imports
from ..process import spawn_flags

# isort: split
from .. import temporary
from ..package import run as packages
from ..package.changelog import notes
from ..test import archive
from . import layout, pins, policy, registry, reports, screenshots, version

# Type Checking Imports

# === Public Exports === #

# === Constants === #

STABLE = r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
SUMS = "SHA256SUMS"
STATE = "published.json"
# Staged downloads live under <target>/; SHA256SUMS and the publication state span targets and live under any/.
ASSETS = "target/release-assets"
SHARED = "any"
# Registry Publication Stays In The Protected Native Release Workflow.
RELEASE_WORKFLOW = "release.yml"
VERBS = [
    "prepare",
    "review",
    "dry",
    "crates-dry",
    "rehearse",
    "identity",
    "tag",
    "crates",
    "publish",
    "python",
    "finalize",
    "verify",
    "guard",
    "assets",
    "stage",
]

# === Types === #

# === Classes === #

# === Public Functions === #


def continuous_integration() -> bool:
    """Whether A CI Runner Executes This Process."""
    return any(os.environ.get(name, "").lower() in {"1", "true"} for name in ("CI", "GITHUB_ACTIONS", "GITLAB_CI"))


def release_tag(directory: Path) -> str:
    """The Stable Tag Of The Prepared Version: `v` Plus `[version] value`."""
    value = policy.policy(directory, "version").get("value")
    if not isinstance(value, str) or re.fullmatch(STABLE, value) is None:
        raise policy.ToolError("version.value must declare a stable MAJOR.MINOR.PATCH version")
    return "v" + value


def native_target(settings: dict) -> str:
    system = {"Linux": "linux", "Windows": "windows", "Darwin": "macos"}.get(platform.system())
    architecture = {"amd64": "x86_64", "arm64": "aarch64"}.get(platform.machine().lower(), platform.machine().lower())
    selected = f"{system}-{architecture}"
    if selected not in settings.get("targets", []):
        raise policy.ToolError(f"native target {selected} is not one of release.targets")
    return selected


def assets(directory: Path, settings: dict, selected: str, tag: str, profile: str | None = None) -> list[dict]:
    """Resolve The Platform's Reference Host Archive From Its Existing Package."""
    profile = policy.profile() if profile is None else profile
    name = layout.conformance_name(directory, settings, selected, tag[1:])
    relative = f"target/package/{profile}/{selected}/rust/{name}"
    packages.regular(directory, relative)
    return [{"name": name, "path": relative, "target": selected}]


def stage(
    directory: Path,
    settings: dict,
    tag: str,
    targets: list[str],
    profile: str | None = None,
    *,
    source_revision: str | None = None,
    release_notes: str | None = None,
    channel: str = "stable",
) -> list[dict]:
    """Replace Release Staging With The Exact Channel Inventory From Existing Packages."""
    if channel not in {"stable", "nightly"}:
        raise policy.ToolError("release channel must be stable or nightly")
    if not targets:
        raise policy.ToolError("release staging requires at least one packaged target")
    profile = policy.profile() if profile is None else profile
    root = _root(directory)
    if root.is_symlink() or root.exists() and not root.is_dir():
        raise policy.ToolError(f"release staging requires a regular directory: {root}")
    # Validate Directory Shapes Before Removing Any Owned Staging.
    for owned in [*targets, SHARED]:
        destination = policy.inside(root, owned)
        if destination == root or destination.is_symlink() or destination.exists() and not destination.is_dir():
            raise policy.ToolError(f"release staging requires a regular directory: {destination}")
    for owned in [*targets, SHARED]:
        destination = policy.inside(root, owned)
        if destination.exists():
            shutil.rmtree(destination)
        destination.mkdir(parents=True)
    shared = root / SHARED
    files = []
    for selected in targets:
        for asset in assets(directory, settings, selected, tag, profile):
            destination = root / selected / asset["name"]
            shutil.copyfile(packages.regular(directory, asset["path"]), destination)
            files.append(_describe(root, destination))
    content = notes(directory, tag) if release_notes is None else release_notes
    if source_revision is not None:
        content += f"\nSource Commit: {source_revision}\n"
    protocol = shared / f"harmos-proto-{tag[1:]}.zip"
    source = directory / "proto"
    definitions = sorted(source.rglob("*.proto"))
    if not definitions:
        raise policy.ToolError("protocol archive requires definitions")
    behaviour = packages.regular(directory, "proto/harmos/plugin/v1/behaviour.md")
    with zipfile.ZipFile(protocol, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for path in [*definitions, behaviour]:
            original = packages.regular(source, path.relative_to(source).as_posix())
            archive.write(original, original.relative_to(source).as_posix())
    files.append(_describe(root, protocol))
    compliance = shared / f"harmos-compliance-{tag[1:]}.zip"
    with zipfile.ZipFile(compliance, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for selected in targets:
            source = directory / "target/package" / profile / selected / "compliance"
            if not source.is_dir() or source.is_symlink():
                raise policy.ToolError(f"missing regular compliance directory: {source}")
            abi = layout.triple(directory, settings, selected)
            for name in ("NOTICE", "THIRD-PARTY-NOTICES", f"crates/{abi}/workspace.cdx.json", "python/any/sdk.cdx.json"):
                original = packages.regular(source, name)
                # Preserve Each Target's Notices; Python's Locked Inventory Is Shared.
                if name.startswith("python/") and selected != targets[0]:
                    first = directory / "target/package" / profile / targets[0] / "compliance" / name
                    if original.read_bytes() != first.read_bytes():
                        raise policy.ToolError(f"Stage Python Compliance: inventories differ between {targets[0]} and {selected}: {name}")
                    continue
                member = f"notices/{selected}/{name}" if "/" not in name else name
                archive.write(original, member)
    files.append(_describe(root, compliance))
    if channel == "nightly":
        for destination in reports.stage(directory, shared, tag[1:]):
            files.append(_describe(root, destination))
    manifest = layout.downloads(
        settings,
        version=tag[1:],
        channel=channel,
        tag=tag if channel == "stable" else channel,
        published=datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
        files=files,
        sums="".join(f"{item['sha256']}  {item['name']}\n" for item in files),
    )
    policy.write_json(shared / "release.json", {"notes": content, "pub_date": manifest["pub_date"], "downloads": manifest})
    files.append(_describe(root, shared / "release.json"))
    (shared / SUMS).write_text("".join(f"{item['sha256']}  {item['name']}\n" for item in files), encoding="utf-8")
    files.append(_describe(root, shared / SUMS))
    layout.upload_manifest(directory, root, files)
    return files


def planned_version(directory: Path, current: str) -> str:
    """The Current Version Before Its First Tag, Else git-cliff's Configured Automatic Version."""
    if not policy.execute(directory, ["git", "tag", "--list", "v" + current], capture=True).strip():
        return current
    # Breaking changes bump the minor and everything else the patch while the version is 0.x, as tools/config/cliff.toml [bump] declares.
    command = ["git-cliff", "--config", "tools/config/cliff.toml", "--bumped-version"]
    planned = policy.execute(directory, command, capture=True).strip().removeprefix("v")
    if planned == current:
        since = policy.execute(directory, ["git", "log", "--format=%h %s", f"v{current}..HEAD"], capture=True).strip()
        listing = "\n".join("  " + line for line in since.splitlines()) or "  (no commits)"
        raise policy.ToolError(f"v{current} is released and no Conventional Commit since warrants a new version; commits since:\n{listing}")
    return planned


def prepare(directory: Path, settings: dict) -> None:
    """Bump Every Version Carrier And Regenerate Release Inputs In The Working Tree."""
    if policy.execute(directory, ["git", "status", "--porcelain"], capture=True).strip():
        raise policy.ToolError("prepare requires clean committed source")
    _require_publication(directory, settings)
    current = release_tag(directory)[1:]
    planned = planned_version(directory, current)
    if re.fullmatch(STABLE, planned) is None:
        raise policy.ToolError(f"the next version is not a stable version: {planned}")
    version.run(directory, "sync", value=planned)
    _policy_version(directory, planned)
    commands = settings.get("lock_commands")
    if commands is None:
        commands = []
        if (directory / "Cargo.toml").is_file():
            # Cold runners start with an empty Cargo home, so the workspace-only update may fetch the index.
            commands.append(["cargo", "update", "--workspace"])
        for lock in _tracked(directory, "*package-lock.json"):
            commands.append(["npm", "install", "--package-lock-only", "--ignore-scripts", "--prefix", str(directory / Path(lock).parent)])
        for lock in _tracked(directory, "*pnpm-lock.yaml"):
            commands.append(["pnpm", "install", "--lockfile-only", "--ignore-scripts", "--dir", str(directory / Path(lock).parent)])
        for lock in _tracked(directory, "*uv.lock"):
            commands.append(["uv", "lock", "--offline", "--project", str(directory / Path(lock).parent)])
    for command in commands:
        policy.execute(directory, policy.command(command, "release.lock_commands"))
    if policy.policy(directory, "screenshots").get("scenes"):
        screenshots.capture(directory)
    print(f"Prepared v{planned}; review the changes, or let Release / Prepare open the release review")


def review(directory: Path, settings: dict) -> None:
    """Commit The Prepared Tree To release/v<version> And Open Its Review Into dev With The Release Token."""
    token = os.environ.get("RELEASE_TOKEN", "")
    if not continuous_integration() or not token:
        raise policy.ToolError("the release review opens from Release / Prepare with the RELEASE_TOKEN secret")
    tag = release_tag(directory)
    version.run(directory, "check")
    branch = registry.RELEASE_BRANCH + tag
    if settings["provider"] == "github":
        os.environ["GH_TOKEN"] = token
    endpoint = "git/ref/heads/" if settings["provider"] == "github" else "repository/branches/"
    if registry.document(settings, endpoint + _quote(branch)):
        raise policy.ToolError(f"{branch} already exists; merge or close its review before preparing again")
    title = f"chore(release): prepare {tag}"
    policy.execute(directory, ["git", "add", "--all"])
    _commit_review(directory, settings, branch, title)
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


def dry(
    directory: Path,
    settings: dict,
    *,
    profile: str | None = None,
    require_all: bool = False,
    source_revision: str | None = None,
    release_notes: str | None = None,
    targets: list[str] | None = None,
    channel: str = "stable",
) -> None:
    """Stage The Active Profile's Packages Like A Release And Run The Native Downloads From Them, Offline."""
    tag = release_tag(directory)
    version.run(directory, "check")
    # Release reviews package the release profile; every other run packages dev.
    profile = policy.profile("dev") if profile is None else profile
    root = _root(directory)
    targets = list(settings.get("targets", [])) if targets is None else targets
    present = [selected for selected in targets if (directory / "target/package" / profile / selected).is_dir()]
    missing = sorted(set(targets) - set(present))
    if missing and (require_all or continuous_integration()):
        raise policy.ToolError(f"{profile} packages are missing for release targets: {', '.join(missing)}")
    if missing:
        print(f"note: no local {profile} packages for {', '.join(missing)}; CI stages every release target")
    stage(directory, settings, tag, present, profile=profile, source_revision=source_revision, release_notes=release_notes, channel=channel)
    files = layout.uploads(directory, staged=True)
    _checksums(root / SHARED / SUMS, {item["name"]: item for item in files})
    native = _native()
    if native in present:
        # The staged bytes themselves run, from an isolated directory, exactly as a user's download would.
        verify_conformance(directory, settings, profile, native, root)
    print(f"staged {len(files) - 1} download(s) and {SUMS} for {', '.join(present) or 'no target'} in {root.relative_to(directory)}")


def verify_conformance(directory: Path, settings: dict, profile: str, target: str, root: Path) -> None:
    """Run The Downloaded Reference Host With Its Sibling Reference Plugin."""
    name = layout.conformance_name(directory, settings, target, release_tag(directory)[1:])
    source = root / target / name
    if not source.is_file():
        source = root / name
    suffix = ".exe" if "-windows-" in layout.triple(directory, settings, target) else ""
    with temporary.directory() as install:
        for binary in ("harmos-conformance", "harmos-conformance-plugin"):
            destination = install / (binary + suffix)
            destination.write_bytes(packages.binary_bytes(source, destination.name))
            destination.chmod(0o755)
        policy.execute(install, [str(install / ("harmos-conformance" + suffix)), "--help"])


def publish(directory: Path, settings: dict) -> None:
    """Stage And Publish Exactly The Manifest's Downloads From The Protected Release Tag."""
    tag = release_tag(directory)
    sha = _revision(directory)
    version.run(directory, "check")
    registry.require_tag(directory, tag)
    pins.check(directory)
    _refuse_overrides(directory)
    targets = list(settings.get("targets", []))
    if not targets:
        raise policy.ToolError("release.targets must declare the required OS/architecture targets")
    root = _root(directory)
    stage(directory, settings, tag, targets)
    files = layout.uploads(directory, staged=True)
    state = {"sha": sha, "tag": tag, "name": tag, "files": {}}
    if settings["provider"] == "github":
        release = registry.draft(settings, tag, sha, notes(directory, tag, tagged=True))
        state["release_id"] = (release or {})["id"]
        for item in files:
            data = _bytes(directory, item)
            state["files"][item["name"]] = {**item, "endpoint": registry.upload(settings, (release or {})["id"], item, data)}
            policy.write_json(root / SHARED / STATE, state)
    else:
        if registry.document(settings, "releases/" + _quote(tag)):
            raise policy.ToolError("published release cannot be replaced; use a new patch version")
        registry.verify_duplicate_policy(settings)
        for item in files:
            state["files"][item["name"]] = {
                **item,
                **registry.upload_generic(settings, _package(settings), tag, item, _bytes(directory, item)),
            }
            policy.write_json(root / SHARED / STATE, state)
    policy.write_json(root / SHARED / STATE, state)


def finalize(directory: Path, settings: dict) -> None:
    """Verify Every Manifest Download And Signature Before Making The Release Visible."""
    root = _root(directory)
    state = json.loads((root / SHARED / STATE).read_text(encoding="utf-8"))
    if state.get("sha") != _revision(directory) or state.get("tag") != release_tag(directory):
        raise policy.ToolError("the staged publication belongs to another revision or version")
    inventory = {item["name"]: item for item in layout.uploads(directory, staged=True)}
    if set(inventory) != set(state["files"]) or any(
        any(entry.get(key) != value for key, value in inventory[name].items()) for name, entry in state["files"].items()
    ):
        raise policy.ToolError("publication state differs from upload.json")
    with temporary.directory() as downloads:
        for item in state["files"].values():
            (downloads / item["name"]).write_bytes(registry.verify_download(settings, item["endpoint"], item))
        registry.verify_signatures(directory, settings, downloads, list(inventory))
    if registry.remote_tag(settings, state["tag"]) != state["sha"]:
        raise policy.ToolError("the release tag is absent or identifies another revision")
    if settings["provider"] == "github":
        current = registry.document(settings, f"releases/{state['release_id']}")
        if not isinstance(current, dict) or not current.get("draft") or current.get("tag_name") != state["tag"]:
            raise policy.ToolError("the release is no longer the staged draft; verify it manually")
        listed = {item["name"] for item in registry.pages(settings, f"releases/{state['release_id']}/assets")}
        if listed != set(inventory):
            raise policy.ToolError("draft assets differ from upload.json")
        registry.document(settings, f"releases/{state['release_id']}", "PATCH", {"draft": False})
        visible = registry.document(settings, "releases/tags/" + _quote(state["tag"]))
        if not isinstance(visible, dict) or visible.get("draft") or visible.get("prerelease"):
            raise policy.ToolError("published release visibility cannot be verified")
        return
    created = registry.document(
        settings,
        "releases",
        "POST",
        {
            "tag_name": state["tag"],
            "ref": state["sha"],
            "name": state["tag"],
            "description": notes(directory, state["tag"], tagged=True),
            "assets": {"links": [item["link"] for item in state["files"].values()]},
        },
    )
    if not isinstance(created, dict) or created.get("tag_name") != state["tag"] or (created.get("commit") or {}).get("id") != state["sha"]:
        raise policy.ToolError("published release tag identity cannot be verified")
    remote = registry.document(settings, "releases/" + _quote(state["tag"])) or {}
    if {link.get("name") for link in (remote.get("assets") or {}).get("links", [])} != set(inventory):
        raise policy.ToolError("release links differ from upload.json")


def verify(directory: Path, settings: dict) -> None:
    """After Publication: Download Every Published Asset, Match SHA256SUMS And Signatures, Install On This Platform."""
    tag = release_tag(directory)
    sha = _revision(directory)
    if registry.remote_tag(settings, tag) != sha:
        raise policy.ToolError(f"{tag} does not identify the checked-out revision")
    inventory = {item["name"]: item for item in layout.uploads(directory)}
    quoted = _quote(tag)
    if settings["provider"] == "github":
        remote = registry.document(settings, "releases/tags/" + quoted)
        if not isinstance(remote, dict) or remote.get("draft") or remote.get("prerelease"):
            raise policy.ToolError("release is not published")
        downloads = {item["name"]: f"releases/assets/{item['id']}" for item in registry.pages(settings, f"releases/{remote['id']}/assets")}
    else:
        remote = registry.document(settings, "releases/" + quoted)
        if not isinstance(remote, dict):
            raise policy.ToolError("release is not published")
        prefix = f"packages/generic/{_quote(_package(settings))}/{quoted}/"
        downloads = {item["name"]: prefix + _quote(item["name"]) for item in remote["assets"]["links"]}
    if set(downloads) != set(inventory):
        raise policy.ToolError("published downloads differ from upload.json")
    with temporary.directory() as root:
        for name, item in inventory.items():
            data = registry.verify_download(settings, downloads[name], item)
            policy.inside(root, name).write_bytes(data)
        _checksums(root / SUMS, inventory)
        registry.verify_signatures(directory, settings, root, list(inventory))
        native = native_target(settings)
        verify_conformance(directory, settings, "release", native, root)
    policy.write_json(directory / "target/state/release-publication.json", {"sha": sha, "tag": tag, "verified": True})


def crates(directory: Path, settings: dict) -> None:
    """Publish Crates From The Protected Tag Job."""
    registry.publish_crates(directory)


def tag(directory: Path, settings: dict) -> None:
    """Tag An Exactly Rehearsed Merged Release Review, Then Fast-Forward main."""
    sha = _revision(directory)
    event = _tag_event(directory, settings, sha)
    pins.check(directory)
    review = registry.release_review(settings, sha, registry.RELEASE_BRANCH + release_tag(directory))
    if review is None:
        raise policy.ToolError("the revision has no merged release review")
    if event is not None and (review["number"] != event["number"] or review["head"] != event["head"]["sha"]):
        raise policy.ToolError("the merged release review differs from the GitHub event")
    name = release_tag(directory)
    existing = registry.remote_tag(settings, name)
    if existing and existing != sha:
        raise policy.ToolError(f"{name} already identifies another revision; never move a release tag")
    # Only products that render the rehearsal can have rehearsed their review; for them no tag exists without it.
    if _task_defined(directory, "release:rehearse"):
        registry.rehearsed(directory, settings, review, sha)
    else:
        registry._same_tree(directory, review["head"], sha)
        print("note: this product renders no rehearsal; Release / Dry verified the review")
    token = os.environ.get("RELEASE_TOKEN", "")
    if not token:
        raise policy.ToolError("tagging requires the workflow-triggering RELEASE_TOKEN secret")
    # Reject known divergence before any immutable tag or registry publication can occur.
    remote = _push_remote(settings, token)
    _run(directory, ["git", "fetch", "--quiet", remote, "refs/heads/main"], token)
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
    if _task_defined(directory, "release:publish:rust"):
        policy.execute(directory, ["mise", "run", "release:publish:rust"])
    # main is the last released commit: a fast-forward to the tag, never a merge or a force.
    _run(directory, ["git", "push", remote, f"{sha}:refs/heads/main"], token)
    print(f"{name}: tagged at {sha}; main fast-forwarded")
    branch = registry.RELEASE_BRANCH + name
    if registry.delete_branch(settings, branch):
        print(f"{branch}: deleted after its squash")


def rehearse(directory: Path, settings: dict, *, target: str | None = None, profile: str = "release", stage_only: bool = False) -> None:
    """Verify Selected Local Packages Or Every Hosted Target Without Publishing Credentials."""
    hosted = continuous_integration()
    if profile not in {"dev", "release"}:
        raise policy.ToolError("rehearsal profile must be dev or release")
    if hosted and (profile != "release" or stage_only):
        raise policy.ToolError("hosted rehearsal requires complete release-profile verification")
    if hosted and target not in (None, "all"):
        raise policy.ToolError("hosted rehearsal requires all declared release targets")
    target = target or ("all" if hosted else "native")
    selected = native_target(settings) if target == "native" else target
    if selected != "all" and selected not in settings.get("targets", []):
        raise policy.ToolError(f"undeclared release target: {selected}")
    targets = list(settings.get("targets", [])) if selected == "all" else [selected]
    with policy.credential_free(settings, profile=profile):
        _refuse_overrides(directory)
        revision = _revision(directory) if stage_only else None
        dry(directory, settings, profile=profile, require_all=True, source_revision=revision, targets=targets)
        if not stage_only and _task_defined(directory, "release:publish:rust"):
            registry.rehearse_crates(directory, skip_published=False)
        if not stage_only and _task_defined(directory, "release:publish:python"):
            registry.python(directory, settings, dry_run=True)
    print(f"release {'staging' if stage_only else 'rehearsal'} passed using staged {profile}-profile packages")


def identity(directory: Path, settings: dict) -> None:
    """Print The OIDC Identity Registries See And The Trusted Publisher Values That Match It."""
    url = os.environ.get("ACTIONS_ID_TOKEN_REQUEST_URL")
    request = os.environ.get("ACTIONS_ID_TOKEN_REQUEST_TOKEN")
    if not url or not request:
        print("note: no GitHub OIDC token is available to this job; grant id-token: write to diagnose Trusted Publishing")
        return
    claims = registry.oidc_claims(url, request)
    repository = claims.get("repository", "")
    environment = claims.get("environment") or "(none)"
    caller = registry.workflow_filename(claims.get("workflow_ref", ""))
    called = registry.workflow_filename(claims.get("job_workflow_ref") or claims.get("workflow_ref", ""))
    # Both registries execute in release.yml; PyPI uses job_workflow_ref when present.
    print(
        f"crates.io Trusted Publisher: repository {repository}, workflow {caller}, environment {environment} (workflow_ref {claims.get('workflow_ref')})"
    )
    print(
        f"PyPI Trusted Publisher: repository {repository}, workflow {called}, environment {environment} (job_workflow_ref {claims.get('job_workflow_ref')})"
    )
    for registry_name, actual, expected in [("crates.io", caller, RELEASE_WORKFLOW), ("PyPI", called, RELEASE_WORKFLOW)]:
        if actual != expected:
            print(f"warning: {registry_name} sees workflow {actual}, but the rendered guidance names {expected}")


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("verb", choices=VERBS)
    parser.add_argument("--target", help="Rehearse native, one declared release target, or all targets; hosted runs require all.")
    parser.add_argument("--profile", choices=("dev", "release"), help="Local rehearsal package profile; defaults to release.")
    parser.add_argument("--stage-only", action="store_true", help="Local rehearsal staging without registry dry-runs.")
    options = parser.parse_args(arguments)
    try:
        run(policy.root(), options.verb, target=options.target, profile=options.profile, stage_only=options.stage_only)
    except (policy.ToolError, OSError, ValueError, KeyError, TypeError) as error:
        print(f"release: {error}", file=sys.stderr)
        return 1
    return 0


def run(directory: Path, verb: str, *, target: str | None = None, profile: str | None = None, stage_only: bool = False) -> None:
    if verb != "rehearse" and (target is not None or profile is not None or stage_only):
        raise policy.ToolError("--target, --profile and --stage-only are supported only by rehearsal")
    settings = policy.policy(directory, "release")
    operations = {
        "prepare": prepare,
        "review": review,
        "dry": dry,
        "rehearse": lambda directory, settings: rehearse(
            directory, settings, target=target, profile=profile or "release", stage_only=stage_only
        ),
        "identity": identity,
        "crates-dry": lambda directory, settings: registry.rehearse_crates(directory, skip_published=False),
        "tag": tag,
        "crates": crates,
        "publish": publish,
        "python": lambda directory, settings: registry.python(directory, settings, dry_run=False),
        "finalize": finalize,
        "verify": verify,
        "guard": lambda directory, settings: registry.require_tag(directory, release_tag(directory)),
        "assets": lambda directory, settings: print(
            json.dumps(assets(directory, settings, native_target(settings), release_tag(directory)), indent=2)
        ),
        "stage": lambda directory, settings: stage(directory, settings, release_tag(directory), settings["targets"]),
    }
    operations[verb](directory, settings)


# === Private Functions === #


def _checksums(path: Path, inventory: dict) -> None:
    expected = {name: item["sha256"] for name, item in inventory.items() if name != SUMS}
    actual = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        digest, separator, name = line.partition("  ")
        if not separator or name in actual:
            raise policy.ToolError("invalid or duplicate SHA256SUMS entry")
        actual[name] = digest
    if actual != expected:
        raise policy.ToolError("SHA256SUMS differs from upload.json")


def _tag_event(directory: Path, settings: dict, sha: str) -> dict | None:
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
    workflow = f"{settings['repository']}/.github/workflows/{RELEASE_WORKFLOW}@"
    if not os.environ.get("GITHUB_WORKFLOW_REF", "").startswith(workflow):
        raise policy.ToolError("GitHub tagging runs only in release.yml")
    merged = registry.release_review(settings, sha, registry.RELEASE_BRANCH + release_tag(directory))
    if merged is None:
        raise policy.ToolError("the revision has no merged release review")
    review = registry.document(settings, f"pulls/{merged['number']}")
    if (
        not isinstance(review, dict)
        or review.get("merged") is not True
        or review.get("merge_commit_sha") != sha
        or type(review.get("number")) is not int
        or review.get("base", {}).get("ref") != "dev"
        or review.get("head", {}).get("ref") != registry.RELEASE_BRANCH + release_tag(directory)
        or any(review.get(side, {}).get("repo", {}).get("full_name") != settings["repository"] for side in ("base", "head"))
    ):
        raise policy.ToolError("GitHub review is not the merged release review for this version and revision")
    return review


def _commit_review(directory: Path, settings: dict, branch: str, title: str) -> None:
    """Create The Preparation Commit Through The Provider API, Signed By The Provider."""
    head = _revision(directory)
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
        signature = registry.document(settings, "repository/commits/" + _quote(commit["id"]) + "/signature")
        verified = signature and signature.get("verification_status") in {"verified", "verified_system"}
        if not verified:
            registry.request(settings, "repository/branches/" + _quote(branch), "DELETE")
    if not verified:
        raise policy.ToolError("preparation API commit is not verified; GitLab requires web-based commit signing enabled")


def _bytes(root: Path, item: dict) -> bytes:
    data = policy.inside(root, item["path"]).read_bytes()
    if hashlib.sha256(data).hexdigest() != item["sha256"]:
        raise policy.ToolError(f"staged release bytes changed: {item['name']}")
    return data


def _describe(root: Path, path: Path) -> dict:
    data = path.read_bytes()
    return {"name": path.name, "path": path.relative_to(root).as_posix(), "sha256": hashlib.sha256(data).hexdigest(), "size": len(data)}


def _native() -> str:
    system = {"Linux": "linux", "Windows": "windows", "Darwin": "macos"}.get(platform.system(), "")
    architecture = {"amd64": "x86_64", "arm64": "aarch64"}.get(platform.machine().lower(), platform.machine().lower())
    return f"{system}-{architecture}"


def _online(settings: dict) -> bool:
    """Whether Provider Credentials Allow Reading Remote Tags."""
    names = ("GH_TOKEN", "GITHUB_TOKEN") if settings.get("provider") == "github" else ("RELEASE_TOKEN", "GITLAB_TOKEN")
    return any(os.environ.get(name) for name in names)


def _policy_version(directory: Path, value: str) -> None:
    """Write `[version] value` In The Project Policy, Keeping Every Other Line."""
    path = directory / "tools/project.toml"
    text = path.read_text(encoding="utf-8")
    match = re.search(r"(?ms)(^\[version\]\s*\n)(.*?)(?=^\[|\Z)", text)
    if match is None:
        raise policy.ToolError("prepare requires [version] policy")
    body = re.sub(r"(?m)^value\s*=.*$", f'value = "{value}"', match.group(2), count=1)
    path.write_text(text[: match.start()] + match.group(1) + body + text[match.end() :], encoding="utf-8")


def _package(settings: dict) -> str:
    """The GitLab Generic Package That Carries The Release Downloads."""
    return settings.get("package", settings["product"])


def _push_remote(settings: dict, token: str) -> str:
    """An Authenticated Push URL For The Release Identity, Which The main Ruleset Admits."""
    if settings["provider"] == "github":
        repository = settings.get("repository", os.environ.get("GITHUB_REPOSITORY"))
        return f"https://x-access-token:{token}@github.com/{repository}.git"
    return f"https://oauth2:{token}@{os.environ['CI_SERVER_HOST']}/{os.environ['CI_PROJECT_PATH']}.git"


def _quote(value: str) -> str:
    """A Single Path Segment Of A Provider Endpoint."""
    return urllib.parse.quote(value, safe="")


def _refuse_overrides(directory: Path) -> None:
    """Published Bytes Never Come From Unpublished Local Dependency Checkouts."""
    if not (directory / "Cargo.toml").is_file():
        return
    metadata = json.loads(policy.execute(directory, ["cargo", "metadata", "--locked", "--format-version", "1"], capture=True))
    outside = [
        package["name"]
        for package in metadata["packages"]
        if package["source"] is None and not Path(package["manifest_path"]).resolve().is_relative_to(directory.resolve())
    ]
    if outside:
        raise policy.ToolError(f"unpublished local dependency override: {', '.join(sorted(outside))}")


def _require_publication(directory: Path, settings: dict) -> None:
    """Refuse A Release Whose Crates Cannot Publish: A First Publication Needs The Bootstrap Token."""
    if not (directory / "Cargo.toml").is_file():
        return
    missing = [name for name in registry.publishable(directory) if not registry.crate_published(name)]
    if not missing:
        return
    available = os.environ.get("CRATES_BOOTSTRAP_AVAILABLE") == "true" or bool(os.environ.get("CARGO_REGISTRY_TOKEN"))
    message = f"crates not yet on crates.io need the bootstrap token for their first publication: {', '.join(missing)}"
    if continuous_integration() and not available:
        raise policy.ToolError(message + "; configure the [release] crates_bootstrap_secret environment secret before preparing")
    if not available:
        print("warning: " + message)


def _revision(directory: Path) -> str:
    sha = archive.revision(directory)
    if not sha:
        raise policy.ToolError("release operations require a committed HEAD")
    return sha


def _root(directory: Path) -> Path:
    return policy.inside(directory, ASSETS)


def _run(directory: Path, command: list[str], secret: str) -> str:
    """Run A Command That Carries A Credential, Never Echoing It; Returns Its Scrubbed Standard Output."""
    result = subprocess.run(
        command,
        creationflags=spawn_flags(),
        cwd=directory,
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=False,
    )
    scrub = (lambda text: text.replace(secret, "***")) if secret else (lambda text: text)
    print(scrub(result.stderr or ""), end="", file=sys.stderr)
    if result.returncode:
        raise policy.ToolError(f"{command[0]} {command[1]} failed with exit code {result.returncode}")
    return scrub(result.stdout or "")


@cache
def _tracked(directory: Path, *patterns: str) -> list[str]:
    """Tracked paths matching Git pathspec patterns, where `*` also crosses directories."""
    listed = policy.execute(directory, ["git", "ls-files", "-z", "--", *patterns], capture=True)
    return sorted(filter(None, listed.split("\0")))


def _task_defined(directory: Path, name: str) -> bool:
    with (directory / "mise.toml").open("rb") as file:
        return name in tomllib.load(file).get("tasks", {})


# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
