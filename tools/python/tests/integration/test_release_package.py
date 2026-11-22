# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Retain the Selected Release Package Inputs."""

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
import harmos_tools.package.run
from harmos_tools.ci import tasks
from harmos_tools.package import conformance
from harmos_tools.release import layout, notices, policy, sbom

# Type Checking Imports


# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #


class ReleasePackage(unittest.TestCase):
    def test_python_selection_stages_existing_wheel_without_rebuilding(self) -> None:
        temporary_directory = tempfile.TemporaryDirectory()
        try:
            directory = Path(temporary_directory.name)
            (directory / "tools").mkdir()
            (directory / "tools/project.toml").write_text('[version]\nvalue = "1.2.3"\n')
            for profile in ("dev", "release"):
                source = directory / "target/build" / profile / "any/python"
                source.mkdir(parents=True)
                wheel = source / "harmos_sdk-1.2.3-py3-none-any.whl"
                wheel.write_bytes(profile.encode())
                destination = directory / "target/package" / profile / "any/python"
                destination.mkdir(parents=True)
                (destination / "stale.whl").write_bytes(b"stale")
                other = destination.parent / "rust/retained.crate"
                other.parent.mkdir()
                other.write_bytes(b"retained")

                with (
                    self.subTest(profile=profile),
                    patch.object(Path, "cwd", return_value=directory),
                    patch("sys.argv", ["package.run", "python", "--profile", profile, "--target", "any"]),
                ):
                    harmos_tools.package.run.main()

                self.assertEqual(list(destination.iterdir()), [destination / wheel.name])
                self.assertEqual((destination / wheel.name).read_bytes(), wheel.read_bytes())
                self.assertEqual(other.read_bytes(), b"retained")
        finally:
            temporary_directory.cleanup()

    def test_python_selection_refuses_missing_build_output(self) -> None:
        temporary_directory = tempfile.TemporaryDirectory()
        try:
            directory = Path(temporary_directory.name)
            (directory / "tools").mkdir()
            (directory / "tools/project.toml").write_text('[version]\nvalue = "1.2.3"\n')
            for empty in (False, True):
                if empty:
                    (directory / "target/build/dev/any/python").mkdir(parents=True)
                with (
                    self.subTest(empty=empty),
                    patch.object(Path, "cwd", return_value=directory),
                    patch("sys.argv", ["package.run", "python", "--target", "any"]),
                    self.assertRaises(policy.ToolError),
                ):
                    harmos_tools.package.run.main()
        finally:
            temporary_directory.cleanup()

    def test_python_task_selects_existing_platform_independent_packages(self) -> None:
        for profile in ("dev", "release"):
            with self.subTest(profile=profile), patch("harmos_tools.task.execute.module", return_value=0) as module:
                self.assertEqual(tasks.dispatch(["package", "python", "--profile", profile]), 0)
            module.assert_called_once_with("package.run", "python", "--profile", profile, "--target", "any")

    def test_matching_product_bytes_and_locked_licenses_become_release_inputs(self) -> None:
        temporary_directory = tempfile.TemporaryDirectory()
        temporary = temporary_directory.name
        try:
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
            archive = zipfile.ZipFile(wheel, "w")
            try:
                archive.writestr("fixture-1.0.dist-info/METADATA", "Name: fixture\nVersion: 1.0\nLicense-Expression: MIT\n")
                archive.writestr("fixture-1.0.dist-info/licenses/LICENSE", "Fixture License")
            finally:
                archive.close()
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
                        self.assertEqual(harmos_tools.package.run.binary_bytes(archive, name + suffix), (name + target).encode())
                    rust = json.loads((packaged / "compliance/crates" / abi / "workspace.cdx.json").read_text())
                    python = json.loads((packaged / "compliance/python/any/sdk.cdx.json").read_text())
                    self.assertEqual(rust["components"][0]["licenses"], [{"expression": "MIT"}])
                    self.assertEqual(python["components"][0]["purl"], "pkg:pypi/fixture@1.0")
                    self.assertEqual(rust["dependencies"], [{"ref": rust["components"][0]["bom-ref"], "dependsOn": []}])
                    self.assertEqual(
                        cargo.call_args.args[1], ["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", abi]
                    )
                    (build / "product/target.txt").write_text("wrong-abi")
                    with self.assertRaises(policy.ToolError):
                        conformance.package(directory, "release", target)
                with patch.object(notices, "download", return_value=b"corrupt"), self.assertRaises(policy.ToolError):
                    import tomllib

                    notices.python_notices(tomllib.loads((sdk / "uv.lock").read_text()))
        finally:
            temporary_directory.cleanup()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
