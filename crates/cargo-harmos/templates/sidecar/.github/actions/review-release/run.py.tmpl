# Rendered by stkr project update from templates/ci/review-release; sha256:a3c15316079b71dcab1d9dba8279b21c053f4799428033a62a49cb0998416977
"""Find A Green Review Of This Exact Tree And Restore Its Package Artifacts."""

# === Imports === #

# Standard Library Imports
import io
import json
import os
import re
import runpy
import subprocess
import urllib.parse
import urllib.request
import zipfile
from datetime import UTC, datetime
from pathlib import Path

# Third Party Imports
# Local Imports
# Type Checking Imports

# === Public Exports === #

# === Constants === #

spawn_flags = runpy.run_path(str(Path(__file__).resolve().parents[1] / "process.py"))["spawn_flags"]

OID = re.compile(r"[0-9a-f]{40}(?:[0-9a-f]{24})?")

# === Types === #

# === Classes === #


class ProviderRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        redirected = super().redirect_request(req, fp, code, msg, headers, newurl)
        origin = urllib.parse.urlsplit(req.full_url)
        destination = urllib.parse.urlsplit(newurl)
        if destination.scheme != "https":
            raise ValueError("insecure artifact redirect")
        if redirected is not None and (origin.scheme, origin.netloc) != (destination.scheme, destination.netloc):
            redirected.remove_header("PRIVATE-TOKEN")
        return redirected


# === Public Functions === #


def command(*args):
    return subprocess.run(args, check=True, capture_output=True, text=True, timeout=120, creationflags=spawn_flags()).stdout.strip()


def request(endpoint, *, binary=False):
    if os.environ.get("GITHUB_ACTIONS"):
        return json.loads(command("gh", "api", f"repos/{os.environ['GITHUB_REPOSITORY']}/{endpoint}"))
    token = os.environ["RELEASE_TOKEN"]
    if not token:
        raise ValueError("provider token unavailable")
    base = os.environ["CI_API_V4_URL"]
    project = urllib.parse.quote(os.environ["CI_PROJECT_ID"], safe="")
    native = urllib.request.Request(f"{base}/projects/{project}/{endpoint}", headers={"PRIVATE-TOKEN": token})
    with urllib.request.build_opener(ProviderRedirect()).open(native, timeout=60) as response:
        data = response.read()
    return data if binary else json.loads(data)


def pages(endpoint, key=None):
    values = []
    for page in range(1, 101):
        separator = "&" if "?" in endpoint else "?"
        response = request(f"{endpoint}{separator}per_page=100&page={page}")
        batch = response[key] if key else response
        if not isinstance(batch, list):
            raise TypeError("invalid provider page")
        values.extend(batch)
        if len(batch) < 100:
            return values
    raise ValueError("provider pagination limit")


def same_tree(head, tree):
    if not isinstance(head, str) or OID.fullmatch(head) is None:
        return False
    command("git", "fetch", "--quiet", "--no-tags", "--no-write-fetch-head", "origin", head)
    return command("git", "rev-parse", "--verify", head + "^{tree}") == tree


def github_review_run(run, review):
    if run.get("head_sha") != review["head"]["sha"]:
        return False
    associations = run.get("pull_requests")
    if associations:
        return any(pr.get("number") == review["number"] for pr in associations)
    # GitHub Can Drop The Association After The Merged Source Branch Is Deleted.
    repository = review["head"].get("repo", {}).get("id")
    return (
        associations == []
        and isinstance(repository, int)
        and repository > 0
        and run.get("event") == "pull_request"
        and run.get("head_branch") == review["head"]["ref"]
        and run.get("head_repository", {}).get("id") == repository
        and run.get("repository", {}).get("id") == review.get("base", {}).get("repo", {}).get("id")
        and isinstance(run.get("repository", {}).get("id"), int)
    )


