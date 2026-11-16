# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Run Release Selections."""

# === Imports === #

# Standard Library Imports
import argparse
import os
from pathlib import Path

# Third Party Imports
# Local Imports
from . import execute

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(options: argparse.Namespace, parser: argparse.ArgumentParser) -> int:
    root = Path.cwd()
    operation = options.command.partition(":")[2]
    if operation == "publish" and options.registry:
        if options.channel != "stable" or options.stage:
            parser.error("--registry pypi requires stable publication without --stage")
        if not options.dry and os.environ.get("GITHUB_ACTIONS"):
            execute.module("release.checks")
            execute.module("release.run", "identity")
        return execute.module("release.run", "python-dry" if options.dry else "python")
    if operation in ("publish", "verify") and options.channel == "nightly":
        import harmos_tools.release.policy

        from ..release import nightly

        if operation == "verify":
            nightly.verify(root)
        elif options.dry:
            import harmos_tools.release.rehearse

            harmos_tools.release.rehearse.dry(
                root, harmos_tools.release.policy.policy(root, "release"), profile="release", require_all=True, channel="nightly"
            )
        elif options.stage:
            parser.error("nightly publication has one complete publish entry point")
        else:
            nightly.publish(root)
        return 0
    if operation == "publish":
        if not options.dry and options.stage in (None, "assets") and os.environ.get("GITHUB_ACTIONS"):
            execute.module("release.checks")
            execute.module("release.run", "identity")
        if options.dry:
            return execute.task("release:publish:dry", *options.arguments)
        if options.stage:
            return execute.task(f"release:publish:{options.stage}", *options.arguments)
        execute.task("release:publish:assets")
        return execute.task("release:publish:finalize")
    if operation == "prepare" and options.review:
        execute.module("release.run", "prepare")
        if os.environ.get("GITHUB_RUNNER_WORKFLOWS"):
            previous = os.environ.get("GH_TOKEN")
            os.environ["GH_TOKEN"] = os.environ.get("RUNNER_GROUP_READ_TOKEN", previous or "")
            try:
                execute.module("test.runners")
            finally:
                if previous is None:
                    os.environ.pop("GH_TOKEN", None)
                else:
                    os.environ["GH_TOKEN"] = previous
        operation = "review"
    if operation == "tag" and os.environ.get("GITHUB_ACTIONS"):
        execute.module("release.run", "identity")
        execute.module("release.checks")
    extra = options.arguments[1:] if options.arguments[:1] == ["--"] else options.arguments
    if operation == "rehearse":
        if options.target:
            extra += ["--target", options.target]
        if options.profile:
            extra += ["--profile", options.profile]
        if options.stage_only:
            extra += ["--stage-only"]
    return execute.module("release.run", operation, *extra)


# === Private Functions === #

# === Entry Point === #
