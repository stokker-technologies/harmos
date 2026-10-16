# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Verify Native Release Authorization And Durable Provider Publication"""

# === Imports === #

# Standard Library Imports
import hashlib
import io
import json
import os
import tempfile
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from pathlib import Path
from typing import Any

# Third Party Imports

# Local Imports
# isort: split
from . import _policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class ProviderRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        redirected = super().redirect_request(req, fp, code, msg, headers, newurl)
        origin = urllib.parse.urlsplit(req.full_url)
        destination = urllib.parse.urlsplit(newurl)
        if destination.scheme != "https":
            raise _policy.ToolError("release download redirect must use HTTPS")
        if redirected is not None and (origin.scheme, origin.netloc) != (
            destination.scheme,
            destination.netloc,
        ):
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
) -> bytes | None:
    provider = settings["provider"]
    if provider == "github":
        base = os.environ.get(
            "GITHUB_API_URL", settings.get("api_url", "https://api.github.com")
        )
        repository = settings.get("repository", os.environ.get("GITHUB_REPOSITORY"))
        token = os.environ.get("GH_TOKEN", os.environ.get("GITHUB_TOKEN"))
        if not repository or not token:
            raise _policy.ToolError("GitHub repository and native token required")
        headers = {
            "Authorization": "Bearer " + token,
            "X-GitHub-Api-Version": "2022-11-28",
        }
        if upload:
            base = settings.get("upload_url", "https://uploads.github.com")
        url = f"{base}/repos/{repository}/{endpoint}"
    elif provider == "gitlab":
        base = os.environ.get(
            "CI_API_V4_URL", settings.get("api_url", "https://gitlab.com/api/v4")
        )
        if not isinstance(base, str) or not base:
            raise _policy.ToolError("GitLab API URL required")
        project = settings.get("project_id", os.environ.get("CI_PROJECT_ID"))
        token = os.environ.get("GITLAB_TOKEN")
        job_token = os.environ.get("CI_JOB_TOKEN")
        if not project or not (token or job_token):
            raise _policy.ToolError(
                "GitLab project and protected native token required"
            )
        if graphql:
            if not token or not base.rstrip("/").endswith("/api/v4"):
                raise _policy.ToolError(
                    "GitLab policy read requires GITLAB_TOKEN and a native /api/v4 URL"
                )
            headers = {"Authorization": "Bearer " + token}
            url = base.rstrip("/").removesuffix("/v4") + "/graphql"
        else:
            if method == "GET":
                if not token:
                    raise _policy.ToolError(
                        "GitLab reads require GITLAB_TOKEN with read_api access"
                    )
                headers = {"Authorization": "Bearer " + token}
            else:
                allowed = (
                    method == "PUT" and endpoint.startswith("packages/generic/")
                ) or (method == "POST" and endpoint == "releases")
                if not allowed:
                    raise _policy.ToolError(
                        "GitLab release write endpoint is not allowed"
                    )
                ci_project = os.environ.get("CI_PROJECT_ID")
                if job_token:
                    identity = document(settings, "")
                    if (
                        not ci_project
                        or not isinstance(identity, dict)
                        or str(identity.get("id")) != ci_project
                    ):
                        raise _policy.ToolError(
                            "GitLab job-token publication requires the same CI project"
                        )
                    headers = {"JOB-TOKEN": job_token}
                elif ci_project or os.environ.get("GITLAB_CI"):
                    raise _policy.ToolError(
                        "GitLab CI publication requires CI_JOB_TOKEN"
                    )
                elif token:
                    headers = {"Authorization": "Bearer " + token}
                else:
                    raise _policy.ToolError(
                        "GitLab local publication requires GITLAB_TOKEN"
                    )
            url = f"{base}/projects/{urllib.parse.quote(str(project), safe='')}/{endpoint}"
    else:
        raise _policy.ToolError("release.provider must be github or gitlab")
    headers["Accept"] = "application/octet-stream" if binary else "application/json"
    headers["Content-Type"] = (
        "application/octet-stream" if binary else "application/json"
    )
    native = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.build_opener(ProviderRedirect()).open(
            native, timeout=60
        ) as response:
            return response.read()
    except urllib.error.HTTPError as error:
        if error.code == 404 and method == "GET":
            return None
        raise _policy.ToolError(
            f"release provider {method} failed: HTTP {error.code}"
        ) from error