def lookup():
    sha = command("git", "rev-parse", "HEAD")
    tree = command("git", "rev-parse", "HEAD^{tree}")
    if OID.fullmatch(tree) is None:
        raise ValueError("invalid checkout tree")
    github = bool(os.environ.get("GITHUB_ACTIONS"))
    if github:
        reviews = [
            review
            for review in pages(f"commits/{sha}/pulls")
            if review.get("merged_at") and review.get("merge_commit_sha") == sha and review.get("base", {}).get("ref") == "dev"
        ]
    else:
        reviews = [
            review
            for review in pages(f"repository/commits/{sha}/merge_requests")
            if review.get("state") == "merged"
            and sha in (review.get("merge_commit_sha"), review.get("squash_commit_sha"))
            and review.get("target_branch") == "dev"
        ]
    if len(reviews) != 1:
        raise ValueError("no unambiguous merged review")
    review = reviews[0]
    head = review["head"]["sha"] if github else review["sha"]
    branch = review["head"]["ref"] if github else review["source_branch"]
    if not same_tree(head, tree):
        raise ValueError("review tree differs")
    profile = "release" if branch.startswith("release/v") and os.environ.get("REVIEW_RELEASE") == "true" else "dev"
    tag = os.environ.get("GITHUB_REF", "").startswith("refs/tags/") or bool(os.environ.get("CI_COMMIT_TAG"))
    if tag and profile != "release":
        raise ValueError("tag requires release-profile evidence")
    required = json.loads(os.environ.get("REVIEW_PACKAGES", "[]"))
    preparation = os.environ.get("GITHUB_EVENT_NAME") == "workflow_dispatch" if github else os.environ.get("CI_PIPELINE_SOURCE") == "web"
    if tag or preparation:
        required += json.loads(os.environ.get("REVIEW_BUILDS", "[]"))
    if github:
        runs = pages(f"actions/workflows/ci.yml/runs?event=pull_request&head_sha={head}", "workflow_runs")
        runs = [run for run in runs if github_review_run(run, review)]
        run = max(runs, key=lambda item: item["id"])
        if run.get("status") != "completed" or run.get("conclusion") != "success":
            raise ValueError("review run is not green")
        jobs = pages(f"actions/runs/{run['id']}/jobs?filter=latest", "jobs")
        if not jobs or any(job.get("conclusion") not in ("success", "skipped") for job in jobs):
            raise ValueError("review jobs are not green")
        artifacts = pages(f"actions/runs/{run['id']}/artifacts", "artifacts")
        for name in required:
            matches = [artifact for artifact in artifacts if artifact.get("name") == name.replace("PROFILE", profile)]
            if len(matches) != 1 or matches[0].get("expired") is not False:
                raise ValueError("review package unavailable")
    else:
        review = request(f"merge_requests/{review['iid']}")
        run = review["head_pipeline"]
        if run.get("sha") != head or run.get("status") != "success" or run.get("source") != "merge_request_event":
            raise ValueError("review pipeline is not green")
        jobs = pages(f"pipelines/{run['id']}/jobs?include_retried=false")
        if any(job.get("name") == "Plan / Impact" for job in jobs):
            bridges = [bridge for bridge in pages(f"pipelines/{run['id']}/bridges") if bridge.get("name") == "CI / Required"]
            if len(bridges) != 1 or bridges[0].get("status") != "success":
                raise ValueError("review child gate is not green")
            child = bridges[0].get("downstream_pipeline")
            if (
                not isinstance(child, dict)
                or child.get("sha") != head
                or child.get("status") != "success"
                or not isinstance(child.get("id"), int)
            ):
                raise ValueError("review child pipeline is not green for this exact revision")
            run = child
            jobs = pages(f"pipelines/{run['id']}/jobs?include_retried=false")
        if not jobs or any(job.get("status") not in ("success", "skipped", "manual") for job in jobs):
            raise ValueError("review jobs are not green")
        for name in required:
            matches = [job for job in jobs if job.get("name") == name]
            if len(matches) != 1 or matches[0].get("status") != "success" or not matches[0].get("artifacts_file", {}).get("filename"):
                raise ValueError("review package unavailable")
            expiry = matches[0].get("artifacts_expire_at")
            if expiry and datetime.fromisoformat(expiry) <= datetime.now(UTC):
                raise ValueError("review package expired")
    if not isinstance(run["id"], int) or run["id"] <= 0:
        raise ValueError("invalid review run ID")
    if tag and not any(
        job.get("name", "").endswith("Release / Rehearsal") and job.get("conclusion" if github else "status") == "success" for job in jobs
    ):
        raise ValueError("release review has no successful rehearsal")
    protected = os.environ.get("GITHUB_REF_PROTECTED") == "true" or os.environ.get("CI_COMMIT_REF_PROTECTED") == "true"
    if tag and not protected:
        raise ValueError("release tag must be protected")
    version = branch.removeprefix("release/") if profile == "release" else f"dev.{run['id']}+{head[:8]}"
    if re.fullmatch(r"(?:v\d+\.\d+\.\d+|dev\.\d+\+[0-9a-f]{8})", version) is None:
        raise ValueError("invalid review version")
    return {
        "validated": "true",
        "review_run": str(run["id"]),
        "review_profile": profile,
        "review_sha": head,
        "review_version": version,
        "stable_release": str(bool(tag and protected)).lower(),
    }


