# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Verify Release Package Consumption And Export"""

# === Imports === #

# Standard Library Imports
import io
from http.client import HTTPMessage
import json
import os
import shutil
import subprocess
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch
from urllib.request import HTTPRedirectHandler

# Third Party Imports

# Local Imports
from harmos_tools import packages

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class PackageTests(unittest.TestCase):
    def test_native_requests(self) -> None:
        with patch.dict("os.environ", {"GITHUB_TOKEN": "native-test-token"}):
            request = packages._request("actions/artifacts/123/zip")
        self.assertEqual(
            request.get_header("Authorization"), "Bearer native-test-token"
        )
        redirected = HTTPRedirectHandler().redirect_request(
            request,
            io.BytesIO(),
            302,
            "Found",
            HTTPMessage(),
            "https://artifact-storage.example.invalid/archive.zip",
        )
        assert redirected is not None
        self.assertIsNone(redirected.get_header("Authorization"))
        with patch(
            "harmos_tools.packages._json",
            side_effect=[
                {"jobs": [{"id": n} for n in range(100)]},
                {"jobs": [{"id": 100}]},
            ],
        ) as provider:
            self.assertEqual(
                len(packages._pages("actions/runs/42/jobs?filter=all", "jobs")), 101
            )
            self.assertIn("&per_page=100&page=2", provider.call_args.args[0])

    def test_ci_preflight(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._repository(root)
            self._package(root)
            manifest = packages.location(root) / "reports/build.json"
            original = manifest.read_bytes()
            (root / "input.txt").write_text("changed CI input")
            with patch.dict("os.environ", {"CI": "true"}):
                with self.assertRaisesRegex(RuntimeError, "clean committed source"):
                    packages.stage(root)
            self.assertEqual(manifest.read_bytes(), original)

    def test_package_transfer(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            original = Path(directory) / "original"
            self._repository(original)
            self._package(original)
            relocated = Path(directory) / "relocated"
            shutil.copytree(original, relocated)
            with patch(
                "harmos_tools.packages.subprocess.run", wraps=subprocess.run
            ) as run:
                paths = packages.verify(relocated)
                self.assertEqual(
                    set(paths), set(packages.BINARIES) | set(packages.PLUGINS)
                )
                self.assertTrue(
                    all(path.is_relative_to(relocated) for path in paths.values())
                )
                self.assertFalse(
                    any("build" in call.args[0] for call in run.call_args_list)
                )
            if os.name != "nt":
                asset = paths[packages.BINARIES[0]]
                original = asset.read_bytes()
                asset.chmod(0o644)
                packages.restore(relocated)
                self.assertEqual(asset.stat().st_mode & 0o777, 0o755)
                self.assertEqual(asset.read_bytes(), original)
            with patch.dict("os.environ", {"RUSTFLAGS": "--cfg=changed_build_inputs"}):
                with self.assertRaisesRegex(RuntimeError, "resolved inputs"):
                    packages.verify(relocated)
            report = packages.location(relocated) / "reports/build.json"
            manifest = json.loads(report.read_text())
            self.assertEqual(manifest["revision"], "a" * 40)
            (relocated / "input.txt").write_text("changed source")
            with self.assertRaisesRegex(RuntimeError, "source inputs"):
                packages.verify(relocated)

    def test_incomplete_package(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._repository(root)
            self._package(root)
            manifest = packages.location(root) / "reports/build.json"
            value = json.loads(manifest.read_text())
            asset = packages.location(root) / next(iter(value["artifacts"].values()))
            original = asset.read_bytes()
            for damage in ("missing", "corrupt", "manifest"):
                with self.subTest(damage=damage):
                    if damage == "missing":
                        asset.unlink()
                    elif damage == "corrupt":
                        asset.write_bytes(b"changed")
                    else:
                        value["artifacts"].pop(next(iter(value["artifacts"])))
                        manifest.write_text(json.dumps(value))
                    with self.assertRaises(RuntimeError):
                        packages.verify(root)
                    asset.write_bytes(original)

            self._package(root)
            license_file = packages.location(root) / "LICENSE-APACHE"
            license_file.write_text("corrupt license")
            with self.assertRaisesRegex(RuntimeError, "Apache license"):
                packages.verify(root)

    def test_failure_export(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._repository(root)
            self._package(root)
            reports = root / ".stkr/tests"
            reports.mkdir(parents=True)
            (reports / "tests-integration-rust.xml").write_text(
                '<testsuite failures="1"/>'
            )
            diagnostics = root / "target/validation"
            diagnostics.mkdir(parents=True)
            (diagnostics / "failure.log").write_text("native failure output")
            packages.export(root)
            with zipfile.ZipFile(
                root / "dist" / f"harmos-{packages.platform()}.zip"
            ) as archive:
                self.assertEqual(
                    archive.read("LICENSE-APACHE"),
                    (root / "LICENSE-APACHE").read_bytes(),
                )
                self.assertIn("tests/tests-integration-rust.xml", archive.namelist())
                self.assertIn(
                    "reports/test-diagnostics/failure.log", archive.namelist()
                )
                self.assertFalse(
                    any(name.startswith("dist/") for name in archive.namelist())
                )
                self.assertEqual(
                    archive.read("reports/build.json"),
                    (packages.location(root) / "reports/build.json").read_bytes(),
                )

    def test_validation_receipt(self) -> None:
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.dict(
                "os.environ",
                {
                    "CI": "true",
                    "GITHUB_RUN_ID": "42",
                    "GITHUB_JOB": f"test_{packages.platform()}",
                    "GITHUB_RUN_ATTEMPT": "2",
                },
            ),
        ):
            root = Path(directory)
            self._repository(root)
            self._evidence(root)
            self._package(root)
            packages.validate(root)
            record = packages.location(root) / "reports/validation.json"
            self.assertEqual(json.loads(record.read_text())["run"], "42")
            manifest = root / ".stkr/tests/latest.json"
            original = json.loads(manifest.read_text())
            for field, value in (
                ("revision", "stale-revision"),
                ("platform", "wrong-platform"),
                ("started_unix_seconds", 0),
                ("configurations", []),
                ("requested_configuration", "focused"),
            ):
                with self.subTest(field=field):
                    records = json.loads(json.dumps(original))
                    records["tests"]["integration-rust"][field] = value
                    manifest.write_text(json.dumps(records))
                    with self.assertRaises(RuntimeError):
                        packages.validate(root)
                    self.assertFalse(record.exists())
            manifest.write_text(json.dumps(original))
            packages.validate(root)
            (root / ".stkr/tests/coverage-rust.xml").unlink()
            with self.assertRaises(FileNotFoundError):
                packages.validate(root)
            self.assertFalse(record.exists())

    def test_reuse_archive(self) -> None:
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.dict(
                "os.environ",
                {
                    "CI": "true",
                    "GITHUB_RUN_ID": "42",
                    "GITHUB_JOB": f"test_{packages.platform()}",
                    "GITHUB_RUN_ATTEMPT": "2",
                    "HARMOS_VALIDATION_RUN_ID": "42",
                },
            ),
        ):
            original = Path(directory) / "original"
            self._repository(original)
            self._evidence(original)
            self._package(original)
            packages.validate(original)
            packages.export(original)
            downloadable = original / "dist" / f"harmos-{packages.platform()}.zip"
            immutable = downloadable.read_bytes()
            container = io.BytesIO()
            with zipfile.ZipFile(container, "w") as archive:
                for path in (original / "dist").rglob("*"):
                    if path.is_file():
                        archive.write(
                            path, path.relative_to(original / "dist").as_posix()
                        )
            proposed = Path(directory) / "proposed"
            shutil.copytree(
                original, proposed, ignore=shutil.ignore_patterns("dist", ".stkr")
            )
            actual_run = subprocess.run

            def run(command, **kwargs):
                if command[:2] == ["git", "fetch"]:
                    return subprocess.CompletedProcess(command, 0)
                return actual_run(command, **kwargs)

            actual_output = packages._output

            def output(repository, command):
                if command[:2] == ["git", "rev-parse"]:
                    return "same-tree"
                return actual_output(repository, command)

            pipeline = {
                "id": 42,
                "status": "completed",
                "conclusion": "success",
                "head_sha": "a" * 40,
                "path": ".github/workflows/full.yml",
                "run_attempt": 2,
            }
            jobs = [
                {
                    "id": (99 if system == packages.platform() else 100)
                    if phase == "test"
                    else (88 if system == packages.platform() else 89)
                    if phase == "build:release"
                    else index + 1,
                    "name": f"{'Build Release' if phase == 'build:release' else phase} ({system})",
                    "conclusion": "success",
                }
                for index, (phase, system) in enumerate(
                    (phase, system)
                    for phase in ("check", "Build Release", "test")
                    for system in ("linux", "windows")
                )
            ]
            artifact = {
                "id": 123,
                "name": f"harmos-test-{packages.platform()}-2",
                "expired": False,
                "workflow_run": {"id": 42, "head_sha": "a" * 40},
                "digest": "sha256:"
                + packages.hashlib.sha256(container.getvalue()).hexdigest(),
            }

            def provider(endpoint):
                if "/attempts/" in endpoint:
                    attempt = int(endpoint.split("/attempts/")[1].split("/")[0])
                    return {
                        "jobs": [
                            item
                            for item in jobs
                            if (str(item["name"]).startswith("test ")) == (attempt == 2)
                        ]
                    }
                if "/artifacts?" in endpoint:
                    return {"artifacts": [artifact]}
                return pipeline

            with (
                patch("harmos_tools.packages._json", side_effect=provider),
                patch("harmos_tools.packages._output", side_effect=output),
                patch(
                    "harmos_tools.packages.subprocess.run", side_effect=run
                ) as executed,
                patch(
                    "harmos_tools.packages.urlopen",
                    side_effect=lambda *args, **kwargs: io.BytesIO(
                        container.getvalue()
                    ),
                ),
            ):
                packages.reuse(proposed)
                self.assertFalse(
                    any("build" in call.args[0] for call in executed.call_args_list)
                )
                self.assertEqual(
                    (proposed / "dist" / downloadable.name).read_bytes(), immutable
                )
                for field, value, message in (
                    ("expired", True, "missing or expired"),
                    ("digest", "sha256:" + "0" * 64, "digest is corrupt"),
                    (
                        "name",
                        f"harmos-test-{packages.platform()}-1",
                        "missing or expired",
                    ),
                    (
                        "workflow_run",
                        {"id": 43, "head_sha": "a" * 40},
                        "does not belong",
                    ),
                ):
                    original_value = artifact[field]
                    artifact[field] = value
                    with (
                        self.subTest(field=field),
                        self.assertRaisesRegex(RuntimeError, message),
                    ):
                        packages.reuse(proposed)
                    artifact[field] = original_value
                    self.assertEqual(
                        (proposed / "dist" / downloadable.name).read_bytes(), immutable
                    )
                for malicious in (
                    "dist/linux/../outside",
                    "dist/linux/C:/outside",
                    "dist/linux/..\\outside",
                ):
                    with self.subTest(path=malicious):
                        corrupt = io.BytesIO()
                        with zipfile.ZipFile(corrupt, "w") as archive:
                            archive.writestr(malicious, b"unsafe")
                        artifact["digest"] = (
                            "sha256:"
                            + packages.hashlib.sha256(corrupt.getvalue()).hexdigest()
                        )
                        with patch(
                            "harmos_tools.packages.urlopen",
                            return_value=io.BytesIO(corrupt.getvalue()),
                        ):
                            with self.assertRaisesRegex(RuntimeError, "unsafe"):
                                packages.reuse(proposed)
                        self.assertEqual(
                            (proposed / "dist" / downloadable.name).read_bytes(),
                            immutable,
                        )
                artifact["digest"] = (
                    "sha256:"
                    + packages.hashlib.sha256(container.getvalue()).hexdigest()
                )
                jobs[-1]["conclusion"] = "failure"
                with self.assertRaisesRegex(RuntimeError, "platform matrix"):
                    packages.reuse(proposed)
                jobs[-1]["conclusion"] = "success"
                pipeline["conclusion"] = "failure"
                with self.assertRaisesRegex(RuntimeError, "not succeeded"):
                    packages.reuse(proposed)
                pipeline["conclusion"] = "success"
                with patch(
                    "harmos_tools.packages._output",
                    side_effect=["old-tree", "new-tree"],
                ):
                    with self.assertRaisesRegex(RuntimeError, "source tree"):
                        packages.reuse(proposed)

    @staticmethod
    def _evidence(root: Path) -> None:
        policy = root / "tools/python/evidence.toml"
        policy.parent.mkdir(parents=True)
        policy.write_text(
            '[testing.rust]\ncategories=["integration"]\n[testing.python]\ncategories=["integration", "acceptance"]\n'
        )
        reports = root / ".stkr/tests"
        reports.mkdir(parents=True)
        records = {}
        for name in ("integration-rust", "integration-python", "acceptance-python"):
            category, language = name.split("-")
            records[name] = {
                "outcome": "passed",
                "requested_configuration": None,
                "category": category,
                "language": language,
                "coverage": True,
                "revision": packages._output(root, ["git", "rev-parse", "HEAD"]),
                "platform": packages.platform(),
                "started_unix_seconds": 2,
                "finished_unix_seconds": 3,
                "configurations": [{"configuration": "default", "success": True}],
            }
            (reports / f"tests-{name}.xml").write_text(
                '<testsuite><testcase name="scenario" time="0.01"/></testsuite>'
            )
        for language in ("rust", "python"):
            (reports / f"coverage-{language}.xml").write_text(
                '<coverage lines-valid="1"><class filename="source"/></coverage>'
            )
        (reports / "latest.json").write_text(json.dumps({"tests": records}))

    @staticmethod
    def _repository(root: Path) -> None:
        root.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            ["git", "init", "--quiet", str(root)],
            check=True,
            env=packages._git_environment(),
        )
        (root / ".gitignore").write_text("dist/\n.stkr/\ntarget/\n.tmp/\n")
        (root / "input.txt").write_text("product source")
        (root / "LICENSE-APACHE").write_text("Apache License Fixture")
        subprocess.run(
            ["git", "add", "."], cwd=root, check=True, env=packages._git_environment()
        )
        subprocess.run(
            [
                "git",
                "-c",
                "user.name=Package Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                "test: package fixture",
            ],
            cwd=root,
            check=True,
            env=packages._git_environment(),
        )

    @staticmethod
    def _package(root: Path) -> None:
        destination = packages.location(root)
        suffix = ".exe" if packages.platform() == "windows" else ""
        artifacts = {
            name: f"cli/{name}{suffix}" for name in packages.BINARIES
        } | packages.PLUGINS
        destination.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / "LICENSE-APACHE", destination / "LICENSE-APACHE")
        for relative in artifacts.values():
            path = destination / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"release asset")
        report = destination / "reports/build.json"
        report.parent.mkdir(parents=True, exist_ok=True)
        report.write_text(
            json.dumps(
                {
                    "schema": 2,
                    "platform": packages.platform(),
                    "revision": "a" * 40,
                    "built_unix_seconds": 1,
                    "run": "42",
                    "attempt": "1",
                    "job": f"build_release_{packages.platform()}",
                    "source": packages.source(root),
                    "inputs": packages.release.inputs(root),
                    "artifacts": artifacts,
                    "versions": {
                        name: packages._output(root, command)
                        for name, command in packages.VERSION_COMMANDS.items()
                    },
                    "digests": {
                        relative: packages._digest(destination / relative)
                        for relative in artifacts.values()
                    },
                }
            )
        )


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    unittest.main()
