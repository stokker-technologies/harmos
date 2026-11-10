# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Select Repository Task Steps Without Preparing The Environment Again."""

# === Imports === #

# Standard Library Imports
import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

# Third Party Imports

# Local Imports
from ..build import cargo
from ..process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

LANGUAGES = ("rust", "python", "toml")
LAYERS = ("unit", "integration", "acceptance", "system")

# === Types === #

# === Classes === #

# === Public Functions === #


def main(arguments: list[str] | None = None) -> int:
    from . import artifacts

    # Nested Task Leaves Must Not Repeat Transport Or See The Outer Job's Plan.
    inputs = os.environ.pop("CI_INPUT_ARCHIVES", "[]")
    outputs = os.environ.pop("CI_OUTPUT_ARCHIVES", "[]")
    selected = arguments if arguments is not None else sys.argv[1:]
    if "--help" in selected or "-h" in selected:
        return dispatch(arguments)
    root = Path.cwd()
    archives = [Path(path) for path in json.loads(inputs)]
    for archive in archives:
        artifacts.extract(root, archive)
    status = dispatch(arguments)
    if status:
        return status
    from ..build.paths import target

    for output in json.loads(outputs):
        artifacts.pack(root, Path(output["archive"]), output["paths"], output.get("profile", os.environ.get("PROFILE", "dev")), target())
    return status


