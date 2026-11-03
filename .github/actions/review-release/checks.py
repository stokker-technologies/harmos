"""Require Green CI For The Exact Release Commit Before Any Publication."""

# === Imports === #

# Standard Library Imports
import os
import re
import runpy
import time
from pathlib import Path

# Third Party Imports
# Local Imports
# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def green(sha: str, pages) -> bool:
    """Select The Latest Dev Push And Its Required Gate, Never A Prior Run Or PR."""
    if re.fullmatch(r"[0-9a-f]{40}(?:[0-9a-f]{24})?", sha) is None:
        raise ValueError("invalid target revision")
    runs = pages(f"actions/workflows/ci.yml/runs?branch=dev&event=push&head_sha={sha}", "workflow_runs")
    runs = [run for run in runs if run.get("head_sha") == sha and run.get("event") == "push" and run.get("head_branch") == "dev"]
    if not runs:
        return False
    run = max(runs, key=lambda item: item["id"])
    if run.get("status") != "completed":
        return False
    if run.get("conclusion") != "success":
        raise ValueError("target commit CI failed")

    checks = pages(f"commits/{sha}/check-runs?filter=latest", "check_runs")
    gates = [
        check for check in checks
        if check.get("name") == "CI / Required"
        and check.get("app", {}).get("slug") == "github-actions"
        and check.get("check_suite", {}).get("id") == run["check_suite_id"]
    ]
    if len(gates) != 1 or gates[0].get("status") != "completed":
        return False
    if gates[0].get("conclusion") != "success":
        raise ValueError("target commit required gate failed")

    # Honor Additional Required Contexts In The Owner's Main Ruleset.
    rules = pages("rules/branches/main")
    required = [context for rule in rules if rule.get("type") == "required_status_checks"
                for context in rule["parameters"]["required_status_checks"]]
    statuses = pages(f"commits/{sha}/statuses")
    for context in required:
        matches = [check for check in checks if check.get("name") == context["context"]
                   and (not context.get("integration_id") or check.get("app", {}).get("id") == context["integration_id"])]
        if matches:
            check = max(matches, key=lambda item: item["id"])
            if check.get("status") != "completed":
                return False
            if check.get("conclusion") != "success":
                raise ValueError("target commit required check failed: " + context["context"])
            continue
        legacy = [status for status in statuses if status.get("context") == context["context"] and not context.get("integration_id")]
        if not legacy:
            return False
        status = max(legacy, key=lambda item: item["id"])
        if status.get("state") == "pending":
            return False
        if status.get("state") != "success":
            raise ValueError("target commit required status failed: " + context["context"])
    return True


def wait(sha: str, pages, *, sleep=time.sleep, clock=time.monotonic) -> None:
    deadline = clock() + 1800
    while not green(sha, pages):
        if clock() >= deadline:
            raise ValueError("target commit required CI is missing or pending; rerun Release after CI succeeds")
        print("Waiting For Target Commit CI", flush=True)
        sleep(10)
    print(f"Required CI Is Green For {sha}", flush=True)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    review = runpy.run_path(str(Path(__file__).with_name("run.py")))
    sha = review["command"]("git", "rev-parse", "HEAD")
    expected = os.environ["RELEASE_TARGET_SHA"]
    if sha != expected:
        raise ValueError("release checkout differs from target revision")
    wait(sha, review["pages"])