def document(
    settings: dict, endpoint: str, method: str = "GET", body: dict | None = None
) -> Any:
    value = request(
        settings,
        endpoint,
        method,
        json.dumps(body).encode() if body is not None else None,
    )
    return json.loads(value) if value is not None else None


def verify_duplicate_policy(settings: dict) -> None:
    # Job Tokens Cannot Read GraphQL Or The Complete Native Validation APIs.
    if not os.environ.get("GITLAB_TOKEN"):
        raise _policy.ToolError(
            "GitLab publication requires protected GITLAB_TOKEN with read_api and namespace read access; CI_JOB_TOKEN alone is insufficient"
        )
    project = document(settings, "")
    if not isinstance(project, dict):
        raise _policy.ToolError("GitLab project identity is unavailable")
    path = project.get("path_with_namespace")
    owner = project.get("namespace")
    namespace = owner.get("full_path") if isinstance(owner, dict) else None
    if not isinstance(path, str) or not isinstance(namespace, str):
        raise _policy.ToolError("GitLab group namespace identity is unavailable")
    query = "query ReleasePolicy($project: ID!) { project(fullPath: $project) { fullPath group { fullPath packageSettings { genericDuplicatesAllowed genericDuplicateExceptionRegex } } } }"
    body = json.dumps({"query": query, "variables": {"project": path}}).encode()
    raw = request(settings, "graphql", "POST", body, graphql=True)
    result = json.loads(raw) if raw is not None else {}
    if not isinstance(result, dict) or result.get("errors"):
        raise _policy.ToolError("GitLab duplicate policy query failed")
    data = result.get("data")
    selected = data.get("project") if isinstance(data, dict) else None
    group = selected.get("group") if isinstance(selected, dict) else None
    policy = group.get("packageSettings") if isinstance(group, dict) else None
    if (
        not isinstance(selected, dict)
        or selected.get("fullPath") != path
        or not isinstance(group, dict)
        or group.get("fullPath") != namespace
        or not isinstance(policy, dict)
        or policy.get("genericDuplicatesAllowed") is not False
        or policy.get("genericDuplicateExceptionRegex") != ""
        or "genericDuplicateExceptionRegex" not in policy
    ):
        raise _policy.ToolError(
            "GitLab generic package duplicate protection must be verified for the project's group"
        )


def pages(settings: dict, endpoint: str, key: str | None = None) -> list:
    result = []
    page = 1
    while True:
        separator = "&" if "?" in endpoint else "?"
        response = document(settings, f"{endpoint}{separator}per_page=100&page={page}")
        values = (
            response.get(key, []) if key and isinstance(response, dict) else response
        )
        if not isinstance(values, list):
            raise _policy.ToolError("provider returned invalid paginated evidence")
        result.extend(values)
        if len(values) < 100:
            return result
        page += 1


def job_identity(settings: dict) -> str:
    if settings["provider"] == "gitlab":
        return os.environ.get("CI_JOB_ID", "")
    run = os.environ.get("GITHUB_RUN_ID")
    attempt = os.environ.get("GITHUB_RUN_ATTEMPT", "1")
    name = os.environ.get("RELEASE_JOB_NAME")
    if not run or not name:
        raise _policy.ToolError("GitHub stage requires run and exact RELEASE_JOB_NAME")
    matches = [
        job
        for job in pages(
            settings, f"actions/runs/{run}/attempts/{attempt}/jobs", "jobs"
        )
        if job["name"] == name
    ]
    if len(matches) != 1:
        raise _policy.ToolError(
            "GitHub job name does not identify exactly one native job"
        )
    return str(matches[0]["id"])


