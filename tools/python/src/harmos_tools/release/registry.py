# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Talk To The Release Provider And The Package Registries"""

# === Imports === #

# Standard Library Imports
import base64
import hashlib
import http.client
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
from typing import IO, Literal, overload

# Third Party Imports
# Local Imports
import harmos_tools.release.review

from ..process import spawn_flags
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# crates.io admits a short burst of new crates, then one per ten minutes.
REHEARSAL_CHECK = "Release / Rehearsal"
RELEASE_BRANCH = "release/"
GITLAB_REHEARSAL_JOB = "Release / Rehearsal"
# Transient Provider Failures Retry With Doubling Pauses; Only Idempotent Methods Retry Server Errors.
RETRY_ATTEMPTS = 4
RETRY_PAUSE_SECONDS = 2
THROTTLED = frozenset({429})
TRANSIENT = frozenset({500, 502, 503, 504})
IDEMPOTENT = frozenset({"GET", "PUT", "PATCH", "DELETE"})

# === Types === #

# === Classes === #


class ProviderRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(
        self, req: urllib.request.Request, fp: IO[bytes], code: int, msg: str, headers: http.client.HTTPMessage, newurl: str
    ) -> urllib.request.Request | None:
        redirected = super().redirect_request(req, fp, code, msg, headers, newurl)
        origin = urllib.parse.urlsplit(req.full_url)
        destination = urllib.parse.urlsplit(newurl)
        if destination.scheme != "https":
            raise policy.ToolError(
                "release download redirect must use HTTPS; Configure the release download endpoint to redirect over HTTPS"
            )
        if redirected is not None and (origin.scheme, origin.netloc) != (destination.scheme, destination.netloc):
            for name in ["Authorization", "PRIVATE-TOKEN", "JOB-TOKEN"]:
                redirected.remove_header(name)
        return redirected


# === Public Functions === #


