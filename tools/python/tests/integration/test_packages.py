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
        for event, target, expected in (
            ("push", "refs/heads/dev", True),
            ("push", "refs/heads/main", False),
            ("pull_request", "dev", True),
            ("pull_request", "main", False),
            ("workflow_dispatch", "refs/heads/dev", False),
        ):
            with patch.dict(
                "os.environ",
                {
                    "GITHUB_EVENT_NAME": event,
                    "GITHUB_REF": target,
                    "GITHUB_BASE_REF": target,
                },
                clear=True,
            ):
                self.assertEqual(packages.development_fast(), expected)
                with tempfile.TemporaryDirectory() as directory:
                    output = Path(directory) / "output"
                    with patch.dict("os.environ", {"GITHUB_OUTPUT": str(output)}):
                        packages.development_policy()
                    self.assertEqual(
                        output.read_text(), f"fast={str(expected).lower()}\n"
                    )
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

        revision = "a" * 40
        with (
            patch("harmos_tools.packages._output", return_value=revision),
            patch(
                "harmos_tools.packages._json",
                return_value={"workflow_runs": [{"id": 42}]},
            ) as provider,
        ):
            self.assertEqual(packages._reference(Path("unused")), "42")
            self.assertIn(f"head_sha={revision}", provider.call_args.args[0])

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
                original_bytes = asset.read_bytes()
                asset.chmod(0o644)
                packages.restore(relocated)
                self.assertEqual(asset.stat().st_mode & 0o777, 0o755)
                self.assertEqual(asset.read_bytes(), original_bytes)
            with patch.dict("os.environ", {"RUSTFLAGS": "--cfg=changed_build_inputs"}):
                with self.assertRaisesRegex(RuntimeError, "resolved inputs"):
                    packages.verify(relocated)
            report = packages.location(relocated) / "reports/build.json"
            manifest = json.loads(report.read_text())
            self.assertEqual(
                manifest["revision"],
                packages._output(relocated, ["git", "rev-parse", "HEAD"]),
            )
            (relocated / "input.txt").write_text("changed source")
            with self.assertRaisesRegex(RuntimeError, "source inputs"):
                packages.verify(relocated)

            (relocated / "input.txt").write_text("product source")
            previous_tree = packages._output(
                relocated, ["git", "rev-parse", "HEAD^{tree}"]
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
                    "--allow-empty",
                    "-m",
                    "test: identical tree with new sha",
                ],
                cwd=relocated,
                check=True,
                env=packages._git_environment(),
            )
            self.assertEqual(
                packages._output(relocated, ["git", "rev-parse", "HEAD^{tree}"]),
                previous_tree,
            )
            with self.assertRaisesRegex(RuntimeError, "fresh build"):
                packages.verify(relocated)

    def test_incomplete_package(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._repository(root)
            self._package(root)
            manifest = packages.location(root) / "reports/build.json"
            value = json.loads(manifest.read_text())
            wrong_profile = dict(
                value, profile="release" if packages.build_profile() == "dev" else "dev"
            )
            manifest.write_text(json.dumps(wrong_profile))
            with self.assertRaisesRegex(RuntimeError, "artifact profile"):
                packages.verify(root)
            manifest.write_text(json.dumps(value))
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

    def test_archive_roundtrip(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            original = Path(directory) / "original"
            self._repository(original)
            self._package(original)
            root = packages.location(original)
            manifest = json.loads((root / "reports/build.json").read_text())
            tool = manifest["containers"]["tool"]
            expected = set(tool["members"].values())
            archive = root / tool["path"]
            if archive.suffix == ".zip":
                with zipfile.ZipFile(archive) as files:
                    self.assertEqual(set(files.namelist()), expected)
            else:
                with packages.tarfile.open(archive, "r:gz") as files:
                    self.assertEqual(set(files.getnames()), expected)
            self.assertEqual(len(expected), 3)
            examples = root / manifest["containers"]["examples"]["path"]
            with zipfile.ZipFile(examples) as files:
                self.assertIn("reports/build.json", files.namelist())
                self.assertIn(packages.PLUGINS["schematic-tools"], files.namelist())
                self.assertFalse(
                    any(
                        "fixture" in name or name.endswith(".core.wasm")
                        for name in files.namelist()
                    )
                )
            packages.export(original)
            envelope = original / "dist" / f"harmos-ci-{packages.platform()}.zip"
            immutable = envelope.read_bytes()
            with zipfile.ZipFile(envelope) as files:
                self.assertFalse(
                    any(
                        name.endswith("schematic-guest.wasm")
                        for name in files.namelist()
                    )
                )
                self.assertFalse(any("/cli/" in name for name in files.namelist()))
                self.assertIn(f"{packages.platform()}/{tool['path']}", files.namelist())
            relocated = Path(directory) / "relocated"
            shutil.copytree(original, relocated, ignore=shutil.ignore_patterns("dist"))
            (relocated / "dist").mkdir()
            shutil.copy2(envelope, relocated / "dist" / envelope.name)
            with patch(
                "harmos_tools.packages.subprocess.run", wraps=subprocess.run
            ) as run:
                packages.restore(relocated)
                self.assertFalse(
                    any("build" in call.args[0] for call in run.call_args_list)
                )
            restored = packages.location(relocated)
            self.assertEqual(
                (restored / tool["path"]).read_bytes(), archive.read_bytes()
            )
            self.assertEqual(
                (restored / manifest["containers"]["examples"]["path"]).read_bytes(),
                examples.read_bytes(),
            )
            reports = original / ".stkr/tests"
            reports.mkdir(parents=True)
            (reports / "failure.log").write_text("retained failure")
            packages.export(original)
            self.assertEqual(envelope.read_bytes(), immutable)
            with zipfile.ZipFile(
                original / "dist" / f"harmos-tests-{packages.platform()}.zip"
            ) as files:
                self.assertIn("tests/failure.log", files.namelist())
                self.assertFalse(
                    any(
                        name.startswith(("archives/", "examples/", "cli/", "fixtures/"))
                        for name in files.namelist()
                    )
                )

    def test_archive_substitution(self) -> None:
        for damage in ("member", "symlink"):
            with (
                self.subTest(damage=damage),
                tempfile.TemporaryDirectory() as directory,
            ):
                if damage == "symlink" and os.name == "nt":
                    continue
                root = Path(directory) / "repository"
                self._repository(root)
                self._package(root)
                package = packages.location(root)
                if damage == "symlink":
                    packages.export(root)
                    shutil.rmtree(package)
                    outside = Path(directory) / "outside"
                    outside.mkdir()
                    marker = outside / "valuable"
                    marker.write_bytes(b"preserved")
                    package.symlink_to(outside, target_is_directory=True)
                    with self.assertRaisesRegex(RuntimeError, "symlink"):
                        packages.restore(root)
                    self.assertEqual(set(outside.iterdir()), {marker})
                    self.assertEqual(marker.read_bytes(), b"preserved")
                    continue
                report = package / "reports/build.json"
                manifest = json.loads(report.read_text())
                container = manifest["containers"]["tool"]
                executable = next(
                    name
                    for name in container["members"]
                    if name.startswith("cli/cargo-harmos")
                )
                original = (package / executable).read_bytes()
                member = container["members"][executable]
                archive = package / container["path"]
                rewritten = io.BytesIO()
                if archive.suffix == ".zip":
                    with (
                        zipfile.ZipFile(archive) as source,
                        zipfile.ZipFile(rewritten, "w") as output,
                    ):
                        for entry in source.infolist():
                            content = (
                                b"corrupt executable"
                                if entry.filename == member
                                else source.read(entry)
                            )
                            output.writestr(entry, content)
                else:
                    with (
                        packages.tarfile.open(archive, "r:gz") as source,
                        packages.tarfile.open(fileobj=rewritten, mode="w:gz") as output,
                    ):
                        for entry in source.getmembers():
                            extracted = source.extractfile(entry)
                            assert extracted is not None
                            with extracted:
                                content = (
                                    b"corrupt executable"
                                    if entry.name == member
                                    else extracted.read()
                                )
                            entry.size = len(content)
                            output.addfile(entry, io.BytesIO(content))
                archive.write_bytes(rewritten.getvalue())
                container["sha256"] = packages._digest(archive)
                report.write_text(json.dumps(manifest))
                with self.assertRaisesRegex(RuntimeError, "member differs"):
                    packages.restore(root)
                self.assertEqual((package / executable).read_bytes(), original)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._repository(root)
            self._package(root)
            package = packages.location(root)
            report = package / "reports/build.json"
            manifest = json.loads(report.read_text())
            container = manifest["containers"]["examples"]
            archive = package / container["path"]
            original = archive.read_bytes()
            # An Extra Entry Is Refused Even If Its Outer Digest Was Restamped.
            with zipfile.ZipFile(archive, "a") as files:
                files.writestr("unrelated-payload", b"extra")
            container["sha256"] = packages._digest(archive)
            report.write_text(json.dumps(manifest))
            with self.assertRaisesRegex(RuntimeError, "unexpected members"):
                packages.verify(root)
            archive.write_bytes(original)
            container["sha256"] = packages._digest(archive)
            # Invented Schema Members Cannot Introduce Traversal Destinations.
            container["members"]["../outside"] = "unrelated-payload"
            report.write_text(json.dumps(manifest))
            with self.assertRaisesRegex(RuntimeError, "unrelated payloads"):
                packages.restore(root)
            self.assertFalse((package.parent / "outside").exists())

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
                root / "dist" / f"harmos-tests-{packages.platform()}.zip"
            ) as archive:
                self.assertFalse(
                    any(
                        name.startswith(("archives/", "examples/", "cli/"))
                        for name in archive.namelist()
                    )
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
            for entry in original["tests"].values():
                entry["coverage"] = False
            manifest.write_text(json.dumps(original))
            with patch.dict("os.environ", {"STKR_COVERAGE": "0"}):
                packages.validate(root)
                self.assertTrue(record.exists())
            with patch.dict("os.environ", {"STKR_COVERAGE": "1"}):
                with self.assertRaisesRegex(RuntimeError, "requested coverage"):
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
            downloadable = original / "dist" / f"harmos-ci-{packages.platform()}.zip"
            immutable = downloadable.read_bytes()
            container = io.BytesIO()
            with zipfile.ZipFile(container, "w") as archive:
                report = original / "dist" / f"harmos-tests-{packages.platform()}.zip"
                archive.write(report, report.name)
            build_container = io.BytesIO()
            with zipfile.ZipFile(build_container, "w") as archive:
                archive.write(downloadable, downloadable.name)
            proposed = Path(directory) / "proposed"
            shutil.copytree(
                original, proposed, ignore=shutil.ignore_patterns("dist", ".stkr")
            )
            revision = packages._output(original, ["git", "rev-parse", "HEAD"])
            actual_run = subprocess.run

            def run(command, **kwargs):
                if command[:2] == ["git", "fetch"]:
                    return subprocess.CompletedProcess(command, 0)
                return actual_run(command, **kwargs)

            actual_output = packages._output

            def output(repository, command):
                if command[:2] == ["git", "rev-parse"]:
                    return revision
                return actual_output(repository, command)

            pipeline = {
                "id": 42,
                "status": "completed",
                "conclusion": "success",
                "head_sha": revision,
                "path": ".github/workflows/full.yml",
                "run_attempt": 2,
            }
            jobs: list[dict[str, int | str | None]] = [
                {
                    "id": (99 if system == packages.platform() else 100)
                    if phase == "test"
                    else (88 if system == packages.platform() else 89)
                    if phase == "build:release"
                    else index + 1,
                    "name": f"{phase} ({system if phase == 'test' else system.title()})",
                    "conclusion": "success",
                }
                for index, (phase, system) in enumerate(
                    (phase, system)
                    for phase in ("Check Fast", "Build CI", "test")
                    for system in ("linux", "windows")
                )
            ]
            artifact = {
                "id": 123,
                "name": f"harmos-test-{packages.platform()}-2",
                "expired": False,
                "workflow_run": {"id": 42, "head_sha": revision},
                "digest": "sha256:"
                + packages.hashlib.sha256(container.getvalue()).hexdigest(),
            }

            build_artifact = artifact | {
                "id": 124,
                "name": f"harmos-build-{packages.platform()}-1",
                "digest": "sha256:"
                + packages.hashlib.sha256(build_container.getvalue()).hexdigest(),
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
                    return {"artifacts": [artifact, build_artifact]}
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
                        build_container.getvalue()
                        if "/124/" in args[0].full_url
                        else container.getvalue()
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
                if packages.platform() == "linux":
                    # Linux Dev Proof Is Independent Of Honest Windows Outcomes
                    pipeline.update(status="in_progress", conclusion=None, event="push")
                    jobs.append(
                        {
                            "id": 200,
                            "name": "check:merge-route",
                            "conclusion": "success",
                        }
                    )
                    for job in jobs:
                        if "(windows)" in str(job["name"]):
                            job["conclusion"] = None
                    with (
                        patch.dict(
                            "os.environ",
                            {
                                "GITHUB_EVENT_NAME": "push",
                                "GITHUB_REF": "refs/heads/dev",
                            },
                        ),
                    ):
                        packages.development_catalog(proposed)
                        catalog = proposed / "dist/development.json"
                        proof = json.loads(catalog.read_text())
                        self.assertEqual(proof["purpose"], "development-validation")
                        self.assertEqual(proof["platforms"], ["linux"])
                        self.assertEqual(proof["proof"]["revision"], revision)
                        self.assertEqual(len(proof["proof"]["artifacts"]), 2)
                        self.assertFalse(
                            any("windows" in name for name in proof["proof"]["jobs"])
                        )
                        for conclusion in (None, "failure", "success"):
                            for job in jobs:
                                if "(windows)" in str(job["name"]):
                                    job["conclusion"] = conclusion
                            packages.development_catalog(proposed)
                            self.assertEqual(
                                json.loads(catalog.read_text())["platforms"], ["linux"]
                            )
                        linux = next(
                            job for job in jobs if job["name"] == "test (linux)"
                        )
                        linux["conclusion"] = "failure"
                        with self.assertRaisesRegex(
                            RuntimeError, "native platform matrix"
                        ):
                            packages.development_catalog(proposed)
                        self.assertFalse(catalog.exists())
                        linux["conclusion"] = "success"
                        with patch.dict(
                            "os.environ", {"GITHUB_REF": "refs/heads/main"}
                        ):
                            with self.assertRaisesRegex(RuntimeError, "restricted"):
                                packages.development_catalog(proposed)
                        with self.assertRaisesRegex(RuntimeError, "has not succeeded"):
                            packages.reuse(proposed)
                    pipeline.update(status="completed", conclusion="success")
                    jobs.pop()
                    for job in jobs:
                        job["conclusion"] = "success"
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
                        {"id": 43, "head_sha": revision},
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
                        corrupt_bytes = corrupt.getvalue()
                        with patch(
                            "harmos_tools.packages.urlopen",
                            side_effect=lambda request, corrupt_bytes=corrupt_bytes, **kwargs: (
                                io.BytesIO(
                                    build_container.getvalue()
                                    if "/124/" in request.full_url
                                    else corrupt_bytes
                                )
                            ),
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
                    return_value="b" * 40,
                ):
                    with self.assertRaisesRegex(RuntimeError, "fresh build"):
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
            ["git", "init", "--initial-branch=dev", "--quiet", str(root)],
            check=True,
            env=packages._git_environment(),
        )
        (root / ".gitignore").write_text("dist/\n.stkr/\ntarget/\n.tmp/\n")
        (root / "input.txt").write_text("product source")
        (root / "LICENSE-APACHE").write_text("Apache License Fixture")
        (root / "README.md").write_text("# Test Product")
        (root / "examples/schematic").mkdir(parents=True)
        (root / "examples/schematic/README.md").write_text("# Test Example")
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
        artifacts = packages.artifact_paths()
        destination.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / "LICENSE-APACHE", destination / "LICENSE-APACHE")
        for relative in artifacts.values():
            path = destination / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"release asset")
        schemas = {}
        for producer in packages.SCHEMA_PRODUCERS:
            directory = destination / f"examples/schematic/grpc/{producer}/operations"
            directory.mkdir(parents=True, exist_ok=True)
            for name, content in {
                "descriptors.bin": b"descriptor",
                "schematic/v1/Schematic.proto": b'syntax = "proto3";',
                "manifest.txt": b"descriptors.bin\ncontracts.json\nschematic/v1/Schematic.proto\n",
                "contracts.json": b'{"schema":1,"contracts":[]}',
            }.items():
                path = directory / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(content)
                schemas[path.relative_to(destination).as_posix()] = packages._digest(
                    path
                )
        report = destination / "reports/build.json"
        report.parent.mkdir(parents=True, exist_ok=True)
        report.write_text(
            json.dumps(
                {
                    "schema": 2,
                    "profile": packages.build_profile(),
                    "platform": packages.platform(),
                    "version": "0.2.0",
                    "target": "x86_64-pc-windows-msvc"
                    if packages.platform() == "windows"
                    else "x86_64-unknown-linux-gnu",
                    "revision": packages._output(root, ["git", "rev-parse", "HEAD"]),
                    "built_unix_seconds": 1,
                    "run": "42",
                    "attempt": "1",
                    "job": f"build_ci_{packages.platform()}",
                    "source": packages.source(root),
                    "inputs": packages.release.inputs(root),
                    "artifacts": artifacts,
                    "schemas": schemas,
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
        packages.archive_payloads(root, destination)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    unittest.main()