def verify_jobs(
    settings: dict, receipt: dict, *, require_artifact: bool = True
) -> None:
    if receipt["provider"] != settings["provider"]:
        raise _policy.ToolError("native provider differs from release policy")
    if settings["provider"] == "gitlab" and not os.environ.get("GITLAB_TOKEN"):
        raise _policy.ToolError(
            "GitLab native job/pipeline verification requires GITLAB_TOKEN; CI_JOB_TOKEN cannot read these endpoints"
        )
    sha = receipt["sha"]
    run_id = receipt["run_id"]
    if settings["provider"] == "github":
        run = document(settings, f"actions/runs/{run_id}")
        if not isinstance(run, dict) or run.get("head_sha") != sha:
            raise _policy.ToolError("GitHub run SHA differs")
        for job_id in [receipt["build_job"], receipt["validation"]["job"]]:
            job = document(settings, f"actions/jobs/{job_id}")
            if (
                not isinstance(job, dict)
                or str(job.get("run_id")) != str(run_id)
                or job.get("head_sha") != sha
                or job.get("conclusion") != "success"
                or job.get("status") != "completed"
            ):
                raise _policy.ToolError(
                    "required GitHub job missing, failed, skipped, cancelled or mismatched"
                )
        if not require_artifact:
            return
        name = f"release-{sha}-{receipt['target']}"
        artifacts = [
            value
            for value in pages(
                settings, f"actions/runs/{run_id}/artifacts", "artifacts"
            )
            if value["name"] == name and not value["expired"]
        ]
        if len(artifacts) != 1:
            raise _policy.ToolError("release artifact missing, expired or ambiguous")
        artifact = artifacts[0]
        data = request(settings, f"actions/artifacts/{artifact['id']}/zip", binary=True)
        if (
            data is None
            or artifact.get("digest") != "sha256:" + hashlib.sha256(data).hexdigest()
        ):
            raise _policy.ToolError("native GitHub artifact archive checksum differs")
        verify_archive(receipt, data, "")
        receipt["artifact"] = {key: artifact[key] for key in ["id", "name", "digest"]}
    else:
        run = document(settings, f"pipelines/{run_id}")
        if not isinstance(run, dict) or run.get("sha") != sha:
            raise _policy.ToolError("GitLab pipeline SHA differs")
        for job_id in [receipt["build_job"], receipt["validation"]["job"]]:
            job = document(settings, f"jobs/{job_id}")
            if (
                not isinstance(job, dict)
                or str(job.get("pipeline", {}).get("id")) != str(run_id)
                or job.get("commit", {}).get("id") != sha
                or job.get("status") != "success"
            ):
                raise _policy.ToolError(
                    "required GitLab job missing, failed, skipped, cancelled or mismatched"
                )
        if not require_artifact:
            return
        data = request(
            settings, f"jobs/{receipt['validation']['job']}/artifacts", binary=True
        )
        if data is None:
            raise _policy.ToolError("native GitLab release archive missing or expired")
        verify_archive(receipt, data, f"target/release-assets/{receipt['target']}/")
        receipt["artifact"] = {
            "job_id": receipt["validation"]["job"],
            "run_id": run_id,
            "archive_sha256": hashlib.sha256(data).hexdigest(),
        }


def verify_archive(receipt: dict, data: bytes, prefix: str) -> None:
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        original = json.loads(archive.read(prefix + "platform.json"))
        expected = {
            name: value for name, value in receipt.items() if name != "artifact"
        }
        if original != expected:
            raise _policy.ToolError("downloaded native release receipt differs")
        for asset in receipt["assets"]:
            value = archive.read(prefix + asset["name"])
            if (
                len(value) != asset["size"]
                or hashlib.sha256(value).hexdigest() != asset["sha256"]
            ):
                raise _policy.ToolError("downloaded native asset checksum differs")
        report = archive.read(prefix + "validation.json")
        if hashlib.sha256(report).hexdigest() != receipt["validation"]["sha256"]:
            raise _policy.ToolError("downloaded native test report checksum differs")