def request(
    settings: dict,
    endpoint: str,
    method: str = "GET",
    data: bytes | None = None,
    *,
    binary: bool = False,
    upload: bool = False,
    graphql: bool = False,
    channel: bool = False,
) -> bytes | None:
    provider = settings["provider"]
    if provider == "github":
        base = os.environ.get("GITHUB_API_URL", settings.get("api_url", "https://api.github.com"))
        repository = settings.get("repository", os.environ.get("GITHUB_REPOSITORY"))
        token = os.environ.get("GH_TOKEN", os.environ.get("GITHUB_TOKEN"))
        if method == "GET" and endpoint.startswith("actions/"):
            token = os.environ.get("GITHUB_TOKEN") or token
        if not repository or not token:
            raise policy.ToolError("GitHub repository and native token required; Configure release.repository and GH_TOKEN or GITHUB_TOKEN")
        headers = {"Authorization": "Bearer " + token, "X-GitHub-Api-Version": "2022-11-28"}
        if upload:
            base = settings.get("upload_url", "https://uploads.github.com")
        url = (
            os.environ.get("GITHUB_GRAPHQL_URL", base.rstrip("/").removesuffix("/v3") + "/graphql")
            if graphql
            else f"{base}/repos/{repository}/{endpoint}"
        )
        # Release assets return their bytes only for octet-stream; every other REST call accepts only the REST
        # media type and answers 415 otherwise.
        headers["Accept"] = (
            "application/octet-stream" if binary and endpoint.startswith("releases/assets/") else "application/vnd.github+json"
        )
    elif provider == "gitlab":
        base = os.environ.get("CI_API_V4_URL", settings.get("api_url", "https://gitlab.com/api/v4"))
        if not isinstance(base, str) or not base:
            raise policy.ToolError("GitLab API URL required; Configure CI_API_V4_URL or release.api_url")
        project = settings.get("project_id", os.environ.get("CI_PROJECT_ID"))
        token = os.environ.get("RELEASE_TOKEN") or os.environ.get("GITLAB_TOKEN")
        job_token = os.environ.get("CI_JOB_TOKEN")
        if not project or not (token or job_token):
            raise policy.ToolError(
                "GitLab project and protected native token required; Configure the project ID and protected RELEASE_TOKEN or CI_JOB_TOKEN"
            )
        if graphql:
            if not token or not base.rstrip("/").endswith("/api/v4"):
                raise policy.ToolError(
                    "GitLab policy read requires RELEASE_TOKEN and a native /api/v4 URL; "
                    "Configure RELEASE_TOKEN and a native /api/v4 API URL"
                )
            headers = {"Authorization": "Bearer " + token}
            url = base.rstrip("/").removesuffix("/v4") + "/graphql"
        else:
            if channel:
                resource = endpoint.split("?", 1)[0]
                links = re.fullmatch(r"releases/nightly/assets/links(?:/[0-9]+)?", resource)
                release_creation = resource == "releases" and method == "POST"
                nightly_release = resource == "releases/nightly" and method in {"GET", "PUT"}
                nightly_links = bool(links) and method in {"GET", "POST", "DELETE"}
                allowed = release_creation or nightly_release or nightly_links
                if not allowed:
                    raise policy.ToolError(
                        "nightly requires native job-token release endpoints; Select the native Nightly release or release-link endpoint"
                    )
            cleanup = method == "DELETE" and endpoint.startswith("repository/branches/release%2Fv")
            merging = method == "PUT" and re.fullmatch(r"merge_requests/[0-9]+/merge", endpoint) is not None
            tag_creation = method == "POST" and endpoint in {"repository/tags", "repository/commits"}
            tagging = tag_creation or cleanup or merging
            package_upload = method == "PUT" and endpoint.startswith("packages/generic/")
            release_creation = method == "POST" and endpoint == "releases"
            publication = package_upload or release_creation
            if not channel and method != "GET" and not tagging and not publication:
                raise policy.ToolError(
                    "GitLab release write endpoint is not allowed; Select a supported tagging, release or generic package endpoint"
                )
            if channel:
                if not job_token:
                    raise policy.ToolError("nightly requires native CI_JOB_TOKEN; Run Nightly with the native CI_JOB_TOKEN")
                headers = {"JOB-TOKEN": job_token}
            elif tagging:
                if not token:
                    raise policy.ToolError(
                        "GitLab tagging and release reviews require the RELEASE_TOKEN release identity; "
                        "Configure the protected RELEASE_TOKEN release identity"
                    )
                headers = {"Authorization": "Bearer " + token}
            elif method == "GET":
                if not token:
                    raise policy.ToolError(
                        "GitLab reads require RELEASE_TOKEN with read_api access; Configure RELEASE_TOKEN with read_api access"
                    )
                headers = {"Authorization": "Bearer " + token}
            elif job_token:
                headers = {"JOB-TOKEN": job_token}
            elif os.environ.get("GITLAB_CI"):
                raise policy.ToolError("GitLab CI publication requires CI_JOB_TOKEN; Run publication with the native CI_JOB_TOKEN")
            elif token:
                headers = {"Authorization": "Bearer " + token}
            else:
                raise policy.ToolError("GitLab local publication requires RELEASE_TOKEN; Configure RELEASE_TOKEN for local publication")
            url = f"{base}/projects/{urllib.parse.quote(str(project), safe='')}/{endpoint}"
        headers["Accept"] = "application/octet-stream" if binary else "application/json"
    else:
        raise policy.ToolError("release.provider must be github or gitlab; Set release.provider to github or gitlab")
    if data is not None:
        headers["Content-Type"] = "application/octet-stream" if binary else "application/json"
    for attempt in range(1, RETRY_ATTEMPTS + 1):
        native = urllib.request.Request(url, data=data, headers=headers, method=method)
        try:
            response = urllib.request.build_opener(ProviderRedirect()).open(native, timeout=60)
            try:
                return response.read()
            finally:
                response.close()
        except urllib.error.HTTPError as error:
            try:
                if error.code == 404 and (method == "GET" or method == "DELETE" and attempt > 1):
                    return None
                message = error.read().decode("utf-8", errors="replace")
                for secret in (token, os.environ.get("CI_JOB_TOKEN")):
                    if secret:
                        message = message.replace(secret, "***")
                retry = error.code in THROTTLED or error.code in TRANSIENT and method in IDEMPOTENT
                if not retry or attempt == RETRY_ATTEMPTS:
                    raise policy.ToolError(
                        f"release provider {method} failed: HTTP {error.code}: {message}; "
                        "Inspect the release provider response and correct its revision, workflow evidence or credentials"
                    ) from error
                print(f"release provider {method} {endpoint}: HTTP {error.code}; retrying ({attempt}/{RETRY_ATTEMPTS})", flush=True)
                time.sleep(RETRY_PAUSE_SECONDS * 2 ** (attempt - 1))
            finally:
                error.close()
    raise AssertionError("unreachable")


