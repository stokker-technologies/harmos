# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Release Identity."""

# === Imports === #

# Standard Library Imports
import os
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.release.source

from . import registry

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(directory: Path, settings: dict) -> None:
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
    for registry_name, actual, expected in [
        ("crates.io", caller, harmos_tools.release.source.RELEASE_WORKFLOW),
        ("PyPI", called, harmos_tools.release.source.RELEASE_WORKFLOW),
    ]:
        if actual != expected:
            print(f"warning: {registry_name} sees workflow {actual}, but the rendered guidance names {expected}")


# === Private Functions === #

# === Entry Point === #
