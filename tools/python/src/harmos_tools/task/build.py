# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Run the Build Selection."""

# === Imports === #

# Standard Library Imports
import argparse
from pathlib import Path

# Third Party Imports
# Local Imports
from ..build import cargo
from . import cpp, csharp, execute

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(options: argparse.Namespace, parser: argparse.ArgumentParser) -> int:
    root = Path.cwd()

    focused_fixtures = (
        options.focused_messages,
        options.libraries,
        options.inspection_messages,
        options.inspections,
        options.componentizer,
    )

    if any(value is not None for value in focused_fixtures) and (options.artifact != "fixtures" or options.package):
        parser.error("focused fixture selectors require --artifact fixtures")

    if options.lib:
        if not options.package or options.package == "harmos-sdk" or options.example or options.language not in (None, "rust"):
            parser.error("--lib requires a Rust --package without --example")

    if options.language == "csharp":
        return csharp.run(options, parser)

    if options.language == "cpp":
        return cpp.run(options, parser)

    if options.manifest_path and not options.package:
        parser.error("--manifest-path requires --package")

    if options.features and not options.package:
        parser.error("--features requires --package")

    if options.package:
        if options.artifact != "product" or options.compatibility or options.extended:
            parser.error("--package cannot select artifacts, compatibility, C++ or extended builds")
        if options.package == "harmos-sdk":
            if options.features or options.example:
                parser.error("Python SDK builds have no Cargo features or examples")
            if options.check:
                return execute.task("check:python:sdk")
            return execute.command(["mise", "-C", "sdk/python", "run", "build", "--", "--profile", options.profile])
        command = ["--locked", "--profile", options.profile, "-p", options.package]
        if options.manifest_path:
            command += ["--manifest-path", str(options.manifest_path)]
        if options.features:
            command += ["--features", options.features]
        if options.no_default_features:
            command += ["--no-default-features"]
        if options.example:
            command += ["--example", options.example]
        if options.lib:
            command += ["--lib"]
        if options.target:
            command += ["--target", options.target]
        if options.messages is not None:
            return cargo.focused(root, "check" if options.check else "build", command, messages=options.messages)
        return cargo.focused(root, "check" if options.check else "build", command)

    if options.example:
        parser.error("--example requires --package")

    if options.check:
        parser.error("--check requires --package")

    if options.artifact == "fixtures":
        if options.compatibility or options.extended:
            parser.error("--artifact fixtures cannot select C++, compatibility or extended builds")
        selected = ["--profile", options.profile]
        for name in ("focused_messages", "libraries", "inspection_messages", "inspections", "componentizer"):
            value = getattr(options, name)
            if value is not None:
                selected += ["--" + name.replace("_", "-"), str(value)]
        return execute.task("prepare:tests:fixtures", *selected)

    if options.compatibility:
        return execute.task("build:rust:compatibility", "--profile", options.profile)

    if options.artifact == "bench":
        return execute.task("build:rust:bench", *(["--extended"] if options.extended else []))

    if options.artifact == "tests":
        if options.extended:
            return execute.module("build.rust.tests", "--extended", "--profile", options.profile)
        return execute.task("build:rust:tests", "--profile", options.profile)

    if options.extended:
        parser.error("--extended requires --artifact tests or bench")

    return execute.task("build:product", "--profile", options.profile)


# === Private Functions === #

# === Entry Point === #
