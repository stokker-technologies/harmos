# Rendered by stkr project update from templates/python/test/runners.py; sha256:09b95e3d9f5ccdc85f659ade97907fd82d90da6345f48c1c97c06673ce46ba95
# ***
#
# __PRODUCT_NAME__
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Check Selected-Workflow Access Before Preparing A Release."""

# === Imports === #

# Standard Library Imports
import json
import os
import subprocess

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def permitted(group: dict, repository: str, workflow: str, reference: str) -> bool:
    return not group.get("restricted_to_workflows", False) or f"{repository}/.github/workflows/{workflow}@{reference}" in group.get(
        "selected_workflows", []
    )


def main() -> None:
    repository = os.environ["GITHUB_REPOSITORY"]
    organization = repository.split("/")[0]
    workflows = json.loads(os.environ["GITHUB_RUNNER_WORKFLOWS"])
    groups = [group for page in _pages(f"orgs/{organization}/actions/runner-groups") for group in page["runner_groups"]]
    references = [os.environ["GITHUB_REF"], "refs/heads/release/v" + os.environ["RELEASE_VERSION"]]
    for workflow, requirements in workflows.items():
        for required in requirements:
            eligible = []
            for group in groups:
                runners = [
                    runner
                    for page in _pages(f"orgs/{organization}/actions/runner-groups/{group['id']}/runners")
                    for runner in page["runners"]
                ]
                if any(set(required).issubset({label["name"] for label in runner["labels"]}) for runner in runners):
                    eligible.append(group)
            for reference in references:
                if not any(permitted(group, repository, workflow, reference) for group in eligible):
                    raise RuntimeError(f"runner group allowlist does not permit {workflow}@{reference} with labels {required}")


# === Private Functions === #


def _pages(endpoint: str) -> list[dict]:
    return json.loads(subprocess.check_output(["gh", "api", "--hostname", "github.com", "--paginate", "--slurp", endpoint]))


# === Entry Point === #

if __name__ == "__main__":
    main()