@overload
def document(
    settings: dict,
    endpoint: str,
    method: str = "GET",
    body: dict | None = None,
    *,
    channel: bool = False,
    collection: Literal[False] = False,
) -> dict | None: ...


@overload
def document(
    settings: dict, endpoint: str, method: str = "GET", body: dict | None = None, *, channel: bool = False, collection: Literal[True]
) -> list[dict] | None: ...


def document(
    settings: dict, endpoint: str, method: str = "GET", body: dict | None = None, *, channel: bool = False, collection: bool = False
) -> dict | list[dict] | None:
    value = request(settings, endpoint, method, json.dumps(body).encode() if body is not None else None, channel=channel)
    if not value:
        return None
    result = json.loads(value)
    if collection:
        if not isinstance(result, list) or any(not isinstance(item, dict) for item in result):
            raise policy.ToolError(
                "provider collection must contain objects; Check the configured provider endpoint and its response schema"
            )
        return result
    if not isinstance(result, dict):
        raise policy.ToolError("provider response must be an object; Check the configured provider endpoint and its response schema")
    return result


def pages(settings: dict, endpoint: str, key: str | None = None, *, channel: bool = False) -> list:
    result = []
    page = 1
    while True:
        separator = "&" if "?" in endpoint else "?"
        address = f"{endpoint}{separator}per_page=100&page={page}"
        response = document(settings, address, channel=channel) if key else document(settings, address, channel=channel, collection=True)
        values = response.get(key, []) if key and isinstance(response, dict) else response
        if not isinstance(values, list):
            raise policy.ToolError(
                "provider returned invalid paginated evidence; Check the configured provider endpoint and its pagination response schema"
            )
        result.extend(values)
        if len(values) < 100:
            return result
        page += 1


def arm_auto_merge(settings: dict, branch: str, title: str, *, sleep: Callable[[float], None] = time.sleep) -> None:
    """Arm Native Squash Auto-Merge After Opening The Release Review, Waiting Briefly For GitLab's Pipeline."""
    if settings["provider"] == "github":
        repository = settings.get("repository", os.environ.get("GITHUB_REPOSITORY", ""))
        head = urllib.parse.quote(repository.split("/")[0] + ":" + branch, safe="")
        reviews = pages(settings, f"pulls?state=open&base=dev&head={head}")
    else:
        source = urllib.parse.quote(branch, safe="")
        reviews = pages(settings, f"merge_requests?state=opened&target_branch=dev&source_branch={source}")
    if len(reviews) != 1:
        raise policy.ToolError(
            f"expected one open release review for {branch}; auto-merge was not armed; "
            "Inspect the release provider response and correct its revision, workflow evidence or credentials"
        )
    review = reviews[0]
    if settings["provider"] == "github":
        payload = {
            "query": (
                "mutation($input: EnablePullRequestAutoMergeInput!) { enablePullRequestAutoMerge(input: $input) "
                "{ pullRequest { autoMergeRequest { mergeMethod commitHeadline } } } }"
            ),
            "variables": {"input": {"pullRequestId": review["node_id"], "mergeMethod": "SQUASH", "commitHeadline": title}},
        }
        response = json.loads(request(settings, "graphql", "POST", json.dumps(payload).encode(), graphql=True) or b"{}")
        if response.get("errors"):
            raise policy.ToolError("enablePullRequestAutoMerge failed: " + json.dumps(response["errors"]))
        result = ((response.get("data") or {}).get("enablePullRequestAutoMerge") or {}).get("pullRequest") or {}
        if result.get("autoMergeRequest") != {"mergeMethod": "SQUASH", "commitHeadline": title}:
            raise policy.ToolError(
                f"GitHub did not confirm squash auto-merge for {branch}; "
                "Inspect the release provider response and correct its revision, workflow evidence or credentials"
            )
        return

    endpoint = f"merge_requests/{review['iid']}"
    for attempt in range(12):
        current = document(settings, endpoint) or {}
        if current.get("head_pipeline"):
            try:
                result = (
                    document(
                        settings,
                        endpoint + "/merge",
                        "PUT",
                        {
                            "merge_when_pipeline_succeeds": True,
                            "squash": True,
                            "squash_commit_message": title,
                            "should_remove_source_branch": True,
                        },
                    )
                    or {}
                )
            except policy.ToolError as error:
                # GitLab can still be computing mergeability after creating the head pipeline.
                if not isinstance(error.__cause__, urllib.error.HTTPError) or error.__cause__.code != 405:
                    raise
            else:
                if result.get("merge_when_pipeline_succeeds") or result.get("state") == "merged":
                    return
                raise policy.ToolError(
                    f"GitLab did not confirm auto-merge for {branch}; "
                    "Inspect the release provider response and correct its revision, workflow evidence or credentials"
                )
        if attempt < 11:
            sleep(5)
    raise policy.ToolError(
        f"GitLab release review for {branch} was not ready for auto-merge after 12 attempts; the review remains open; "
        "Inspect the release provider response and correct its revision, workflow evidence or credentials"
    )


