# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Parse Public Task Selections."""

# === Imports === #

# Standard Library Imports
import argparse
from pathlib import Path

# Third Party Imports


# Local Imports


# Type Checking Imports

# === Public Exports === #

# === Constants === #

LANGUAGES = ("rust", "python", "toml", "typescript")
LAYERS = ("unit", "integration", "acceptance", "system")

# === Types === #

# === Classes === #

# === Public Functions === #


def parse(arguments: list[str] | None = None) -> tuple[argparse.ArgumentParser, argparse.Namespace]:
    parser = argparse.ArgumentParser(description="Select Repository Task Steps")
    commands = parser.add_subparsers(dest="command", required=True)
    prepare = commands.add_parser("prepare")
    prepare.add_argument("--docs-only", action="store_true")
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
            release.add_argument("--registry", choices=("pypi",))
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
        "review",
        "release-mode",
        "required",
    )
    for name in helpers:
        helper = commands.add_parser(f"ci:{name}")
        helper.add_argument("--profile", choices=("dev", "release", "bench"), default="dev")
        helper.add_argument("arguments", nargs=argparse.REMAINDER)
    options = parser.parse_args(arguments)

    return parser, options


# === Private Functions === #

# === Entry Point === #