def route(
    directory: Path, settings: dict, source: str, destination: str, base_sha: str
) -> None:
    if (
        destination == "dev"
        and not source.startswith("release/")
        and source not in ["main", "dev"]
    ):
        return
    if destination == "dev" and (source == "main" or source.startswith("release/")):
        return
    if destination == "main" and source == f"release/{settings['tag'][1:]}":
        return
    if destination == "main" and source.startswith("fix/"):
        context = settings.get("hotfix", {})
        tag = context.get("tag", "")
        from .release import tag_version

        tag_version(tag)
        published = document(
            settings,
            "releases/tags/" + tag
            if settings["provider"] == "github"
            else "releases/" + tag,
        )
        if not published:
            raise _policy.ToolError("hotfix base must be an actual published release")
        tagged = _policy.execute(
            directory, ["git", "rev-parse", tag + "^{commit}"], capture=True
        ).strip()
        if tagged != context.get("sha"):
            raise _policy.ToolError("hotfix base tag SHA differs from declared context")
        remote_tag = document(
            settings,
            "git/ref/tags/" + tag
            if settings["provider"] == "github"
            else "repository/tags/" + tag,
        )
        remote_sha = (
            remote_tag.get("object", {}).get("sha")
            if settings["provider"] == "github"
            else remote_tag.get("commit", {}).get("id")
        )
        if remote_sha != tagged:
            raise _policy.ToolError("published hotfix tag differs from fetched tag")
        development = _policy.execute(
            directory,
            [
                "git",
                "rev-list",
                settings.get("remote", "origin") + "/dev",
                "^" + tagged,
            ],
            capture=True,
        ).splitlines()
        candidate = _policy.execute(
            directory, ["git", "rev-list", "HEAD", "^" + tagged], capture=True
        ).splitlines()
        if set(candidate) & set(development):
            raise _policy.ToolError(
                "hotfix contains unrelated integrated development; start from the published tag"
            )
        for descendant in [base_sha, "HEAD"]:
            _policy.execute(
                directory, ["git", "merge-base", "--is-ancestor", tagged, descendant]
            )
        return
    raise _policy.ToolError(
        "forbidden branch route; main requires release/<version> or verified hotfix context"
    )


def check_route(directory: Path, settings: dict) -> None:
    if settings["provider"] == "github":
        event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
        proposal = event.get("pull_request")
        if not isinstance(proposal, dict):
            if os.environ.get("GITHUB_REF") != "refs/heads/main":
                raise _policy.ToolError(
                    "release workflow must identify a PR or main push"
                )
            authorize(directory, settings, {"sha": os.environ["GITHUB_SHA"]})
            return
        route(
            directory,
            settings,
            proposal["head"]["ref"],
            proposal["base"]["ref"],
            proposal["base"]["sha"],
        )
    else:
        if os.environ.get("CI_PIPELINE_SOURCE") == "merge_request_event":
            route(
                directory,
                settings,
                os.environ["CI_MERGE_REQUEST_SOURCE_BRANCH_NAME"],
                os.environ["CI_MERGE_REQUEST_TARGET_BRANCH_NAME"],
                os.environ["CI_MERGE_REQUEST_DIFF_BASE_SHA"],
            )
        else:
            if (
                os.environ.get("CI_PIPELINE_SOURCE") != "push"
                or os.environ.get("CI_COMMIT_BRANCH") != "main"
                or os.environ.get("CI_COMMIT_REF_PROTECTED") != "true"
            ):
                raise _policy.ToolError(
                    "release route requires a protected main push or merge request"
                )
            actual = _policy.execute(
                directory, ["git", "rev-parse", "HEAD"], capture=True
            ).strip()
            if actual != os.environ.get("CI_COMMIT_SHA"):
                raise _policy.ToolError("native main push SHA differs from checkout")


def authorize(directory: Path, settings: dict, manifest: dict) -> None:
    sha = manifest["sha"]
    if settings["provider"] == "github":
        if (
            os.environ.get("GITHUB_EVENT_NAME") != "push"
            or os.environ.get("GITHUB_REF") != "refs/heads/main"
            or os.environ.get("GITHUB_SHA") != sha
        ):
            raise _policy.ToolError(
                "publication requires the protected main merge push"
            )
        branch = document(settings, "branches/main")
        if not isinstance(branch, dict) or not branch.get("protected"):
            raise _policy.ToolError("publication main protection cannot be verified")
        proposals = pages(settings, f"commits/{sha}/pulls")
        matches = [
            item
            for item in proposals
            if item.get("merged_at")
            and item.get("merge_commit_sha") == sha
            and item.get("base", {}).get("ref") == "main"
        ]
        if len(matches) != 1:
            raise _policy.ToolError(
                "main SHA must identify one approved merged release PR"
            )
        proposal = matches[0]
        route(
            directory,
            settings,
            proposal["head"]["ref"],
            "main",
            proposal["base"]["sha"],
        )
    else:
        if (
            os.environ.get("CI_PIPELINE_SOURCE") != "push"
            or os.environ.get("CI_COMMIT_BRANCH") != "main"
            or os.environ.get("CI_COMMIT_SHA") != sha
            or os.environ.get("CI_COMMIT_REF_PROTECTED") != "true"
        ):
            raise _policy.ToolError(
                "publication requires the protected main merge push"
            )
        proposals = pages(settings, f"repository/commits/{sha}/merge_requests")
        matches = [
            item
            for item in proposals
            if item.get("state") == "merged"
            and gitlab_merged_revision(item) == sha
            and item.get("target_branch") == "main"
            and not item.get("squash_commit_sha")
            and not item.get("squash")
        ]
        if len(matches) != 1:
            raise _policy.ToolError(
                "main SHA must identify one ancestry-preserving merged release MR"
            )
        proposal = document(settings, f"merge_requests/{matches[0]['iid']}")
        if (
            not proposal
            or gitlab_merged_revision(proposal) != sha
            or proposal.get("state") != "merged"
            or proposal.get("target_branch") != "main"
            or proposal.get("squash_commit_sha")
            or proposal.get("squash")
        ):
            raise _policy.ToolError("release MR authorization changed")
        route(
            directory,
            settings,
            proposal["source_branch"],
            "main",
            proposal["diff_refs"]["base_sha"],
        )
    # A merge authorizes publication; ancestry must survive promotion.
    source_sha = (
        proposal["head"]["sha"] if settings["provider"] == "github" else proposal["sha"]
    )
    _policy.execute(directory, ["git", "merge-base", "--is-ancestor", source_sha, sha])