def verify_duplicate_policy(settings: dict) -> None:
    """Read Back The Group's Generic Package Duplicate Protection; Publication Never Changes It."""
    project = document(settings, "")
    if not isinstance(project, dict):
        raise policy.ToolError("GitLab project identity is unavailable; Verify the configured project ID and project API access")
    path = project.get("path_with_namespace")
    owner = project.get("namespace")
    namespace = owner.get("full_path") if isinstance(owner, dict) else None
    if not isinstance(path, str) or not isinstance(namespace, str):
        raise policy.ToolError("GitLab group namespace identity is unavailable; Verify the project group and its namespace API access")
    query = "query ReleasePolicy($project: ID!) { project(fullPath: $project) { fullPath group { fullPath packageSettings { genericDuplicatesAllowed genericDuplicateExceptionRegex } } } }"
    raw = request(settings, "graphql", "POST", json.dumps({"query": query, "variables": {"project": path}}).encode(), graphql=True)
    result = json.loads(raw) if raw else {}
    if not isinstance(result, dict) or result.get("errors"):
        raise policy.ToolError("GitLab duplicate policy query failed; Inspect the group duplicate-protection API response")
    data = result.get("data")
    selected = data.get("project") if isinstance(data, dict) else None
    group = selected.get("group") if isinstance(selected, dict) else None
    package = group.get("packageSettings") if isinstance(group, dict) else None
    if (
        not isinstance(selected, dict)
        or selected.get("fullPath") != path
        or not isinstance(group, dict)
        or group.get("fullPath") != namespace
        or not isinstance(package, dict)
        or package.get("genericDuplicatesAllowed") is not False
        or package.get("genericDuplicateExceptionRegex") != ""
    ):
        raise policy.ToolError(
            "GitLab generic package duplicate protection must be verified for the project's group; "
            "Verify generic package duplicate protection for the owning group"
        )


