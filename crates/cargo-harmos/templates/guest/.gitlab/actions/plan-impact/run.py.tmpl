"""Select Conservative Native CI Jobs And Emit A GitLab Child Pipeline."""

# === Imports === #

# Standard Library Imports
import json
import os
import re
import runpy
import subprocess
from pathlib import Path

# Third Party Imports
# Local Imports
spawn_flags = runpy.run_path(str(Path(__file__).resolve().parents[1] / 'process.py'))['spawn_flags']

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def changed_paths(target: str, root: Path | None = None) -> set[str] | None:
    """Include Deleted, Added, Dirty And Untracked Paths Against One Merge Base."""

    def git(*arguments: str) -> str:
        return subprocess.check_output(
            ["git", *arguments],
            cwd=root,
            creationflags=spawn_flags(),
            stderr=subprocess.DEVNULL,
            timeout=30,
        ).decode("utf-8")

    try:
        if os.environ.get("GITLAB_CI") == "true" and target.startswith("origin/"):
            branch = target.removeprefix("origin/")
            git("check-ref-format", "--branch", branch)
            git(
                "fetch",
                "--no-tags",
                "origin",
                f"refs/heads/{branch}:refs/remotes/origin/{branch}",
            )
        revision = git("rev-parse", "--verify", "--end-of-options", target + "^{commit}").strip()
        bases = git("merge-base", "--all", revision, "HEAD").splitlines()
        if len(bases) != 1:
            return None
        diff = git("diff", "--name-only", "--no-renames", "-z", bases[0], "--")
        untracked = git("ls-files", "--others", "--exclude-standard", "-z")
        return set(filter(None, diff.split("\0") + untracked.split("\0")))
    except (OSError, UnicodeError, subprocess.SubprocessError):
        return None


def select(plan: dict, paths: set[str] | None) -> set[str]:
    """Match Every Changed Path And Preserve The Complete Dependency Closure."""
    graph = plan["dependencies"]
    full = set(graph)
    if not paths:
        return full
    selected = set()
    for path in paths:
        matching = [rule for rule in plan["rules"] if _matches(rule["include"], path) and not _matches(rule["exclude"], path)]
        if not matching:
            return full
        for rule in matching:
            selected.update(rule["jobs"])
    pending = list(selected)
    while pending:
        job = pending.pop()
        if job not in graph:
            return full
        for dependency in graph[job]:
            if dependency not in selected:
                selected.add(dependency)
                pending.append(dependency)
    return selected


def selection(root: Path, plan: dict, target: str = "origin/dev") -> set[str]:
    """Resolve Portable Local Impact From The Project's Changes."""
    return select(plan, changed_paths(target, root))


def main(plan: dict) -> None:
    """Native Providers Supply The Target Context; Missing Context Selects Full Proof."""
    target = os.environ.get("IMPACT_TARGET")
    paths = changed_paths(target) if target else None
    selected = select(plan, paths)
    if "document" in plan:
        document = plan["document"]
        original = [job for job in document.values() if isinstance(job, dict) and "stage" in job]
        routing = next((job for job in original if job["stage"] == "Check"), original[0] if original else {})
        # Only Native Protected Non-MR Refs May Receive Shared Cache Configuration.
        protected = os.environ.get("CI_COMMIT_REF_PROTECTED") == "true"
        source = os.environ.get("CI_PIPELINE_SOURCE")
        branch = os.environ.get("CI_COMMIT_BRANCH")
        tag = os.environ.get("CI_COMMIT_TAG", "")
        stable = re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", tag) is not None
        if protected and source != "merge_request_event" and (branch in ("dev", "main") or stable):
            for name, fields in plan.get("trusted_cache", {}).items():
                document[name].update(fields)
        for name in list(document):
            if isinstance(document[name], dict) and "stage" in document[name] and name not in selected:
                del document[name]
        if not selected:
            gate = {
                "stage": "Check",
                "before_script": [],
                "after_script": [],
                "cache": [],
                "needs": [],
                "script": ['echo "No CI Jobs Selected"'],
            }
            if tags := routing.get("tags"):
                gate["tags"] = tags
            if image := routing.get("image", document.get("default", {}).get("image")):
                gate["image"] = image
            document["CI / No Jobs"] = gate
        # Original Event Rules Remain Meaningful Inside The Native Child Pipeline.
        text = json.dumps(document).replace("CI_PIPELINE_SOURCE", "PIPELINE_SOURCE")
        destination = Path("target/ci/impact-child.yml")
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(text + "\n", encoding="utf-8")
    else:
        result = json.dumps(sorted(selected), separators=(",", ":"))
        if output := os.environ.get("GITHUB_OUTPUT"):
            with Path(output).open("a", encoding="utf-8") as stream:
                stream.write(f"jobs={result}\n")
        print(result)


# === Private Functions === #


def _matches(patterns: list[str], path: str) -> bool:
    # Compile The Glob Dialect Once For Both Local And Provider Resolution.
    return any(re.fullmatch(pattern.encode("utf-8"), path.encode("utf-8")) is not None for pattern in patterns)


# === Entry Point === #

if __name__ == '__main__':
    print('::group::Plan / Impact', flush=True)
    try:
        main(json.loads(os.environ.get('IMPACT_PLAN') or bytes.fromhex(__import__('sys').argv[2]).decode('utf-8')))
    finally:
        print('::endgroup::', flush=True)
