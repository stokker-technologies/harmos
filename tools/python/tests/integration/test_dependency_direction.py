# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Production Dependency Direction"""

# === Imports === #

# Standard Library Imports
import tomllib
import unittest
from pathlib import Path

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class DependencyDirection(unittest.TestCase):
    def test_production_edges_follow_domain_ownership(self) -> None:
        root = Path(__file__).resolve().parents[4]
        owners = {
            "harmos-protocol": set(),
            "harmos-domain": {"harmos-protocol"},
            "harmos-query": {"harmos-domain", "harmos-protocol"},
            "harmos-storage": {"harmos-query", "harmos-domain", "harmos-protocol"},
            "harmos-operations": {"harmos-storage", "harmos-query", "harmos-domain", "harmos-protocol"},
            "harmos-runtime": {"harmos-operations", "harmos-storage", "harmos-query", "harmos-domain", "harmos-protocol"},
            "harmos": {"harmos-protocol"},
        }
        workspace = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]
        manifests = [directory / "Cargo.toml" for pattern in workspace["members"] for directory in root.glob(pattern)]
        manifests = [manifest for manifest in manifests if manifest.is_file()]
        local = {tomllib.loads(manifest.read_text(encoding="utf-8"))["package"]["name"] for manifest in manifests}
        shared = workspace["dependencies"]
        for manifest in manifests:
            data = tomllib.loads(manifest.read_text(encoding="utf-8"))
            package = data["package"]["name"]
            if package not in owners:
                continue
            dependencies = list(data.get("dependencies", {}).items())
            for target in data.get("target", {}).values():
                dependencies.extend(target.get("dependencies", {}).items())
            for name, dependency in dependencies:
                if isinstance(dependency, dict) and dependency.get("workspace"):
                    dependency = shared[name]
                path_dependency = isinstance(dependency, dict) and "path" in dependency
                dependency = dependency.get("package", name) if isinstance(dependency, dict) else name
                if dependency == "harmos-sdk-derive":
                    continue
                if dependency in local or path_dependency:
                    self.assertIn(dependency, owners[package], f"{manifest}: forbidden production dependency {dependency}")


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
