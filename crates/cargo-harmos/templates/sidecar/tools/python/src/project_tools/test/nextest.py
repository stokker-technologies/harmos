# Rendered by stkr project update from templates/python/test/nextest.py; sha256:677ca8d19ce1d86bcb021ec06192092aaa50b0a0272c2f5a629146bfec98ecff
# ***
#
# __PRODUCT_NAME__
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Generated Nextest Profile, Layer Filtersets And Commands For Rust Lanes.

Layers are selected by binary: unit is every library, binary and proc-macro target; acceptance
and system are the integration-test binaries named after the layer (`tests/acceptance/main.rs`
autodiscovers as `acceptance`; a `[[test]]` named `acceptance-<suite>` also counts); every other
integration-test binary is the integration layer. The repository `.config/nextest.toml` is never
read: the runner writes the one profile it needs and passes it with `--config-file`.
"""

# === Imports === #

# Standard Library Imports
import json
import re
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports

# Local Imports
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

PROFILE = "stkr"
UNIT = "kind(lib) | kind(bin) | kind(proc-macro)"
PLAIN = re.compile(r"^[A-Za-z0-9_:.\-]+$")
PACKAGE_FLAGS = ("-p", "--package")
FILTER_FLAGS = ("-E", "--filterset", "--filter-expr")
TIMEOUT = 'slow-timeout = { period = "%ds", terminate-after = 1 }'

# === Types === #

# === Classes === #

# === Public Functions === #


def layer(category: str, binaries: Sequence[str]) -> str:
    """The filterset of one test layer; nextest rejects matchers that name no binary, so only names that exist appear."""
    if category == "unit":
        return UNIT
    if category in ("acceptance", "system"):
        return f"kind(test) & ({_named(category, binaries)})"
    return f"kind(test) & !({_named('acceptance', binaries)}) & !({_named('system', binaries)})"


def selection(category: str | None, arguments: Sequence[str], binaries: Sequence[str]) -> tuple[str | None, list[str]]:
    """One filterset joining the layer with the lane's `-p` and `-E` arguments, plus the rest.

    Nextest ORs repeated `-E` expressions, so narrowing folds into a single intersection.
    """
    packages, filters, rest = _split(arguments)
    parts = [layer(category, binaries)] if category else []
    if packages:
        parts.append(" | ".join(f"package(={name})" for name in packages))
    parts.extend(filters)
    if not parts:
        return None, rest
    return " & ".join(f"({part})" for part in parts), rest


def config(
    store: str,
    deadline: int,
    overrides: Sequence[tuple[str, int]],
) -> str:
    """The nextest configuration of one lane; the first matching override wins."""
    lines = [
        "[store]",
        "dir = " + json.dumps(store),
        "",
        f"[profile.{PROFILE}]",
        "retries = 0",
        "fail-fast = false",
        TIMEOUT % deadline,
        "",
        f"[profile.{PROFILE}.junit]",
        'path = "junit.xml"',
    ]
    for expression, seconds in overrides:
        lines += [
            "",
            f"[[profile.{PROFILE}.overrides]]",
            "filter = " + json.dumps(expression),
            TIMEOUT % seconds,
        ]
    return "\n".join(lines) + "\n"


def overrides(
    rules: policy.Policy,
    category: str | None,
    durations: dict[str, dict[str, dict]],
    coverage: bool,
    binaries: Sequence[str],
) -> list[tuple[str, int]]:
    """Reviewed overrides first, then layer deadlines (all-layer runs) or measured stories."""
    categories = [category] if category else list(policy.CATEGORIES)
    factor = policy.COVERAGE_FACTOR if coverage else 1
    selected = [(entry.select, entry.timeout_seconds * factor) for name in categories for entry in rules.selected(name, "rust")]
    if category is None:
        return selected + [(layer(name, binaries), rules.deadline(name, coverage)) for name in categories]
    if coverage:
        return selected

    lane = rules.deadline(category)
    for suite, stories in sorted(durations.items()):
        for story, record in sorted(stories.items()):
            deadline = rules.story(category, record.get("seconds"))
            if deadline < lane and PLAIN.match(suite) and PLAIN.match(story):
                selected.append((f"binary_id(={suite}) & test(={story})", deadline))
    return selected


def run(
    archive: Path,
    directory: Path,
    configuration: Path,
    extract: Path,
    expression: str | None,
    rest: Sequence[str],
) -> list[str]:
    """Run tests from the archive: no compilation, workspace remapped onto this checkout."""
    extract.mkdir(parents=True, exist_ok=True)
    command = [
        "cargo",
        "nextest",
        "run",
        "--archive-file",
        str(archive),
        "--workspace-remap",
        str(directory),
        "--extract-to",
        str(extract),
        "--extract-overwrite",
        "--config-file",
        str(configuration),
        "--profile",
        PROFILE,
        "--no-tests",
        "fail",
    ]
    if expression:
        command += ["-E", expression]
    return command + list(rest)


def coverage(
    profile: str,
    configuration: Path,
    expression: str | None,
    rest: Sequence[str],
) -> list[str]:
    """Instrumented build and run; coverage is the one Rust job that compiles."""
    command = [
        "cargo",
        "llvm-cov",
        "nextest",
        "--no-report",
        "--locked",
        "--workspace",
        "--cargo-profile",
        profile,
        "--config-file",
        str(configuration),
        "--profile",
        PROFILE,
        "--no-tests",
        "fail",
    ]
    if expression:
        command += ["-E", expression]
    return command + list(rest)


def report(profile: str, output: Path) -> list[str]:
    return [
        "cargo",
        "llvm-cov",
        "report",
        "--profile",
        profile,
        "--cobertura",
        "--ignore-filename-regex",
        "(/tests/|/benches/|/fixtures/)",
        "--output-path",
        str(output),
    ]


# === Private Functions === #


def _named(category: str, binaries: Sequence[str]) -> str:
    names = [name for name in binaries if name == category or name.startswith(f"{category}-")]
    return " | ".join(f"binary(={name})" for name in names) or "none()"


def _split(arguments: Sequence[str]) -> tuple[list[str], list[str], list[str]]:
    packages: list[str] = []
    filters: list[str] = []
    rest: list[str] = []
    values = iter(arguments)
    for argument in values:
        flag, _, inline = argument.partition("=")
        if flag in PACKAGE_FLAGS + FILTER_FLAGS:
            value = inline or next(values, "")
            if not value:
                raise policy.TestError(f"{flag} needs a value")
            (packages if flag in PACKAGE_FLAGS else filters).append(value)
        elif argument.startswith("-p") and len(argument) > 2:
            packages.append(argument[2:])
        else:
            rest.append(argument)
    return packages, filters, rest


# === Entry Point === #
