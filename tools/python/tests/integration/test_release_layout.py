# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Stage Both Release Channels From Clean Packages And Update Run-Owned Reports"""

# === Imports === #

# Standard Library Imports
import hashlib
import io
import json
import os
import shutil
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.release import layout, nightly, policy, reports, run

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class ReleaseLayout(unittest.TestCase):
    def test_clean_packages_publish_exact_channel_inventories_and_owned_reports(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            targets = {"linux-x86_64": "x86_64-unknown-linux-gnu", "windows-x86_64": "x86_64-pc-windows-msvc"}
            settings = {
                "product": "harmos",
                "provider": "github",
                "repository": "owner/repo",
                "targets": list(targets),
                "target_triples": targets,
            }
            proto = directory / "proto/harmos/v1"
            proto.mkdir(parents=True)
            (proto / "plugin.proto").write_text('syntax = "proto3";')
            (proto / "behaviour.md").write_text("Fixture Behaviour")
            for target, abi in targets.items():
                packaged = directory / "target/package/release" / target
                host = packaged / "rust" / layout.conformance_name(directory, settings, target, "1.2.3")
                host.parent.mkdir(parents=True)
                host.write_bytes(b"reference host archive")
                for name in ("NOTICE", "THIRD-PARTY-NOTICES", f"crates/{abi}/workspace.cdx.json", "python/any/sdk.cdx.json"):
                    output = packaged / "compliance" / name
                    output.parent.mkdir(parents=True, exist_ok=True)
                    output.write_text("{}")
            shared = directory / run.ASSETS / "any"
            core = {layout.conformance_name(directory, settings, target, "1.2.3") for target in targets}
            core.update({"harmos-proto-1.2.3.zip", "harmos-compliance-1.2.3.zip", "release.json", "SHA256SUMS"})
            with (
                patch.object(run, "release_tag", return_value="v1.2.3"),
                patch.object(run.version, "run"),
                patch.object(run, "_native", return_value="absent"),
            ):
                # Refuse A File At The Staging Root Before Creating The Clean Output Tree.
                release_root = directory / run.ASSETS
                release_root.write_bytes(b"wrong shape")
                with self.assertRaisesRegex(policy.ToolError, "regular directory"):
                    run.stage(directory, settings, "v1.2.3", list(targets), "release", release_notes="Fixture Notes")
                release_root.unlink()
                # Rehearse Both Channels On An Initially Absent Output Tree.
                for channel in ("stable", "nightly"):
                    run.dry(directory, settings, profile="release", require_all=True, release_notes="Fixture Notes", channel=channel)
                    uploads = layout.uploads(directory, staged=True)
                    expected = core | ({"harmos-reports-1.2.3.zip", "benchmarks.json"} if channel == "nightly" else set())
                    self.assertEqual({item["name"] for item in uploads}, expected)
                    document = json.loads((shared / "release.json").read_text())
                    self.assertEqual(set(document), {"notes", "pub_date", "downloads"})
                    manifest = document["downloads"]
                    self.assertEqual(set(manifest), {"version", "channel", "tag", "pub_date", "artifacts"})
                    self.assertEqual(
                        (manifest["version"], manifest["channel"], manifest["tag"]),
                        ("1.2.3", channel, "v1.2.3" if channel == "stable" else "nightly"),
                    )
                    self.assertEqual({row["name"] for row in manifest["artifacts"]}, expected - {"release.json", "SHA256SUMS"})
                    for row in manifest["artifacts"]:
                        self.assertEqual(set(row), {"kind", "os", "arch", "format", "name", "url", "size", "sha256"})
                        self.assertEqual(row["url"], f"https://github.com/owner/repo/releases/download/{manifest['tag']}/{row['name']}")
                    run._checksums(shared / "SHA256SUMS", {item["name"]: item for item in uploads})
                    with zipfile.ZipFile(shared / "harmos-proto-1.2.3.zip") as archive:
                        self.assertEqual(set(archive.namelist()), {"harmos/v1/plugin.proto", "harmos/v1/behaviour.md"})
                    with zipfile.ZipFile(shared / "harmos-compliance-1.2.3.zip") as archive:
                        self.assertEqual(len(archive.namelist()), 7)
                # A File Occupying A Required Directory Fails Locally.
                shutil.rmtree(shared)
                shared.write_bytes(b"wrong shape")
                with self.assertRaisesRegex(policy.ToolError, "regular directory"):
                    run.stage(directory, settings, "v1.2.3", list(targets), "release", release_notes="Fixture Notes")
                shared.unlink()
                report_root = directory / "target/nightly-reports"
                report_root.write_bytes(b"wrong shape")
                with self.assertRaises((policy.ToolError, NotADirectoryError)):
                    reports.collect(directory)
                report_root.unlink()
                run.stage(directory, settings, "v1.2.3", list(targets), "release", release_notes="Fixture Notes", channel="nightly")
            # Available Measurements And Failed Suites Update An Existing Publication.
            report_root = directory / "target/nightly-reports"
            coverage = report_root / "rust/coverage.xml"
            coverage.parent.mkdir(parents=True)
            coverage.write_text('<coverage line-rate="0.75"/>')
            measurements = reports.measurements([("read", 120.0, 100.0), ("first", 40.0, None)], "unit-rust", "a" * 40, 10)
            policy.write_json(report_root / "benchmarks-unit-rust.json", measurements)
            current = {"id": 9, "body": "| CI Run | [7](https://example.test/runs/7) |"}
            initial_manifest = (directory / run.ASSETS / "upload.json").read_bytes()
            remote = {item["name"]: (directory / item["path"]).read_bytes() for item in layout.uploads(directory)}

            def pages(settings, endpoint, **kwargs):
                return [
                    {
                        "id": name,
                        "name": name,
                        "size": len(data),
                        "state": "uploaded",
                        "digest": "sha256:" + hashlib.sha256(data).hexdigest(),
                    }
                    for name, data in remote.items()
                ]

            def request(settings, endpoint, method="GET", payload=None, **kwargs):
                name = endpoint.removeprefix("releases/assets/")
                if method == "DELETE":
                    del remote[name]
                    return None
                return remote[name]

            def document(settings, endpoint, method="GET", payload=None, **kwargs):
                if method == "PATCH":
                    assert payload is not None
                    current.update(payload)
                return current

            def upload(directory, settings, endpoint, item):
                remote[item["name"]] = (directory / item["path"]).read_bytes()

            with (
                patch.dict(os.environ, {"GITHUB_RUN_ID": "7"}),
                patch.object(nightly.policy, "policy", return_value=settings),
                patch.object(nightly.registry, "pages", side_effect=pages),
                patch.object(nightly.registry, "request", side_effect=request),
                patch.object(nightly.registry, "document", side_effect=document),
                patch.object(nightly, "_upload", side_effect=upload),
            ):
                for _ in range(2):
                    nightly.report(
                        directory, {"test": {"result": "failure"}, "coverage": {"result": "success"}}, "https://example.test/runs/7"
                    )
                    self.assertEqual(current["body"].count("<!-- nightly-results -->"), 1)
                    self.assertEqual(
                        json.loads(remote["benchmarks.json"]),
                        {"benchmarks": sorted(measurements["benchmarks"], key=lambda row: (row["suite"], row["name"]))},
                    )
                    with zipfile.ZipFile(io.BytesIO(remote["harmos-reports-1.2.3.zip"])) as archive:
                        self.assertEqual(archive.read("rust/coverage.xml"), coverage.read_bytes())
                        summary = json.loads(archive.read("summary.json"))
                        self.assertEqual(summary["coverage"][0]["line_percent"], 75.0)
                        self.assertEqual(summary["benchmarks"], measurements["benchmarks"])
                    inventory = layout.uploads(directory, staged=True)
                    run._checksums(shared / "SHA256SUMS", {item["name"]: item for item in inventory})
                # Verification Also Works After Reports Replace Their Initial Digests.
                (directory / run.ASSETS / "upload.json").write_bytes(initial_manifest)
                with (
                    patch.object(nightly, "_github_source", return_value=("a" * 40, "7", "https://example.test/runs/7")),
                    patch.object(nightly, "_tag", return_value="a" * 40),
                    patch.object(run, "verify_conformance") as native,
                ):
                    nightly.verify(directory)
                    native.assert_called_once()
                    core_name = "harmos-proto-1.2.3.zip"
                    original = remote[core_name]
                    remote[core_name] = b"changed"
                    with self.assertRaisesRegex(policy.ToolError, "differs from the publication inventory"):
                        nightly.verify(directory)
                    remote[core_name] = original
                previous = dict(remote)
                current["body"] = "| CI Run | [8](https://example.test/runs/8) |"
                nightly.report(directory, {}, "https://example.test/runs/7")
                self.assertEqual(remote, previous)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
