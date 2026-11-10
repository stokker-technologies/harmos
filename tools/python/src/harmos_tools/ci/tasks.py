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
    generate.add_argument("--sdk", choices=("python",))
    for name in ("format", "lint", "check"):
        command = commands.add_parser(name)
        command.add_argument("--language", choices=(*LANGUAGES, "csharp") if name != "lint" else LANGUAGES)
        if name == "format":
            command.add_argument("--check", action="store_true")
        if name in ("lint", "check"):
            command.add_argument("--extended", action="store_true")
        if name == "check":
            command.add_argument("--package", choices=("harmos-plugin",))
            command.add_argument("--repository", action="store_true")
            command.add_argument("--generated", action="store_true")
    build = commands.add_parser("build")
    build.add_argument("--language", choices=("rust", "csharp"))
    build.add_argument("--profile", choices=("dev", "release"), default="dev")
    build.add_argument("--artifact", choices=("product", "tests", "bench", "fixtures"), default="product")
    build.add_argument("--package")
    build.add_argument("--features")
    build.add_argument("--example")
    build.add_argument("--check", action="store_true")
    build.add_argument("--compatibility", action="store_true")
    build.add_argument("--cpp", action="store_true")
    build.add_argument("--extended", action="store_true")
    test = commands.add_parser("test")
    test.add_argument("--language", choices=("rust", "python", "csharp"))
    test.add_argument("--layer", choices=LAYERS)
    test.add_argument("--package")
    test.add_argument("--features")
    test.add_argument("--target")
    test.add_argument("--exact")
    test.add_argument("--coverage", action="store_true")
    test.add_argument("--profile", choices=("dev", "release"), default="dev")
    test.add_argument("--filter")
    test.add_argument("--stress-count", type=int)
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
    helpers = (
        "pack",
        "extract",
        "summary",
        "nightly-gate",
        "nightly-report",
        "mirror",
        "runners",
        "review",
        "release-mode",
        "release-checks",
        "required",
    )
    for name in helpers:
        helper = commands.add_parser(f"ci:{name}")
        helper.add_argument("--profile", choices=("dev", "release", "bench"), default="dev")
        helper.add_argument("arguments", nargs=argparse.REMAINDER)
    options = parser.parse_args(arguments)
    if options.command.startswith("ci:"):
        from . import workflow

        return workflow.run(options.command.partition(":")[2], options.arguments, options.profile)
    root = Path.cwd()

    # C# SDK Selections Remain Independent of Rust Lanes and Tooling Python Tests.
    if options.command in ("build", "test", "format", "check") and options.language == "csharp":
        from .. import csharp

        incompatible = (
            "package",
            "features",
            "example",
            "compatibility",
            "cpp",
            "extended",
            "target",
            "exact",
            "coverage",
            "filter",
            "stress_count",
            "repository",
            "generated",
        )
        if any(getattr(options, name, None) for name in incompatible):
            parser.error("C# selects only --language, --profile and --layer integration")
        if getattr(options, "artifact", "product") != "product" or getattr(options, "layer", None) not in (None, "integration"):
            parser.error("C# builds its SDK and tests its one integration story")
        if options.command == "build" and options.check:
            parser.error("C# formatting uses check --language csharp")
        return csharp.run(root, options.command, getattr(options, "profile", "dev"), check=getattr(options, "check", False))

    # Focused Cargo Selections
    if options.command in ("build", "test") and options.features and not options.package:
        parser.error("--features requires --package")
    if options.command == "check" and options.package:
        if options.language != "python" or options.repository or options.generated or options.extended:
            parser.error("SDK checks require --language python --package harmos-plugin")
        return _task("check:python:sdk")

    if options.command == "test" and options.language == "python" and options.package == "harmos-plugin":
        if (
            options.target not in (None, "integration")
            or options.coverage
            or options.cpp
            or options.extended
            or options.features
            or options.layer
            or options.filter
            or options.stress_count
        ):
            parser.error("Python SDK tests select --target integration and optional --exact")
        selected = [options.exact] if options.exact else ["discover", "-s", "tests/integration", "-v"]
        return _execute(
            [sys.executable, "-m", "unittest", *selected],
            directory=root / "sdk/python",
            environment=dict(os.environ, TEST_PROFILE=options.profile),
        )

    if options.command == "test" and options.language == "python" and options.exact:
        if options.package or options.target or options.coverage or options.cpp or options.extended or options.features:
            parser.error("focused Python tests select only --exact")
        return _execute([sys.executable, "-m", "unittest", options.exact], directory=root / "tools/python")

    if options.command == "build" and options.package:
        if options.artifact != "product" or options.compatibility or options.cpp or options.extended:
            parser.error("--package cannot select artifacts, compatibility, C++ or extended builds")
        if options.package == "harmos-plugin":
            if options.features or options.example:
                parser.error("Python SDK builds have no Cargo features or examples")
            if options.check:
                return _task("check:python:sdk")
            return _execute(
                [
                    sys.executable,
                    "-m",
                    "build",
                    "--wheel",
                    "--no-isolation",
                    "--outdir",
                    str(root / "target/build" / options.profile / "any/python"),
                    str(root / "sdk/python"),
                ]
            )
        command = ["--locked", "--profile", options.profile, "-p", options.package]
        if options.features:
            command += ["--features", options.features]
        if options.example:
            command += ["--example", options.example]
        return cargo.focused(root, "check" if options.check else "build", command)
    if options.command == "test" and options.package:
        if not options.target:
            parser.error("--package requires --target lib or a test target name")
        if options.language not in (None, "rust") or options.coverage or options.cpp or options.extended:
            parser.error("focused tests require Rust without coverage, C++ or extended selection")
        if options.profile != "dev" or options.layer or options.filter or options.stress_count:
            parser.error("focused tests use the default test profile and --exact for filtering")
        selected = ["--lib"] if options.target == "lib" else ["--test", options.target]
        command = ["--locked", "-p", options.package, *selected]
        if options.features:
            command += ["--features", options.features]
        if options.exact:
            command += [options.exact, "--", "--exact"]
        return cargo.focused(root, "test", command)

    if options.command == "build" and options.example:
        parser.error("--example requires --package")

    # Prepare Explicit Inputs
    if options.command == "prepare":
        return 0
    if options.command == "generate":
        if options.sdk == "python":
            return _module("build.python_protocol")
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
                _module("build.python_protocol", "--check")
                _task("test:python:integration", "--profile", os.environ.get("PROFILE", "dev"))
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
            if options.layer != "integration":
                parser.error("--filter and --stress-count require integration")
        if options.cpp:
            return _task("test:rust:cpp")
        if options.extended:
            _task("build:product", "--profile", options.profile)
            _task("build:rust:compatibility", "--profile", options.profile)
            _module("build.rust.tests", "--extended", "--profile", options.profile)
            _task("prepare:tests:fixtures", "--profile", options.profile)
        if options.coverage:
            if options.language == "python":
                parser.error("the tooling unittest suite has no coverage lane")
            extra = ["--category", options.layer] if options.layer else []
            _module("test.preflight", "--category", "acceptance", "--profile", options.profile)
            return _module("test.run", "coverage", "--language", "rust", "--profile", options.profile, *extra)
        languages = [options.language] if options.language else ["rust", "python"]
        layers = [options.layer] if options.layer else list(LAYERS)
        for language in languages:
            for layer in layers:
                if language == "python":
                    if layer == "integration":
                        _task("test:python:integration", "--profile", options.profile)
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


def _execute(command: list[str], *, directory: Path | None = None, environment: dict[str, str] | None = None) -> int:
    result = subprocess.run(
        command,
        cwd=directory,
        env=environment,
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
        creationflags=spawn_flags(),
        check=True,
    )
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
