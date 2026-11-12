# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""SDK Dependency Closure"""

# === Imports === #

# Standard Library Imports
import json
from pathlib import Path
import subprocess
import unittest

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class SdkDependencyClosure(unittest.TestCase):
    def test_plugin_and_client_compile_without_host_packages(self):
        root = Path(__file__).resolve().parents[3]
        result = subprocess.run(
            [
                "cargo",
                "metadata",
                "--locked",
                "--offline",
                "--no-deps",
                "--format-version",
                "1",
            ],
            cwd=root,
            capture_output=True,
            text=True,
            check=True,
        )
        packages = {
            package["name"]: package
            for package in json.loads(result.stdout)["packages"]
        }
        pending = ["harmos"]
        visited = set()
        forbidden = {
            "harmos-domain",
            "harmos-storage",
            "harmos-operations",
            "harmos-runtime",
            "harmos-guest",
            "harmos-packaging",
        }
        while pending:
            name = pending.pop()
            if name in visited:
                continue
            visited.add(name)
            self.assertNotIn(
                name, forbidden, f"SDK normal/build dependency reaches {name}"
            )
            for dependency in packages.get(name, {}).get("dependencies", []):
                if dependency["kind"] != "dev":
                    pending.append(dependency["name"])
        self.assertIn("harmos-protocol", visited)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