def require_tag(directory: Path, tag: str) -> None:
    """The Protected Push Of Exactly This Stable Tag, Checked Out At Its Commit."""
    if re.fullmatch(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", tag) is None:
        raise policy.ToolError("release requires a stable release tag; Prepare the stable release and use its protected version tag")
    if os.environ.get("GITHUB_ACTIONS"):
        allowed = (
            os.environ.get("GITHUB_EVENT_NAME") == "push"
            and os.environ.get("GITHUB_REF") == "refs/tags/" + tag
            and os.environ.get("GITHUB_REF_PROTECTED") == "true"
        )
        sha = os.environ.get("GITHUB_SHA")
    else:
        allowed = (
            os.environ.get("PIPELINE_SOURCE", os.environ.get("CI_PIPELINE_SOURCE")) == "push"
            and os.environ.get("CI_COMMIT_TAG") == tag
            and os.environ.get("CI_COMMIT_REF_PROTECTED") == "true"
        )
        sha = os.environ.get("CI_COMMIT_SHA")
    if not allowed:
        raise policy.ToolError(
            f"publication requires the protected push of {tag}; an unprotected tag means the tag ruleset is missing; "
            "Configure the release tag ruleset and publish from its protected push"
        )
    actual = policy.execute(directory, ["git", "rev-parse", "HEAD"], capture=True).strip()
    tagged = policy.execute(directory, ["git", "rev-parse", "refs/tags/" + tag + "^{commit}"], capture=True).strip()
    if actual != sha or tagged != actual:
        raise policy.ToolError("release tag SHA differs from checkout; Use the checkout at the immutable release tag revision")


def draft(settings: dict, tag: str, sha: str, body: str) -> dict:
    """The GitHub Draft For This Tag: Reused While Still A Draft, Never Replacing A Published Release."""
    releases = [item for item in pages(settings, "releases") if item["tag_name"] == tag]
    if len(releases) > 1:
        raise policy.ToolError("release tag identity ambiguous; Resolve the release tag to one immutable commit before publication")
    if releases and not releases[0]["draft"]:
        raise policy.ToolError("published release cannot be replaced; use a new patch version")
    if releases:
        return releases[0]
    created = document(
        settings,
        "releases",
        "POST",
        {"tag_name": tag, "target_commitish": sha, "name": tag, "body": body, "draft": True, "prerelease": False},
    )
    if not isinstance(created, dict) or not created.get("draft") or created.get("tag_name") != tag:
        raise policy.ToolError("release draft was not created as a draft; Inspect the provider release state before uploading assets")
    return created


def upload(settings: dict, release: int, item: dict, data: bytes) -> str:
    """Upload One Asset To A Draft Unless Present, Then Verify Its Remote Bytes."""
    available = [value for value in pages(settings, f"releases/{release}/assets") if value["name"] == item["name"]]
    if len(available) > 1:
        raise policy.ToolError("duplicate draft asset; Inspect the draft asset inventory before continuing publication")
    if available:
        endpoint = f"releases/assets/{available[0]['id']}"
    else:
        name = urllib.parse.quote(item["name"], safe="")
        uploaded = request(settings, f"releases/{release}/assets?name={name}", "POST", data, binary=True, upload=True)
        endpoint = f"releases/assets/{json.loads(uploaded or b'{}')['id']}"
    verify_download(settings, endpoint, item)
    return endpoint


def upload_generic(settings: dict, package: str, tag: str, item: dict, data: bytes) -> dict:
    """Upload One Asset To The Duplicate-Protected GitLab Generic Package Version."""
    endpoint = f"packages/generic/{urllib.parse.quote(package, safe='')}/{urllib.parse.quote(tag, safe='')}/{urllib.parse.quote(item['name'], safe='')}"
    existing = request(settings, endpoint, binary=True)
    if existing is not None and existing != data:
        raise policy.ToolError("partial upload conflict; use a new patch version")
    if existing is None:
        request(settings, endpoint, "PUT", data, binary=True)
    verify_download(settings, endpoint, item)
    base = os.environ.get("CI_API_V4_URL", settings.get("api_url", "https://gitlab.com/api/v4"))
    project = urllib.parse.quote(str(settings.get("project_id", os.environ.get("CI_PROJECT_ID"))), safe="")
    return {"endpoint": endpoint, "link": {"name": item["name"], "url": f"{base}/projects/{project}/{endpoint}"}}


def verify_download(settings: dict, endpoint: str, item: dict, *, channel: bool = False) -> bytes:
    data = request(settings, endpoint, binary=True, channel=channel)
    if data is None or len(data) != item["size"] or hashlib.sha256(data).hexdigest() != item["sha256"]:
        raise policy.ToolError(
            f"remote release download differs: {item['name']}; Inspect the downloaded asset and its recorded digest before publication"
        )
    return data


def verify_signatures(directory: Path, settings: dict, root: Path, names: list[str]) -> None:
    """Run Product-Owned Signature Checks Over Hash-Verified Bytes With RELEASE_ASSET_ROOT Pointing At Them."""
    previous = os.environ.get("RELEASE_ASSET_ROOT")
    os.environ["RELEASE_ASSET_ROOT"] = str(root)
    try:
        for name in names:
            if not policy.inside(root, name).is_file():
                raise policy.ToolError(
                    f"signature check input missing: {name}; Stage the declared signature-check inputs before verification"
                )
        for command in settings.get("signature_checks", []):
            policy.execute(directory, policy.command(command, "release.signature_checks"))
    finally:
        if previous is None:
            os.environ.pop("RELEASE_ASSET_ROOT", None)
        else:
            os.environ["RELEASE_ASSET_ROOT"] = previous


def release_review(settings: dict, sha: str, branch: str) -> dict | None:
    """The Merged Release Review Whose Merge Commit Is This Revision, If Any."""
    if settings["provider"] == "github":
        matches = [
            item
            for item in pages(settings, f"commits/{sha}/pulls")
            if item.get("merged_at")
            and item.get("merge_commit_sha") == sha
            and item.get("base", {}).get("ref") == "dev"
            and item.get("head", {}).get("ref") == branch
        ]
        if len(matches) > 1:
            raise policy.ToolError(
                "the revision identifies several merged release pull requests; "
                "Inspect the release provider response and correct its revision, workflow evidence or credentials"
            )
        return {"head": matches[0]["head"]["sha"], "number": matches[0]["number"]} if matches else None
    matches = [
        item
        for item in pages(settings, f"repository/commits/{sha}/merge_requests")
        if item.get("state") == "merged"
        and sha in {item.get("merge_commit_sha"), item.get("squash_commit_sha"), item.get("sha")}
        and item.get("target_branch") == "dev"
        and item.get("source_branch") == branch
    ]
    if len(matches) > 1:
        raise policy.ToolError(
            "the revision identifies several merged release merge requests; "
            "Inspect the release provider response and correct its revision, workflow evidence or credentials"
        )
    return {"head": matches[0]["sha"], "number": matches[0]["iid"]} if matches else None


def rehearsed(directory: Path, settings: dict, review: dict, sha: str) -> None:
    """The Release Review Head Passed Its Rehearsal And Its Tree Is Exactly The Merged Tree."""
    head = review["head"]
    if settings["provider"] == "github":
        pull = document(settings, f"pulls/{review['number']}")
        runs = pages(settings, f"actions/workflows/ci.yml/runs?event=pull_request&head_sha={head}", "workflow_runs")
        try:
            if not isinstance(pull, dict) or pull.get("number") != review["number"] or pull.get("head", {}).get("sha") != head:
                raise ValueError("review identity or head differs")
            candidates = [run for run in runs if harmos_tools.release.review.github_review_run(run, pull)]
            _, jobs = harmos_tools.release.review.github_validation_run(
                candidates, lambda identifier: pages(settings, f"actions/runs/{identifier}/jobs?filter=latest", "jobs")
            )
        except (KeyError, TypeError, ValueError) as error:
            raise policy.ToolError(
                f"release PR #{review['number']} head {head} has no green validation: {error}; "
                "Inspect the release provider response and correct its revision, workflow evidence or credentials"
            ) from error
        if not any(job.get("name") == REHEARSAL_CHECK and job.get("conclusion") == "success" for job in jobs):
            raise policy.ToolError(
                f"release PR #{review['number']} head {head} has no successful {REHEARSAL_CHECK} check; "
                "Inspect the release provider response and correct its revision, workflow evidence or credentials"
            )
        _same_tree(directory, head, sha)
        return
    # The merge request's latest pipeline ran on its head; its rehearsal job must have succeeded there.
    merged_review = document(settings, f"merge_requests/{review['number']}")
    pipeline = merged_review.get("head_pipeline") if isinstance(merged_review, dict) else None
    if not isinstance(pipeline, dict) or pipeline.get("sha") != head or not isinstance(pipeline.get("id"), int):
        raise policy.ToolError(
            f"release MR !{review['number']} has no pipeline for its head {head}; "
            "Inspect the release provider response and correct its revision, workflow evidence or credentials"
        )
    pipeline_jobs = pages(settings, f"pipelines/{pipeline['id']}/jobs?include_retried=false")
    if any(job.get("name") == "Plan / Impact" for job in pipeline_jobs):
        bridges = [bridge for bridge in pages(settings, f"pipelines/{pipeline['id']}/bridges") if bridge.get("name") == "CI / Required"]
        if len(bridges) != 1 or bridges[0].get("status") != "success":
            raise policy.ToolError(
                "release review child gate is not green; "
                "Inspect the release provider response and correct its revision, workflow evidence or credentials"
            )
        child = bridges[0].get("downstream_pipeline")
        if (
            not isinstance(child, dict)
            or child.get("sha") != head
            or child.get("status") != "success"
            or not isinstance(child.get("id"), int)
        ):
            raise policy.ToolError(
                "release review child pipeline is not green for its head; "
                "Inspect the release provider response and correct its revision, workflow evidence or credentials"
            )
        pipeline = child
        pipeline_jobs = pages(settings, f"pipelines/{pipeline['id']}/jobs?include_retried=false")
    jobs = [job for job in pipeline_jobs if job.get("name") == GITLAB_REHEARSAL_JOB]
    if not jobs or any(job.get("status") != "success" for job in jobs):
        raise policy.ToolError(
            f"release MR !{review['number']} pipeline {pipeline['id']} has no successful {GITLAB_REHEARSAL_JOB} job; "
            "Inspect the release provider response and correct its revision, workflow evidence or credentials"
        )
    _same_tree(directory, head, sha)


def oidc_claims(url: str, token: str) -> dict:
    """The Claims Of A GitHub OIDC Token Requested For Diagnosis Only; The Token Itself Is Never Printed."""
    separator = "&" if "?" in url else "?"
    native = urllib.request.Request(f"{url}{separator}audience=release-identity", headers={"Authorization": "Bearer " + token})
    response = urllib.request.urlopen(native, timeout=60)
    try:
        value = json.loads(response.read())["value"]
    finally:
        response.close()
    payload = value.split(".")[1]
    # Read Structured Input
    text = base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4))
    return json.loads(text)