def files(root: Path, manifest: dict) -> list[dict]:
    result = list(manifest["files"])
    for name in ["SHA256SUMS", "release.json"]:
        data = (root / name).read_bytes()
        result.append(
            {
                "name": name,
                "path": name,
                "sha256": hashlib.sha256(data).hexdigest(),
                "size": len(data),
            }
        )
    return result


def verify_download(settings: dict, endpoint: str, item: dict) -> bytes:
    data = request(settings, endpoint, binary=True)
    if (
        data is None
        or len(data) != item["size"]
        or hashlib.sha256(data).hexdigest() != item["sha256"]
    ):
        raise _policy.ToolError(f"remote release download differs: {item['name']}")
    return data


def published_state(root: Path, manifest: dict) -> dict:
    path = root / "published.json"
    previous = (
        json.loads(path.read_text())
        if path.exists()
        else {
            "sha": manifest["sha"],
            "tag": manifest["tag"],
            "files": {},
            "visible": False,
        }
    )
    if previous["sha"] != manifest["sha"] or previous["tag"] != manifest["tag"]:
        raise _policy.ToolError("partial publication belongs to a different release")
    return previous


def publish(directory: Path, settings: dict, root: Path, manifest: dict) -> None:
    state = published_state(root, manifest)
    tag = urllib.parse.quote(manifest["tag"], safe="")
    if settings["provider"] == "github":
        # List includes drafts visible to the authorized publishing token.
        releases = [
            item
            for item in pages(settings, "releases")
            if item["tag_name"] == manifest["tag"]
        ]
        if len(releases) > 1:
            raise _policy.ToolError("release tag identity ambiguous")
        remote = releases[0] if releases else None
        if remote and not remote["draft"]:
            raise _policy.ToolError(
                "published release cannot be replaced; use a new patch version"
            )
        if remote is None:
            remote = document(
                settings,
                "releases",
                "POST",
                {
                    "tag_name": manifest["tag"],
                    "target_commitish": manifest["sha"],
                    "name": manifest["tag"],
                    "body": _policy.inside(directory, settings["notes"]).read_text(),
                    "draft": True,
                    "prerelease": False,
                },
            )
        state["release_id"] = remote["id"]
        state["draft"] = True
        _policy.write_json(root / "published.json", state)
        available = pages(settings, f"releases/{remote['id']}/assets")
    else:
        if document(settings, "releases/" + tag):
            raise _policy.ToolError(
                "published release cannot be replaced; use a new patch version"
            )
        available = []
        verify_duplicate_policy(settings)
    for item in files(root, manifest):
        data = _policy.inside(root, item["path"]).read_bytes()
        if hashlib.sha256(data).hexdigest() != item["sha256"]:
            raise _policy.ToolError("local release bytes changed during publication")
        if settings["provider"] == "github":
            matches = [value for value in available if value["name"] == item["name"]]
            if len(matches) > 1:
                raise _policy.ToolError("duplicate draft asset")
            if matches:
                endpoint = f"releases/assets/{matches[0]['id']}"
                verify_download(settings, endpoint, item)
            else:
                endpoint = f"releases/{state['release_id']}/assets?name={urllib.parse.quote(item['name'], safe='')}"
                uploaded = request(
                    settings, endpoint, "POST", data, binary=True, upload=True
                )
                value = json.loads(uploaded or b"{}")
                endpoint = f"releases/assets/{value['id']}"
                verify_download(settings, endpoint, item)
            link = {"name": item["name"], "url": endpoint}
        else:
            package = urllib.parse.quote(
                settings.get("package", manifest["product"]), safe=""
            )
            endpoint = f"packages/generic/{package}/{tag}/{urllib.parse.quote(item['name'], safe='')}"
            existing = request(settings, endpoint, binary=True)
            if existing is not None and existing != data:
                raise _policy.ToolError(
                    "partial upload conflict; use a new patch version"
                )
            if existing is None:
                request(settings, endpoint, "PUT", data, binary=True)
            verify_download(settings, endpoint, item)
            base = os.environ.get(
                "CI_API_V4_URL", settings.get("api_url", "https://gitlab.com/api/v4")
            )
            project = settings.get("project_id", os.environ.get("CI_PROJECT_ID"))
            link = {
                "name": item["name"],
                "url": f"{base}/projects/{urllib.parse.quote(str(project), safe='')}/{endpoint}",
            }
        state["files"][item["name"]] = {
            "sha256": item["sha256"],
            "endpoint": endpoint,
            "link": link,
        }
        _policy.write_json(root / "published.json", state)


