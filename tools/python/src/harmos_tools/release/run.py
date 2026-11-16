# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Run Explicit Release Operations."""

# === Imports === #

# Standard Library Imports
import argparse
import json
import sys
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.release.crates
import harmos_tools.release.python_registry

from . import assets, finalize, identity, open_review, policy, prepare, publish, registry, rehearse, source, tag, verify

# Type Checking Imports

# === Public Exports === #

# === Constants === #

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
    "python-dry",
    "finalize",
    "verify",
    "guard",
    "assets",
    "stage",
]


# === Types === #

# === Classes === #

# === Public Functions === #


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
    match verb:
        case "prepare":
            prepare.run(directory, settings)
        case "review":
            open_review.run(directory, settings)
        case "dry":
            rehearse.dry(directory, settings)
        case "rehearse":
            rehearse.run(directory, settings, target=target, profile=profile or "release", stage_only=stage_only)
        case "identity":
            identity.run(directory, settings)
        case "crates-dry":
            harmos_tools.release.crates.rehearse(directory, skip_published=False)
        case "tag":
            tag.run(directory, settings)
        case "crates":
            harmos_tools.release.crates.publish(directory)
        case "publish":
            publish.run(directory, settings)
        case "python":
            harmos_tools.release.python_registry.publish(directory, settings, dry_run=False)
        case "python-dry":
            harmos_tools.release.python_registry.publish(directory, settings, dry_run=True)
        case "finalize":
            finalize.run(directory, settings)
        case "verify":
            verify.run(directory, settings)
        case "guard":
            registry.require_tag(directory, source.release_tag(directory))
        case "assets":
            inventory = assets.inventory(directory, settings, source.native_target(settings), source.release_tag(directory))
            print(json.dumps(inventory, indent=2))
        case "stage":
            assets.stage(directory, settings, source.release_tag(directory), settings["targets"])
        case _:
            raise policy.ToolError(f"Select a supported release operation: {verb}")


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