def workflow_filename(reference: str) -> str:
    """The Workflow File Name Of A `owner/repo/.github/workflows/<file>@<ref>` Reference."""
    match = re.search(r"([^/]+\.ya?ml)(?=@)", reference)
    return match.group(1) if match else "(unknown)"


def delete_branch(settings: dict, branch: str) -> bool:
    """Delete A Remote Branch If It Still Exists, Since Native Source-Branch Removal Usually Got There First."""
    if settings["provider"] == "github":
        name = urllib.parse.quote(branch, safe="/")
        if document(settings, "git/ref/heads/" + name) is None:
            return False
        request(settings, "git/refs/heads/" + name, "DELETE")
    else:
        endpoint = "repository/branches/" + urllib.parse.quote(branch, safe="")
        if document(settings, endpoint) is None:
            return False
        request(settings, endpoint, "DELETE")
    return True


def create_tag(settings: dict, tag: str, sha: str) -> None:
    """Create The Lightweight Release Tag Through The Provider With The Release Token."""
    if settings["provider"] == "github":
        document(settings, "git/refs", "POST", {"ref": "refs/tags/" + tag, "sha": sha})
    else:
        document(settings, "repository/tags", "POST", {"tag_name": tag, "ref": sha})


def remote_tag(settings: dict, tag: str) -> str | None:
    """The Commit A Remote Tag Identifies, Peeling Annotated Tags."""
    name = urllib.parse.quote(tag, safe="")
    if settings["provider"] == "github":
        existing = document(settings, "git/ref/tags/" + name)
        if not existing:
            return None
        target = existing.get("object", {})
        if target.get("type") == "tag":
            target = (document(settings, "git/tags/" + target["sha"]) or {}).get("object", {})
        return target.get("sha")
    existing = document(settings, "repository/tags/" + name)
    return existing.get("commit", {}).get("id") if existing else None