def finalize(directory: Path, settings: dict, root: Path, manifest: dict) -> None:
    if settings["provider"] == "gitlab":
        verify_duplicate_policy(settings)
    state = published_state(root, manifest)
    selected = files(root, manifest)
    if set(state["files"]) != {item["name"] for item in selected}:
        raise _policy.ToolError("partial publication: required files remain missing")
    for item in selected:
        previous = state["files"][item["name"]]
        if previous["sha256"] != item["sha256"]:
            raise _policy.ToolError("partial publication hash differs")
        verify_download(settings, previous["endpoint"], item)
    verify_signatures(directory, settings, root, selected)
    tag = urllib.parse.quote(manifest["tag"], safe="")
    if settings["provider"] == "github":
        existing_tag = document(settings, "git/ref/tags/" + tag)
        if (
            existing_tag
            and existing_tag.get("object", {}).get("sha") != manifest["sha"]
        ):
            raise _policy.ToolError("existing release tag differs")
        if existing_tag is None:
            document(
                settings,
                "git/refs",
                "POST",
                {"ref": "refs/tags/" + manifest["tag"], "sha": manifest["sha"]},
            )
        remote = document(settings, f"releases/{state['release_id']}")
        if not remote or not remote.get("draft"):
            raise _policy.ToolError(
                "release visibility already changed; verify manually"
            )
        document(settings, f"releases/{state['release_id']}", "PATCH", {"draft": False})
        remote = document(settings, "releases/tags/" + tag)
        if not remote or remote.get("draft") or remote.get("prerelease"):
            raise _policy.ToolError("published release visibility cannot be verified")
    else:
        existing_tag = document(settings, "repository/tags/" + tag)
        if existing_tag and existing_tag.get("commit", {}).get("id") != manifest["sha"]:
            raise _policy.ToolError("existing release tag differs")
        created = document(
            settings,
            "releases",
            "POST",
            {
                "tag_name": manifest["tag"],
                "ref": manifest["sha"],
                "name": manifest["tag"],
                "description": _policy.inside(directory, settings["notes"]).read_text(),
                "assets": {
                    "links": [value["link"] for value in state["files"].values()]
                },
            },
        )
        remote = document(settings, "releases/" + tag)
        tagged = document(settings, "repository/tags/" + tag)
        if (
            not isinstance(created, dict)
            or created.get("tag_name") != manifest["tag"]
            or not isinstance(created.get("commit"), dict)
            or created["commit"].get("id") != manifest["sha"]
            or not isinstance(remote, dict)
            or remote.get("tag_name") != manifest["tag"]
            or not isinstance(remote.get("commit"), dict)
            or remote["commit"].get("id") != manifest["sha"]
            or not isinstance(tagged, dict)
            or not isinstance(tagged.get("commit"), dict)
            or tagged["commit"].get("id") != manifest["sha"]
        ):
            raise _policy.ToolError("published release tag identity cannot be verified")
    for item in selected:
        verify_download(settings, state["files"][item["name"]]["endpoint"], item)
    state["visible"] = True
    _policy.write_json(root / "published.json", state)


