# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Test Run Operations."""

# === Imports === #

# Standard Library Imports
import argparse
import sys
from collections.abc import Sequence

# Third Party Imports
# Local Imports
import harmos_tools.test.benchmark
import harmos_tools.test.coverage
import harmos_tools.test.lane

from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def main(arguments: Sequence[str] | None = None) -> int:
    values = list(sys.argv[1:] if arguments is None else arguments)
    extra: list[str] = []
    if "--" in values:
        split = values.index("--")
        values, extra = values[:split], values[split + 1 :]

    parser = argparse.ArgumentParser(description="Test Lanes And Evidence")
    commands = parser.add_subparsers(dest="command", required=True)
    lane = commands.add_parser("test", help="run one test lane")
    lane.add_argument("--category", choices=policy.CATEGORIES, required=True)
    lane.add_argument("--language", choices=policy.LANGUAGES, required=True)
    lane.add_argument("--coverage", action="store_true")
    bench_lane = commands.add_parser("bench", help="run one benchmark layer")
    bench_lane.add_argument("--category", choices=policy.BENCH_CATEGORIES, required=True)
    bench_lane.add_argument("--language", choices=("rust", "python"), required=True)
    bench_lane.add_argument("--refresh-baseline", action="store_true")
    bench_lane.add_argument("--archive-argument", action="append", default=[])
    measured = commands.add_parser("coverage", help="instrumented run of every layer")
    measured.add_argument("--language", choices=("rust", "python"), required=True)
    for command in (lane, bench_lane, measured):
        command.add_argument("--project", default=".")
    for command in (lane, measured):
        command.add_argument("--profile", choices=policy.PROFILES, default="dev")
    options = parser.parse_args(values)

    try:
        directory = policy.root()
        if options.command == "test" and options.coverage:
            return harmos_tools.test.coverage.run(
                directory,
                options.language,
                options.category,
                options.project,
                options.profile,
                extra,
            )
        if options.command == "test":
            return harmos_tools.test.lane.run(
                directory,
                options.category,
                options.language,
                options.project,
                options.profile,
                extra,
            )
        if options.command == "coverage":
            return harmos_tools.test.coverage.run(
                directory,
                options.language,
                None,
                options.project,
                options.profile,
                extra,
            )
        if options.command == "bench":
            return harmos_tools.test.benchmark.run(
                directory,
                options.category,
                options.language,
                options.project,
                options.refresh_baseline,
                extra,
                options.archive_argument,
            )
        return 0
    except (policy.TestError, OSError, ValueError, KeyError) as error:
        print(f"test: {error}", file=sys.stderr)
        return 1


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