def dispatch(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("prepare")
    generate = commands.add_parser("generate")
    generate.add_argument("--profile", choices=("dev", "release"), default="dev")
    for name in ("format", "lint", "check"):
        command = commands.add_parser(name)
        command.add_argument("--language", choices=LANGUAGES)
        if name == "format":
            command.add_argument("--check", action="store_true")
        if name in ("lint", "check"):
            command.add_argument("--extended", action="store_true")
        if name == "check":
            command.add_argument("--repository", action="store_true")
            command.add_argument("--generated", action="store_true")
    build = commands.add_parser("build")
    build.add_argument("--profile", choices=("dev", "release"), default="dev")
    build.add_argument("--artifact", choices=("product", "tests", "bench", "fixtures"), default="product")
    build.add_argument("--package")
    build.add_argument("--check", action="store_true")
    build.add_argument("--compatibility", action="store_true")
    build.add_argument("--cpp", action="store_true")
    build.add_argument("--extended", action="store_true")
    test = commands.add_parser("test")
    test.add_argument("--language", choices=("rust", "python"))
    test.add_argument("--layer", choices=LAYERS)
    test.add_argument("--package")
    test.add_argument("--target")
    test.add_argument("--exact")
    test.add_argument("--coverage", action="store_true")
    test.add_argument("--profile", choices=("dev", "release"), default="dev")
    test.add_argument("--filter")
    test.add_argument("--stress-count", type=int)
    test.add_argument("--starter", action="store_true")
    test.add_argument("--cpp", action="store_true")
    test.add_argument("--extended", action="store_true")
    bench = commands.add_parser("bench")
    bench.add_argument("--layer", choices=("unit", "integration", "system"))
    bench.add_argument("--refresh-baseline", action="store_true")
    bench.add_argument("--extended", action="store_true")
    package = commands.add_parser("package")
    package.add_argument("--language", choices=("rust",))
    package.add_argument("--profile", choices=("dev", "release"), default="dev")
    for name in ("prepare", "tag", "rehearse", "publish", "verify"):
        release = commands.add_parser(f"release:{name}")
        if name == "publish":
            publication = release.add_mutually_exclusive_group()
            publication.add_argument("--dry", action="store_true")
            publication.add_argument("--stage", choices=("assets", "rust", "finalize", "nightly"))
        if name == "prepare":
            release.add_argument("--review", action="store_true")
        if name == "rehearse":
            release.add_argument("--target")
            release.add_argument("--profile", choices=("dev", "release"))
            release.add_argument("--stage-only", action="store_true")
        release.add_argument("arguments", nargs=argparse.REMAINDER)
    helpers = ("pack", "extract", "summary", "nightly-gate", "nightly-report", "mirror", "runners", "review", "release-checks", "required")
    for name in helpers:
        helper = commands.add_parser(f"ci:{name}")
        helper.add_argument("--profile", choices=("dev", "release", "bench"), default="dev")
        helper.add_argument("arguments", nargs=argparse.REMAINDER)
    options = parser.parse_args(arguments)
    if options.command.startswith("ci:"):
        from . import workflow

        return workflow.run(options.command.partition(":")[2], options.arguments, options.profile)
    root = Path.cwd()

    # Focused Cargo Selections
    if options.command == "build" and options.package:
        if options.artifact != "product" or options.compatibility or options.cpp or options.extended:
            parser.error("--package cannot select artifacts, compatibility, C++ or extended builds")
        return cargo.focused(root, "check" if options.check else "build", ["--locked", "--profile", options.profile, "-p", options.package])
    if options.command == "test" and options.package:
        if not options.target:
            parser.error("--package requires --target lib or a test target name")
        if options.language not in (None, "rust") or options.coverage or options.starter or options.cpp or options.extended:
            parser.error("focused tests require Rust without coverage, starter, C++ or extended selection")
        if options.profile != "dev" or options.layer or options.filter or options.stress_count:
            parser.error("focused tests use the default test profile and --exact for filtering")
        selected = ["--lib"] if options.target == "lib" else ["--test", options.target]
        command = ["--locked", "-p", options.package, *selected]
        if options.exact:
            command += [options.exact, "--", "--exact"]
        return cargo.focused(root, "test", command)

    # Prepare Explicit Inputs
    if options.command == "prepare":
        return 0
    if options.command == "generate":
        from ..build.__main__ import generate

        generate(options.profile)
        return 0

    # Formatting And Checks
    if options.command in ("format", "lint", "check"):
        languages = [options.language] if options.language else list(LANGUAGES)
        if options.command == "check" and options.repository:
            return _task("check:repository")
        if options.command == "check" and options.generated:
            return _task("check:python:generated")
        for language in languages:
            if options.command in ("format", "check"):
                extra = ["--check"] if options.command == "check" or options.check else []
                _task(f"format:{language}", *extra)
            if options.command in ("lint", "check") and language != "toml":
                extra = ["--extended"] if language == "rust" and options.extended else []
                _task(f"lint:{language}", *extra)
            if options.command == "check" and language == "rust":
                _task("check:rust")
            if options.command == "check" and language == "python":
                _task("test:python:integration")
        if options.command == "check":
            _task("check:repository")
            if options.language in (None, "python"):
                _task("check:python:generated")
        return 0

    # Build Producers
    if options.command == "build":
        if options.check:
            parser.error("--check requires --package")
        if options.artifact == "fixtures":
            if options.cpp or options.compatibility or options.extended:
                parser.error("--artifact fixtures cannot select C++, compatibility or extended builds")
            return _task("prepare:tests:fixtures", "--profile", options.profile)
        if options.cpp:
            return _task("build:cpp")
        if options.compatibility:
            return _task("build:rust:compatibility", "--profile", options.profile)
        if options.artifact == "bench":
            return _task("build:rust:bench", *(["--extended"] if options.extended else []))
        if options.artifact == "tests":
            if options.extended:
                return _module("build.rust.tests", "--extended", "--profile", options.profile)
            return _task("build:rust:tests", "--profile", options.profile)
        if options.extended:
            parser.error("--extended requires --artifact tests or bench")
        return _task("build:product", "--profile", options.profile)

    # Test Consumers
    if options.command == "test":
        if options.target or options.exact:
            parser.error("--target and --exact require --package")
        if options.filter or options.stress_count:
            if options.layer != "integration" and not options.starter:
                parser.error("--filter and --stress-count require integration or --starter")
            if options.starter and options.stress_count:
                parser.error("--starter does not support --stress-count")
        if options.cpp:
            return _task("test:rust:cpp")
        if options.starter:
            extra = ["--filter", options.filter] if options.filter else []
            return _task("test:rust:starter", "--profile", options.profile, *extra)
        if options.extended:
            _task("build:product", "--profile", options.profile)
            _task("build:rust:compatibility", "--profile", options.profile)
            _module("build.rust.tests", "--extended", "--profile", options.profile)
            _task("prepare:tests:fixtures", "--profile", options.profile)
        if options.coverage:
            if options.language == "python":
                parser.error("the tooling unittest suite has no coverage lane")
            extra = ["--category", options.layer] if options.layer else []
            _module("test.preflight", "--category", "acceptance", "--starter", "--profile", options.profile)
            return _module("test.run", "coverage", "--language", "rust", "--profile", options.profile, *extra)
        languages = [options.language] if options.language else ["rust", "python"]
        layers = [options.layer] if options.layer else list(LAYERS)
        for language in languages:
            for layer in layers:
                if language == "python":
                    if layer == "integration":
                        _task("test:python:integration")
                    continue
                extra = []
                if options.filter:
                    extra += ["--filter", options.filter]
                if options.stress_count:
                    extra += ["--stress-count", str(options.stress_count)]
                _task(f"test:rust:{layer}", "--profile", options.profile, *extra)
        return 0

    # Benchmarks And Packaging
    if options.command == "bench":
        layers = [options.layer] if options.layer else ["unit", "integration", "system"]
        extra = ["--extended"] if options.extended else []
        if options.refresh_baseline:
            extra.append("--refresh-baseline")
        for layer in layers:
            _task(f"bench:rust:{layer}", *extra)
        return 0
    if options.command == "package":
        return _task("package:rust", "--profile", options.profile)

    # Release Operations
    operation = options.command.partition(":")[2]
    if operation == "publish":
        if not options.dry and options.stage in (None, "assets") and os.environ.get("GITHUB_ACTIONS"):
            _module("release.checks")
            _module("release.run", "identity")
        if options.stage == "nightly":
            return _module("release.nightly")
        if options.dry:
            return _task("release:publish:dry", *options.arguments)
        if options.stage:
            return _task(f"release:publish:{options.stage}", *options.arguments)
        _task("release:publish:assets")
        return _task("release:publish:finalize")
    if operation == "prepare" and options.review:
        _module("release.run", "prepare")
        if os.environ.get("GITHUB_RUNNER_WORKFLOWS"):
            previous = os.environ.get("GH_TOKEN")
            os.environ["GH_TOKEN"] = os.environ.get("RUNNER_GROUP_READ_TOKEN", previous or "")
            try:
                _module("test.runners")
            finally:
                if previous is None:
                    os.environ.pop("GH_TOKEN", None)
                else:
                    os.environ["GH_TOKEN"] = previous
        operation = "review"
    if operation == "tag" and os.environ.get("GITHUB_ACTIONS"):
        _module("release.run", "identity")
        _module("release.checks")
    extra = options.arguments[1:] if options.arguments[:1] == ["--"] else options.arguments
    if operation == "rehearse":
        if options.target:
            extra += ["--target", options.target]
        if options.profile:
            extra += ["--profile", options.profile]
        if options.stage_only:
            extra += ["--stage-only"]
    return _module("release.run", operation, *extra)


# === Private Functions === #


def _execute(command: list[str]) -> int:
    result = subprocess.run(command, stdin=sys.stdin, stdout=sys.stdout, stderr=sys.stderr, creationflags=spawn_flags(), check=True)
    return result.returncode


def _task(name: str, *arguments: str) -> int:
    command = ["mise", "run", name, "--", *arguments]
    compiling = name.startswith("build:") or name == "lint:rust" or name.startswith("release:publish:")
    if compiling and os.environ.get("COMPILER_CACHE_BACKEND"):
        from . import build

        label = name.replace(":", " / ").replace("-", " ").title()
        status = build.run(command, label)
        if status:
            raise SystemExit(status)
        return status
    return _execute(command)


def _module(name: str, *arguments: str) -> int:
    # Reuse This Interpreter; Entry Points May Have A SystemExit Main Guard.
    import runpy

    sys.argv = [name, *arguments]
    try:
        runpy.run_module(f"harmos_tools.{name}", run_name="__main__")
    except SystemExit as error:
        if error.code:
            raise
    return 0


# === Entry Point === #

if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as error:
        raise SystemExit(error.returncode) from None