# === Private Functions === #


def _same_tree(directory: Path, head: str, merged: str) -> None:
    revisions = (head, merged)
    if any(not isinstance(revision, str) or not re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", revision) for revision in revisions):
        raise policy.ToolError(
            "rehearsal tree verification requires full commit object IDs; "
            "Pass complete hexadecimal commit IDs for the rehearsed and merged revisions"
        )
    commands = [["git", "fetch", "--quiet", "--no-tags", "--no-write-fetch-head", "origin", *revisions]]
    commands.extend(["git", "rev-parse", "--verify", revision + "^{tree}"] for revision in revisions)
    outputs = []
    for command in commands:
        try:
            output = subprocess.run(
                command,
                creationflags=spawn_flags(),
                cwd=directory,
                env={**os.environ, "GIT_TERMINAL_PROMPT": "0"},
                capture_output=True,
                text=True,
                timeout=120,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise policy.ToolError(
                "could not fetch and resolve the rehearsed and merged Git trees; "
                "Verify remote access to the rehearsed and merged commit objects"
            ) from error
        if output.returncode:
            raise policy.ToolError(
                "could not fetch and resolve the rehearsed and merged Git trees; "
                "Verify remote access to the rehearsed and merged commit objects"
            )
        outputs.append(output.stdout.strip())
    trees = outputs[1:]
    if any(not re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", tree) for tree in trees):
        raise policy.ToolError(
            "rehearsal tree verification returned no valid tree object IDs; "
            "Verify that the rehearsed and merged revisions resolve to Git tree objects"
        )
    if trees[0] != trees[1]:
        raise policy.ToolError("the merged tree differs from the rehearsed release review head; update the review and merge again")


# === Entry Point === #
