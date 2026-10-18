# Rendered by stkr project update from templates/python/test/policy.py; sha256:06f5eb86cf98859fa7dd3e72e0e156199daf631132ef9d5e3729b621c9d67ca4
# ***
#
# __PRODUCT_NAME__
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Testing Policy From `stkr.toml [conventions.testing]` And The Expected Lane Set.

Keys, all optional:

    [conventions.testing]
    limits = { unit = 512, integration = 256, acceptance = 128, system = 64 }
    deadlines = { unit = 10, integration = 120, acceptance = 600, system = 900 }
    hosts = ["dist/build/{profile}/app"]
    bench_regression_percent = 20

    [[conventions.testing.overrides]]
    category = "acceptance"
    language = "rust"
    select = "test(=cycle::runs)"
    timeout_seconds = 300
    reason = "Streams a full capture"

Limits are project-wide story counts per layer; exceeding one is a curation finding in the
index, never a failure. Deadlines are the per-story maximum of a lane in seconds. An override
replaces the deadline of the stories it selects: for Rust `select` is a nextest filterset, for
the other languages a glob over `<file>::<story>`. Hosts are artifacts acceptance and system
lanes need; `{profile}`, `{platform}`, `{architecture}`, `{target}` and `{exe}` expand to the lane profile,
native operating system, CPU architecture, producer target and executable suffix. Declared hosts require a
fixture stamp whose immutable file map covers every host before a lane can run.

The expected lanes are the `test:<category>:<language>` leaf tasks of `mise.toml`.
"""

# === Imports === #

# Standard Library Imports
import fnmatch
import math
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

CATEGORIES = ("unit", "integration", "acceptance", "system")
BENCH_CATEGORIES = ("unit", "integration", "system")
LANGUAGES = ("rust", "python", "typescript", "javascript")
PROFILES = ("dev", "release")
LIMITS = {"unit": 512, "integration": 256, "acceptance": 128, "system": 64}
DEADLINES = {"unit": 10, "integration": 120, "acceptance": 600, "system": 900}

# Liveness Guard: About Three Recorded Durations, Never Below Ten Seconds.
MEASURED_FACTOR = 3
MEASURED_FLOOR = 10
COVERAGE_FACTOR = 5
OVERRIDE_KEYS = {"category", "language", "select", "timeout_seconds", "reason"}

# === Types === #

# === Classes === #


class TestError(Exception):
    """A preflight, policy or runner failure reported as one clear message."""


@dataclass(frozen=True)
class Override:
    category: str
    language: str
    select: str
    timeout_seconds: int
    reason: str


@dataclass(frozen=True)
class Policy:
    limits: dict[str, int]
    deadlines: dict[str, int]
    overrides: tuple[Override, ...]
    hosts: tuple[str, ...]
    bench_regression_percent: float = 20

    def deadline(self, category: str, coverage: bool = False) -> int:
        """The lane maximum per story, multiplied for instrumented runs."""
        return self.deadlines[category] * (COVERAGE_FACTOR if coverage else 1)

    def story(self, category: str, recorded: float | None) -> int:
        """The liveness deadline of one story from its last recorded duration."""
        lane = self.deadlines[category]
        if category not in ("acceptance", "system") or recorded is None:
            return lane
        measured = max(MEASURED_FACTOR * recorded, MEASURED_FLOOR)
        return min(lane, int(measured + 0.999))

    def selected(self, category: str, language: str) -> list[Override]:
        return [entry for entry in self.overrides if entry.category == category and entry.language == language]

    def matched(self, category: str, language: str, story: str) -> Override | None:
        """The first glob override of a script or Python story."""
        for entry in self.selected(category, language):
            if fnmatch.fnmatchcase(story, entry.select):
                return entry
        return None


# === Public Functions === #


def root(start: Path | None = None) -> Path:
    """The project root: the nearest directory holding `stkr.toml`."""
    current = (start or Path.cwd()).resolve()
    for candidate in (current, *current.parents):
        if (candidate / "stkr.toml").is_file():
            return candidate
    raise TestError("no stkr.toml found above the working directory")


def load(directory: Path) -> Policy:
    document = _toml(directory / "stkr.toml")
    testing = document.get("conventions", {}).get("testing", {})
    if not isinstance(testing, dict):
        raise TestError("[conventions.testing] must be a table")

    limits = _numbers(testing.get("limits", {}), "limits", LIMITS, 0)
    deadlines = _numbers(testing.get("deadlines", {}), "deadlines", DEADLINES, 1)
    hosts = testing.get("hosts", [])
    if not isinstance(hosts, list) or not all(isinstance(host, str) for host in hosts):
        raise TestError("conventions.testing.hosts must be a list of paths")

    overrides = []
    for entry in testing.get("overrides", []):
        if not isinstance(entry, dict) or set(entry) != OVERRIDE_KEYS:
            raise TestError("each override declares " + ", ".join(sorted(OVERRIDE_KEYS)))
        if entry["category"] not in CATEGORIES or entry["language"] not in LANGUAGES:
            raise TestError(f"override names an unknown lane: {entry}")
        if not _positive(entry["timeout_seconds"]) or not str(entry["reason"]).strip():
            raise TestError(f"override needs a positive timeout and a reason: {entry}")
        overrides.append(Override(**entry))
    threshold = testing.get("bench_regression_percent", 20)
    if isinstance(threshold, bool) or not isinstance(threshold, (int, float)) or not math.isfinite(threshold) or threshold <= 0:
        raise TestError("bench_regression_percent must be finite and positive")
    return Policy(limits, deadlines, tuple(overrides), tuple(hosts), float(threshold))


def lanes(directory: Path) -> list[tuple[str, str]]:
    """The expected `(category, language)` lanes: the `test:*:*` leaves in `mise.toml`."""
    path = directory / "mise.toml"
    tasks = _toml(path).get("tasks", {}) if path.is_file() else {}
    expected = []
    for name in tasks:
        parts = name.split(":")
        if len(parts) == 3 and parts[0] == "test":
            if parts[1] in CATEGORIES and parts[2] in LANGUAGES:
                expected.append((parts[1], parts[2]))
    return sorted(expected, key=lambda lane: (CATEGORIES.index(lane[0]), lane[1]))


# === Private Functions === #


def _toml(path: Path) -> dict[str, Any]:
    try:
        return tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise TestError(f"read {path.name}: {error}") from error


def _numbers(value: object, name: str, defaults: dict[str, int], minimum: int) -> dict[str, int]:
    if not isinstance(value, dict) or not set(value) <= set(defaults):
        raise TestError(f"conventions.testing.{name} keys are {', '.join(defaults)}")
    if not all(_positive(number, minimum) for number in value.values()):
        raise TestError(f"conventions.testing.{name} values must be integers >= {minimum}")
    return {**defaults, **value}


def _positive(value: object, minimum: int = 1) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= minimum


# === Entry Point === #