def restore():
    if os.environ.get("validated") != "true":
        return
    run = os.environ["review_run"]
    if not run.isdecimal():
        raise ValueError("invalid review pipeline")
    jobs = pages(f"pipelines/{run}/jobs?include_retried=false")
    for name in json.loads(os.environ["REVIEW_PACKAGES"]) + json.loads(os.environ.get("REVIEW_BUILDS", "[]")):
        matches = [job for job in jobs if job.get("name") == name and job.get("status") == "success"]
        if len(matches) != 1:
            raise ValueError("review package producer unavailable")
        data = request(f"jobs/{matches[0]['id']}/artifacts", binary=True)
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            for member in archive.infolist():
                path = Path(member.filename)
                parent = member.is_dir() and member.filename in ("target/", "target/package/")
                if (
                    path.is_absolute()
                    or ".." in path.parts
                    or "\\" in member.orig_filename
                    or (member.external_attr >> 16) & 0o170000 == 0o120000
                ):
                    raise ValueError("unsafe package archive path")
                if not parent and not member.filename.startswith(("target/package/", "target/ci/")):
                    raise ValueError("unexpected package archive path")
            archive.extractall()
    print(f"Reusing packages from review pipeline {run}")


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    print("::group::Review / Release", flush=True)
    try:
        if os.environ.get("REVIEW_RESTORE") == "true":
            restore()
        else:
            result = {
                "validated": "false",
                "review_run": "",
                "review_profile": "",
                "review_sha": "",
                "review_version": "",
                "stable_release": "false",
            }
            try:
                result = lookup()
            except Exception as error:
                if os.environ.get("REVIEW_REQUIRED") == "true":
                    raise ValueError("required release review evidence unavailable") from error
                # Never print provider responses, commands or credentials on lookup failure.
                print(f"Review evidence unavailable ({type(error).__name__}); running the full pipeline")
            if not os.environ.get("GITHUB_ACTIONS"):
                version = (
                    os.environ.get("CI_COMMIT_TAG")
                    or f"dev.{os.environ.get('CI_PIPELINE_ID', '')}+{os.environ.get('CI_COMMIT_SHORT_SHA', '')}"
                )
                result["DEPLOY_EXPECTED_VERSION"] = result.get("review_version") or version
            output = os.environ.get("GITHUB_OUTPUT", "target/ci/validated.env")
            Path(output).parent.mkdir(parents=True, exist_ok=True)
            with open(output, "a", encoding="utf-8", newline="\n") as stream:
                stream.write("".join(f"{key}={value}\n" for key, value in result.items()))
            print(f"validated={result['validated']} review_run={result['review_run']}")
    finally:
        print("::endgroup::", flush=True)
