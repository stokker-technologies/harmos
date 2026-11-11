# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Package The Reference Host And Its Locked Compliance Evidence"""

# === Imports === #

# Standard Library Imports
import hashlib
import io
import json
import os
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.package import conformance, run as packages
from harmos_tools.release import layout, notices, policy, sbom

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class ReleasePackage(unittest.TestCase):
    def test_matching_product_bytes_and_locked_licenses_become_release_inputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / "tools").mkdir()
            (directory / "tools/project.toml").write_text("""[version]
value = "1.2.3"
[release]
product = "harmos"
[release.target_triples]
linux-x86_64 = "x86_64-unknown-linux-gnu"
windows-x86_64 = "x86_64-pc-windows-msvc"
""")
            (directory / "LICENSE").write_text("Fixture License")
            wheel = io.BytesIO()
            with zipfile.ZipFile(wheel, "w") as archive:
                archive.writestr("fixture-1.0.dist-info/METADATA", "Name: fixture\nVersion: 1.0\nLicense-Expression: MIT\n")
                archive.writestr("fixture-1.0.dist-info/licenses/LICENSE", "Fixture License")
            data = wheel.getvalue()
            digest = hashlib.sha256(data).hexdigest()
            sdk = directory / "sdk/python"
            sdk.mkdir(parents=True)
            (sdk / "uv.lock").write_text(f"""[[package]]
name = "fixture"
version = "1.0"
source = {{ registry = "https://example.test/simple" }}
wheels = [{{url = "https://example.test/fixture.whl", hash = "sha256:{digest}", size = {len(data)}}}]
""")
            metadata = {
                "packages": [
                    {
                        "id": "fixture",
                        "name": "fixture",
                        "version": "1.0",
                        "license": "MIT",
                        "source": None,
                        "manifest_path": str(directory / "Cargo.toml"),
                    }
                ],
                "resolve": {"nodes": [{"id": "fixture", "dependencies": []}]},
            }
            settings = policy.policy(directory, "release")
            with (
                patch.dict(os.environ, {"UV_CACHE_DIR": str(directory / "cache")}),
                patch.object(sbom.policy, "execute", return_value=json.dumps(metadata)) as cargo,
                patch.object(notices, "download", return_value=data),
            ):
                for target, abi in settings["target_triples"].items():
                    build = directory / "target/build/release" / target
                    (build / "bin").mkdir(parents=True)
                    (build / "product").mkdir()
                    (build / "product/target.txt").write_text(abi)
                    suffix = ".exe" if target.startswith("windows") else ""
                    for name in ("harmos-conformance", "harmos-conformance-plugin"):
                        (build / "bin" / (name + suffix)).write_bytes((name + target).encode())
                    conformance.package(directory, "release", target)
                    packaged = directory / "target/package/release" / target
                    archive = packaged / "rust" / layout.conformance_name(directory, settings, target, "1.2.3")
                    for name in ("harmos-conformance", "harmos-conformance-plugin"):
                        self.assertEqual(packages.binary_bytes(archive, name + suffix), (name + target).encode())
                    rust = json.loads((packaged / "compliance/crates" / abi / "workspace.cdx.json").read_text())
                    python = json.loads((packaged / "compliance/python/any/sdk.cdx.json").read_text())
                    self.assertEqual(rust["components"][0]["licenses"], [{"expression": "MIT"}])
                    self.assertEqual(python["components"][0]["purl"], "pkg:pypi/fixture@1.0")
                    self.assertEqual(rust["dependencies"], [{"ref": rust["components"][0]["bom-ref"], "dependsOn": []}])
                    self.assertEqual(
                        cargo.call_args.args[1], ["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", abi]
                    )
                    (build / "product/target.txt").write_text("wrong-abi")
                    with self.assertRaisesRegex(policy.ToolError, "differs from release target"):
                        conformance.package(directory, "release", target)
                with patch.object(notices, "download", return_value=b"corrupt"):
                    with self.assertRaisesRegex(policy.ToolError, "hash mismatch"):
                        import tomllib

                        notices.python_notices(tomllib.loads((sdk / "uv.lock").read_text()))


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
