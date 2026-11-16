# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Run the Test Selection."""

# === Imports === #

# Standard Library Imports
import argparse
import os
import sys
from pathlib import Path

# Third Party Imports
# Local Imports
from ..build import cargo
from . import arguments, cpp, csharp, execute

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(options: argparse.Namespace, parser: argparse.ArgumentParser) -> int:
    root = Path.cwd()

    for selector in ("package", "target", "story", "exact"):
        value = getattr(options, selector)
        if value is not None and not value:
            parser.error(f"--{selector} requires a nonempty value")

    if (
        options.paths
        or options.story
        or options.no_run
        or options.timings
        or (options.package and not options.target and options.language != "python")
    ):
        from ..test import focused
        from ..test.policy import TestError

        if options.language not in (None, "rust") or options.coverage or options.extended or options.profile != "dev":
            parser.error("focused Rust tests use the default test profile without coverage or extended selection")
        if options.layer or options.filter or options.stress_count:
            parser.error("focused Rust tests select --target, --story or --exact")
        if options.paths and (options.package or options.target or options.manifest_path):
            parser.error("--paths selects packages and targets; use --manifest-path with --package instead")
        if not options.paths and not options.package:
            parser.error("focused Rust tests require --package or --paths")
        if options.exact and options.story:
            parser.error("select either --exact or --story")
        if options.paths and (options.exact or options.story):
            parser.error("--paths selects affected suites; use --package and --target to filter a story")
        try:
            return focused.run(
                root,
                package=options.package,
                target=options.target,
                paths=options.paths or (),
                manifest=options.manifest_path,
                features=options.features,
                exact=options.exact,
                story=options.story,
                no_run=options.no_run,
                timings=options.timings,
            )
        except TestError as error:
            parser.error(str(error))

    if options.language == "csharp":
        return csharp.run(options, parser)

    if options.language == "cpp":
        return cpp.run(options, parser)

    if options.manifest_path and not options.package:
        parser.error("--manifest-path requires --package")

    if options.features and not options.package:
        parser.error("--features requires --package")

    if options.language == "python" and options.package == "harmos-sdk":
        if (
            options.target not in (None, "integration")
            or options.coverage
            or options.extended
            or options.features
            or options.layer
            or options.filter
            or options.stress_count
        ):
            parser.error("Python SDK tests select --target integration and optional --exact")
        selected = ["--profile", options.profile]
        if options.exact:
            selected += ["--exact", options.exact]
        environment = dict(os.environ)
        environment.pop("VIRTUAL_ENV", None)
        return execute.command(["mise", "-C", "sdk/python", "run", "test", "--", *selected], environment=environment)

    if options.language == "python" and options.exact:
        if options.package or options.target or options.coverage or options.extended or options.features:
            parser.error("focused Python tests select only --exact")
        return execute.command([sys.executable, "-m", "unittest", options.exact], directory=root / "tools/python")

    if options.package:
        if not options.target:
            parser.error("--package requires --target lib or a test target name")
        if options.language not in (None, "rust") or options.coverage or options.extended:
            parser.error("focused tests require Rust without coverage, C++ or extended selection")
        if options.profile != "dev" or options.layer or options.filter or options.stress_count:
            parser.error("focused tests use the default test profile and --exact for filtering")
        selected = ["--lib"] if options.target == "lib" else ["--test", options.target]
        command = ["--locked", "-p", options.package, *selected]
        if options.manifest_path:
            command += ["--manifest-path", str(options.manifest_path)]
        if options.features:
            command += ["--features", options.features]
        if options.exact:
            command += [options.exact, "--", "--exact"]
        return cargo.focused(root, "test", command)

    if options.target or options.exact:
        parser.error("--target and --exact require --package")

    if options.filter and options.layer not in ("integration", "system"):
        parser.error("--filter requires integration or system")

    if options.stress_count and options.layer != "integration":
        parser.error("--stress-count requires integration")

    if options.extended:
        execute.task("build:product", "--profile", options.profile)
        execute.task("build:rust:compatibility", "--profile", options.profile)
        execute.module("build.rust.tests", "--extended", "--profile", options.profile)
        execute.task("prepare:tests:fixtures", "--profile", options.profile)

    if options.coverage:
        if options.language == "python":
            parser.error("the tooling unittest suite has no coverage lane")
        extra = ["--category", options.layer] if options.layer else []
        execute.module("test.preflight", "--category", "acceptance", "--profile", options.profile)
        return execute.module("test.run", "coverage", "--language", "rust", "--profile", options.profile, *extra)

    languages = [options.language] if options.language else ["rust", "python"]

    layers = [options.layer] if options.layer else list(arguments.LAYERS)

    for language in languages:
        for layer in layers:
            if language == "python":
                if layer == "integration":
                    execute.task("test:python:integration", "--profile", options.profile)
                continue
            extra = []
            if options.filter:
                extra += ["--filter", options.filter]
            if options.stress_count:
                extra += ["--stress-count", str(options.stress_count)]
            execute.task(f"test:rust:{layer}", "--profile", options.profile, *extra)

    return 0


# === Private Functions === #

# === Entry Point === #
