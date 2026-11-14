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
import tomllib
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
    generate.add_argument("--dependencies", action="store_true")
    generate.add_argument("--descriptors", type=Path, action="append", default=[])
    generate.add_argument("--output", type=Path)
    generate.add_argument("--namespace", default="harmos")
    generate.add_argument("--sdk", choices=("python", "rust", "csharp", "cpp"))
    for name in ("format", "lint", "check"):
        command = commands.add_parser(name)
        languages = LANGUAGES
        if name in ("format", "check"):
            languages = (*languages, "csharp")
        if name == "check":
            languages = (*languages, "cpp")
        command.add_argument("--language", choices=languages)
        if name == "format":
            command.add_argument("--check", action="store_true")
        if name in ("lint", "check"):
            command.add_argument("--extended", action="store_true")
        if name == "check":
            command.add_argument("--package", choices=("harmos-sdk",))
            command.add_argument("--repository", action="store_true")
            command.add_argument("--generated", action="store_true")
    build = commands.add_parser("build")
    build.add_argument("--language", choices=("rust", "csharp", "cpp"))
    build.add_argument("--profile", choices=("dev", "release"), default="dev")
    build.add_argument("--artifact", choices=("product", "tests", "bench", "fixtures"), default="product")
    build.add_argument("--package")
    build.add_argument("--manifest-path", type=Path)
    build.add_argument("--target")
    build.add_argument("--messages", type=Path)
    build.add_argument("--features")
    build.add_argument("--no-default-features", action="store_true")
    build.add_argument("--example")
    build.add_argument("--lib", action="store_true")
    build.add_argument("--check", action="store_true")
    build.add_argument("--compatibility", action="store_true")
    build.add_argument("--extended", action="store_true")
    build.add_argument("--focused-messages", type=Path)
    build.add_argument("--libraries")
    build.add_argument("--inspection-messages")
    build.add_argument("--inspections")
    build.add_argument("--componentizer", type=Path)
    test = commands.add_parser("test")
    test.add_argument("--language", choices=("rust", "python", "csharp", "cpp"))
    test.add_argument("--layer", choices=LAYERS)
    test.add_argument("--package")
    test.add_argument("--manifest-path", type=Path)
    test.add_argument("--features")
    test.add_argument("--target")
    test.add_argument("--exact")
    test.add_argument("--story")
    test.add_argument("--paths", nargs="+", type=Path)
    test.add_argument("--no-run", action="store_true")
    test.add_argument("--timings", action="store_true")
    test.add_argument("--coverage", action="store_true")
    test.add_argument("--profile", choices=("dev", "release"), default="dev")
    test.add_argument("--filter")
    test.add_argument("--stress-count", type=int)
    test.add_argument("--extended", action="store_true")
    bench = commands.add_parser("bench")
    bench.add_argument("--layer", choices=("unit", "integration", "system"))
    bench.add_argument("--refresh-baseline", action="store_true")
    bench.add_argument("--extended", action="store_true")
    bench.add_argument("--storage", action="store_true")
    bench.add_argument("--filter")
    package = commands.add_parser("package")
    package.add_argument("--language", choices=("rust", "csharp"))
    package.add_argument("--profile", choices=("dev", "release"), default="dev")
    for name in ("prepare", "tag", "rehearse", "publish", "verify"):
        release = commands.add_parser(f"release:{name}")
        if name in ("publish", "verify"):
            release.add_argument("--channel", choices=("stable", "nightly"), default="stable")
        if name == "publish":
            publication = release.add_mutually_exclusive_group()
            publication.add_argument("--dry", action="store_true")
            publication.add_argument("--stage", choices=("assets", "rust", "finalize"))
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
        "nightly-freshness",
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

    if options.command == "build":
        focused_fixtures = (
            options.focused_messages,
            options.libraries,
            options.inspection_messages,
            options.inspections,
            options.componentizer,
        )
        if any(value is not None for value in focused_fixtures) and (options.artifact != "fixtures" or options.package):
            parser.error("focused fixture selectors require --artifact fixtures")

    if options.command == "build" and options.lib:
        if not options.package or options.package == "harmos-sdk" or options.example or options.language not in (None, "rust"):
            parser.error("--lib requires a Rust --package without --example")
    if options.command == "test":
        for selector in ("package", "target", "story", "exact"):
            value = getattr(options, selector)
            if value is not None and not value:
                parser.error(f"--{selector} requires a nonempty value")

    # Developer Rust Selections Never Require The Shared Test Archive.
    if options.command == "test" and (
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

    # C# SDK Selections Remain Independent of Rust Lanes and Tooling Python Tests.
    if options.command in ("build", "test", "format", "check", "package") and options.language == "csharp":
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
        if getattr(options, "artifact", "product") != "product" or getattr(options, "layer", None) not in (None, "unit", "integration"):
            parser.error("C# builds its SDK and tests its one integration story")
        if options.command == "build" and options.check:
            parser.error("C# formatting uses check --language csharp")
        selected = []
        if options.command in ("build", "test", "package"):
            selected += ["--profile", options.profile]
        if getattr(options, "layer", None):
            selected += ["--layer", options.layer]
        if getattr(options, "check", False):
            selected += ["--check"]
        return _execute(["mise", "-C", "sdk/csharp", "run", options.command, "--", *selected])

    if options.command in ("build", "test", "check") and options.language == "cpp":
        if options.command == "build":
            if (
                options.package
                or options.features
                or options.example
                or options.check
                or options.artifact != "product"
                or options.compatibility
                or options.extended
            ):
                parser.error("C++ builds select only --language cpp and optional --profile")
            return _execute(["mise", "-C", "sdk/cpp", "run", "build", "--", "--profile", options.profile])
        if options.command == "check":
            if options.package or options.repository or options.generated or options.extended:
                parser.error("C++ checks select only --language cpp")
            return _execute(["mise", "-C", "sdk/cpp", "run", "check"])
        if (
            options.package
            or options.features
            or options.target
            or options.coverage
            or options.extended
            or options.filter
            or options.stress_count
            or options.layer not in (None, "unit", "integration")
        ):
            parser.error("C++ conformance selects --language cpp, optional --profile and --exact")
        selected = ["--profile", options.profile]
        if options.layer:
            selected += ["--layer", options.layer]
        if options.exact:
            selected += ["--exact", options.exact]
        return _execute(["mise", "-C", "sdk/cpp", "run", "test", "--", *selected])

    # Focused Cargo Selections
    if options.command in ("build", "test") and options.manifest_path and not options.package:
        parser.error("--manifest-path requires --package")
    if options.command in ("build", "test") and options.features and not options.package:
        parser.error("--features requires --package")
    if options.command == "check" and options.package:
        if options.language != "python" or options.repository or options.generated or options.extended:
            parser.error("SDK checks require --language python --package harmos-sdk")
        return _task("check:python:sdk")

    if options.command == "test" and options.language == "python" and options.package == "harmos-sdk":
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
        return _execute(["mise", "-C", "sdk/python", "run", "test", "--", *selected], environment=environment)

    if options.command == "test" and options.language == "python" and options.exact:
        if options.package or options.target or options.coverage or options.extended or options.features:
            parser.error("focused Python tests select only --exact")
        return _execute([sys.executable, "-m", "unittest", options.exact], directory=root / "tools/python")

    if options.command == "build" and options.package:
        if options.artifact != "product" or options.compatibility or options.extended:
            parser.error("--package cannot select artifacts, compatibility, C++ or extended builds")
        if options.package == "harmos-sdk":
            if options.features or options.example:
                parser.error("Python SDK builds have no Cargo features or examples")
            if options.check:
                return _task("check:python:sdk")
            return _execute(["mise", "-C", "sdk/python", "run", "build", "--", "--profile", options.profile])
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
    if options.command == "test" and options.package:
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

    if options.command == "build" and options.example:
        parser.error("--example requires --package")

    # Prepare Explicit Inputs
    if options.command == "prepare":
        enabled = os.environ.get("MISE_ENABLE_TOOLS")
        if enabled is None or "rust" in enabled.split(","):
            toolchain = tomllib.loads((root / "rust-toolchain.toml").read_text(encoding="utf-8"))["toolchain"]
            _execute(
                [
                    "rustup",
                    "toolchain",
                    "install",
                    toolchain["channel"],
                    "--profile",
                    toolchain["profile"],
                    "--component",
                    ",".join(toolchain["components"]),
                    "--target",
                    ",".join(toolchain["targets"]),
                ]
            )
        return 0
    if options.command == "generate":
        from ..build.resource_roots import generate as resource_roots

        if options.descriptors:
            from ..build.resource_roots import compose

            if options.output is None:
                parser.error("composed client roots require --output")
            compose(root, options.descriptors, options.output, options.namespace, language=options.sdk)
            return 0
        if options.sdk == "python":
            return _execute(["mise", "-C", "sdk/python", "run", "generate"])
        if options.sdk == "csharp":
            return _execute(["mise", "-C", "sdk/csharp", "run", "generate"])
        if options.sdk == "cpp":
            return _execute(["mise", "-C", "sdk/cpp", "run", "generate", "--", "--profile", options.profile])
        if options.sdk == "rust":
            result = cargo.focused(root, "build", ["--locked", "-p", "harmos-protocol", "--example", "generate"])
            if result:
                return result
            binary = (
                root
                / "target"
                / ("debug" if options.profile == "dev" else "release")
                / "examples"
                / ("generate.exe" if os.name == "nt" else "generate")
            )
            result = _execute([str(binary)])
            if result == 0:
                resource_roots(root)
            return result
        from ..build.__main__ import generate

        generate(options.profile)
        return 0

    # Formatting And Checks
    if options.command in ("format", "lint", "check"):
        languages = [options.language] if options.language else list(LANGUAGES)
        if options.command == "check" and options.repository:
            return _task("check:repository")
        if options.command == "check" and (options.generated or options.language in (None, "python")):
            if not (root / "target/generated/python/src").is_dir():
                parser.error(
                    "generated Python clients are absent; run `mise run build` before the complete Python check (hosted CI supplies Product outputs)"
                )
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
            if options.compatibility or options.extended:
                parser.error("--artifact fixtures cannot select C++, compatibility or extended builds")
            selected = ["--profile", options.profile]
            for name in ("focused_messages", "libraries", "inspection_messages", "inspections", "componentizer"):
                value = getattr(options, name)
                if value is not None:
                    selected += ["--" + name.replace("_", "-"), str(value)]
            return _task("prepare:tests:fixtures", *selected)
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
        if options.filter and options.layer not in ("integration", "system"):
            parser.error("--filter requires integration or system")
        if options.stress_count and options.layer != "integration":
            parser.error("--stress-count requires integration")
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
        if options.storage:
            from .. import storage_bench

            return storage_bench.run(root, options.filter)
        if options.filter:
            parser.error("--filter requires --storage")
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
    if operation in ("publish", "verify") and options.channel == "nightly":
        from ..release import nightly
        from ..release import policy as release_policy

        if operation == "verify":
            nightly.verify(root)
        elif options.dry:
            from ..release import run as releases

            releases.dry(root, release_policy.policy(root, "release"), profile="release", require_all=True, channel="nightly")
        elif options.stage:
            parser.error("nightly publication has one complete publish entry point")
        else:
            nightly.publish(root)
        return 0
    if operation == "publish":
        if not options.dry and options.stage in (None, "assets") and os.environ.get("GITHUB_ACTIONS"):
            _module("release.checks")
            _module("release.run", "identity")
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