def verify_publication(directory: Path, settings: dict) -> None:
    if settings["provider"] == "gitlab":
        verify_duplicate_policy(settings)
    tag = urllib.parse.quote(settings["tag"], safe="")
    if settings["provider"] == "github":
        remote = document(settings, "releases/tags/" + tag)
        if (
            not isinstance(remote, dict)
            or remote.get("draft")
            or remote.get("prerelease")
        ):
            raise _policy.ToolError("release is not published")
        downloads = {
            item["name"]: f"releases/assets/{item['id']}"
            for item in pages(settings, f"releases/{remote['id']}/assets")
        }
        data = request(settings, downloads["release.json"], binary=True)
        tagged = document(settings, "git/ref/tags/" + tag)
        sha = tagged["object"]["sha"]
    else:
        remote = document(settings, "releases/" + tag)
        if not remote:
            raise _policy.ToolError("release is not published")
        package = urllib.parse.quote(
            settings.get("package", settings["product"]), safe=""
        )
        prefix = f"packages/generic/{package}/{tag}/"
        data = request(settings, prefix + "release.json", binary=True)
        downloads = {
            item["name"]: prefix + urllib.parse.quote(item["name"], safe="")
            for item in remote["assets"]["links"]
        }
        sha = document(settings, "repository/tags/" + tag)["commit"]["id"]
    manifest = json.loads(data or b"{}")
    if (
        manifest.get("sha") != sha
        or manifest.get("tag") != settings["tag"]
        or set(manifest.get("targets", {})) != set(settings["targets"])
    ):
        raise _policy.ToolError(
            "remote release identity or target completeness differs"
        )
    for receipt in manifest["targets"].values():
        if (
            receipt.get("sha") != sha
            or receipt.get("validation", {}).get("outcome") != "passed"
        ):
            raise _policy.ToolError("remote release validation differs")
        verify_jobs(settings, receipt, require_artifact=False)
    with tempfile.TemporaryDirectory(prefix="release-verification-") as temporary:
        root = Path(temporary)
        for item in manifest["files"]:
            path = _policy.inside(root, item["name"])
            path.write_bytes(verify_download(settings, downloads[item["name"]], item))
        verified = [{**item, "path": item["name"]} for item in manifest["files"]]
        verify_signatures(directory, settings, root, verified)
    sums = request(settings, downloads["SHA256SUMS"], binary=True)
    expected = "".join(
        f"{item['sha256']}  {item['name']}\n" for item in manifest["files"]
    ).encode()
    expected += f"{hashlib.sha256(data or b'').hexdigest()}  release.json\n".encode()
    if sums != expected:
        raise _policy.ToolError("remote checksum catalog differs")
    _policy.write_json(
        directory / ".stkr/state/release-publication.json",
        {"sha": sha, "tag": settings["tag"], "verified": True},
    )


def verify_signatures(
    directory: Path, settings: dict, root: Path, selected: list
) -> None:
    # Normalize Final Downloads For Product-Owned Signature Commands.
    with tempfile.TemporaryDirectory(prefix="release-signatures-") as temporary:
        assets = Path(temporary)
        for item in selected:
            source = _policy.inside(root, item.get("path", item["name"]))
            destination = _policy.inside(assets, item["name"])
            destination.write_bytes(source.read_bytes())
        previous = os.environ.get("RELEASE_ASSET_ROOT")
        os.environ["RELEASE_ASSET_ROOT"] = str(assets)
        try:
            for command in settings.get("signature_checks", []):
                _policy.execute(
                    directory, _policy.command(command, "release.signature_checks")
                )
        finally:
            if previous is None:
                os.environ.pop("RELEASE_ASSET_ROOT", None)
            else:
                os.environ["RELEASE_ASSET_ROOT"] = previous


def gitlab_merged_revision(proposal: dict) -> str | None:
    """A Fast-Forward MR Has No Separate Merge Commit"""
    merge_revision = proposal.get("merge_commit_sha")
    return proposal.get("sha") if merge_revision is None else merge_revision


# === Private Functions === #

# === Entry Point === #
