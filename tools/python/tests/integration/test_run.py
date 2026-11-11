# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Prove Story Results Ignore Measurements And Script Hangs Terminate."""

# === Imports === #

# Standard Library Imports
import json
import runpy
import socket
import sys
import tempfile
import unittest
from pathlib import Path
from types import ModuleType
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.test import run

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[4]

# === Types === #

# === Classes === #


class RunTests(unittest.TestCase):
    runners: list[ModuleType]

    @classmethod
    def setUpClass(cls):
        cls.runners = [run]
        for path in sorted((ROOT / "crates/cargo-harmos/templates").rglob("test/run.py.tmpl")):
            namespace = runpy.run_path(str(path), run_name=f"{run.__package__}.template_runner")
            module = ModuleType(namespace["__name__"])
            module.__dict__.update(namespace)
            cls.runners.append(module)

    def test_completed_story_measurements_preserve_results(self):
        for runner in self.runners:
            for language in ("python", "typescript", "javascript"):
                for instrumented in (False, True):
                    with (
                        self.subTest(runner=runner.__file__, language=language, coverage=instrumented),
                        tempfile.TemporaryDirectory() as temporary,
                    ):
                        directory = Path(temporary).resolve()
                        (directory / "tools").mkdir()
                        (directory / "tools/project.toml").write_text("", encoding="utf-8")
                        rules = runner.policy.load(directory)
                        seconds = rules.deadline("unit", coverage=True) + 1
                        key = f"unit-{language}"
                        with runner._scratch(directory, key, "dev") as scratch:
                            junit = scratch / "junit.xml"
                            document = (
                                f'<testsuite name="fixture"><testcase name="passes" time="{seconds}"/>'
                                '<testcase name="skips"><skipped/></testcase></testsuite>'
                            )
                            junit.write_text(document, encoding="utf-8")
                            environment = {runner.COVERAGE_VARIABLE: "1"} if instrumented else {}
                            with patch.dict(runner._lane.__globals__, _execute=lambda *args, **kwargs: 0):
                                result = runner._lane(
                                    directory, rules, key, "unit", language, ".", "dev", [], directory, environment, junit, scratch
                                )
                        self.assertEqual((result["outcome"], result["passed"], result["failed"], result["skipped"]), ("passed", 1, 0, 1))
                        self.assertEqual(result["durations"], [("fixture", "passes", directory.name, seconds)])
                        destination = runner.report.location(directory, "dev", key)
                        self.assertEqual((destination / "junit.xml").read_text(), document)
                        self.assertEqual(
                            json.loads((destination / "durations.json").read_text()), {"fixture": {"passes": {"seconds": seconds}}}
                        )

    def test_script_hang_kills_owned_process_tree(self):
        for runner in self.runners:
            with self.subTest(runner=runner.__file__), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary).resolve()
                # The Child Holds A Socket Until Killed; The Parent Reports Before Hanging.
                child = (
                    "import pathlib, socket, sys, threading; "
                    "server = socket.socket(); server.bind(('127.0.0.1', 0)); server.listen(); "
                    "pathlib.Path(sys.argv[1]).write_text(str(server.getsockname()[1])); "
                    "threading.Event().wait()"
                )
                parent = (
                    "import pathlib, subprocess, sys, threading, time\n"
                    "port = pathlib.Path(sys.argv[2])\n"
                    f"subprocess.Popen([sys.executable, '-c', {child!r}, str(port)])\n"
                    "while not port.exists(): time.sleep(0.01)\n"
                    "pathlib.Path(sys.argv[1]).write_text('<testsuite><testcase name=\"finished\"/></testsuite>')\n"
                    "threading.Event().wait()\n"
                )
                port = directory / "port"
                rules = runner.policy.Policy(limits={}, deadlines={"system": 3}, overrides=())
                with runner._scratch(directory, "system-typescript", "dev") as scratch:
                    junit = scratch / "junit.xml"
                    result = runner._lane(
                        directory,
                        rules,
                        "system-typescript",
                        "system",
                        "typescript",
                        ".",
                        "dev",
                        [sys.executable, "-c", parent, str(junit), str(port)],
                        directory,
                        None,
                        junit,
                        scratch,
                    )
                self.assertEqual(result["outcome"], "failed")
                with socket.socket() as client:
                    self.assertNotEqual(client.connect_ex(("127.0.0.1", int(port.read_text()))), 0)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    unittest.main()
