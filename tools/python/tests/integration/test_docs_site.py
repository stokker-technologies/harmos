# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Prove Documentation Entries, Link Checks and Site Image Assembly."""

# === Imports === #

# Standard Library Imports
import json
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

# isort: split

# Third Party Imports

# isort: split

# Local Imports
from harmos_tools.docs import image, site
from harmos_tools.release import assets, policy

# Type Checking Imports


# === Public Exports === #

# === Constants === #

SETTINGS = {"product": "harmos", "provider": "github", "repository": "example/harmos"}

# === Types === #

# === Classes === #


class DocumentationSiteTests(unittest.TestCase):
    def test_stable_staging_requires_the_matching_entry_and_version(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        try:
            root = Path(scratch.name)
            shared = root / "staging/any"
            shared.mkdir(parents=True)
            with self.assertRaises(policy.ToolError):
                assets.documentation_archive(root, shared.parent, "v1.2.3", "stable")
            self.assertIsNone(assets.documentation_archive(root, shared.parent, "v1.2.3", "nightly"))
            destination = root / site.OUTPUT / site.ARCHIVE
            destination.parent.mkdir(parents=True)
            for entry, version in (("dev", "1.2.3"), ("1.2", "1.2.2"), ("1.2", "1.2.3")):
                destination.write_bytes(self.archive(root, entry, version).read_bytes())
                if (entry, version) != ("1.2", "1.2.3"):
                    with self.assertRaises(policy.ToolError):
                        assets.documentation_archive(root, shared.parent, "v1.2.3", "stable")
                    continue
                staged = assets.documentation_archive(root, shared.parent, "v1.2.3", "stable")
                assert staged is not None
                self.assertEqual(staged.read_bytes(), destination.read_bytes())
        finally:
            scratch.cleanup()

    def test_archive_is_reproducible_and_has_entry_relative_members(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        try:
            root = Path(scratch.name)
            output = root / "entry/guide"
            output.mkdir(parents=True)
            (output / "index.html").write_bytes(b"<!doctype html>")
            first, second = root / "first.zip", root / "second.zip"
            site.pack(output.parent, first)
            site.pack(output.parent, second)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            archive = zipfile.ZipFile(first)
            try:
                self.assertEqual(archive.namelist(), ["guide/index.html"])
            finally:
                archive.close()
        finally:
            scratch.cleanup()

    def test_assembly_refuses_archive_traversal(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        try:
            root = Path(scratch.name)
            path = self.archive(root, "dev", "1.2.3")
            archive = zipfile.ZipFile(path, "a")
            try:
                archive.writestr("../escape.html", "<!doctype html>")
            finally:
                archive.close()
            with self.assertRaises(policy.ToolError):
                image.assemble([path], root / "image")
            self.assertFalse((root / "image/escape.html").exists())
        finally:
            scratch.cleanup()

    def archive(self, root: Path, entry: str, version: str) -> Path:
        path = root / f"{entry}-{version}.zip"
        source = zipfile.ZipFile(path, "w")
        try:
            source.writestr(site.STAMP, json.dumps({"entry": entry, "version": version, "revision": "a" * 40}))
            source.writestr("index.html", "<!doctype html>")
            source.writestr("guide/index.html", "<!doctype html>")
        finally:
            source.close()
        return path

    def test_release_entry_is_the_release_line(self) -> None:
        with mock.patch.object(site.policy, "policy", return_value={"value": "1.12.3"}):
            self.assertEqual(site.entry(Path.cwd(), release=True), "1.12")
            self.assertEqual(site.entry(Path.cwd(), release=False), "dev")
        with mock.patch.object(site.policy, "policy", return_value={"value": "1.12.3-rc.1"}), self.assertRaises(policy.ToolError):
            site.entry(Path.cwd(), release=True)

    def test_links_report_missing_pages_and_headings_only(self) -> None:
        directory_directory = tempfile.TemporaryDirectory()
        directory = directory_directory.name
        try:
            output = Path(directory)
            (output / "guide").mkdir()
            (output / "guide/index.html").write_text('<h2 id="setup">Setup</h2>', encoding="utf-8")
            (output / "index.html").write_text(
                '<a href="/dev/guide/#setup"></a><a href="guide/"></a><a href="https://example.test/x"></a>'
                '<a href="/dev/guide/#missing"></a><a href="/dev/absent/"></a><a href="/0.1/guide/"></a>',
                encoding="utf-8",
            )
            broken = site.links(output, "/dev/")
            self.assertEqual(len(broken), 3)
            self.assertTrue(all(item.startswith("index.html: ") for item in broken))
        finally:
            directory_directory.cleanup()

    def test_assembly_orders_entries_and_redirects_latest_to_the_newest_line(self) -> None:
        directory_directory = tempfile.TemporaryDirectory()
        directory = directory_directory.name
        try:
            root = Path(directory)
            archives = [self.archive(root, "0.1", "0.1.4"), self.archive(root, "dev", "0.2.4"), self.archive(root, "0.2", "0.2.3")]
            output = root / "image"
            entries = image.assemble(archives, output)
            self.assertEqual([item["entry"] for item in entries], ["latest", "0.2", "0.1", "dev"])
            self.assertEqual(json.loads((output / "versions.json").read_text(encoding="utf-8")), entries)
            for entry in ("0.1", "0.2", "dev"):
                self.assertTrue((output / entry / "guide/index.html").is_file())
            self.assertIn('url=/0.2/guide/"', (output / "latest/guide/index.html").read_text(encoding="utf-8"))
            self.assertIn('url=/latest/"', (output / "index.html").read_text(encoding="utf-8"))
        finally:
            directory_directory.cleanup()

    def test_assembly_without_releases_lands_on_development(self) -> None:
        directory_directory = tempfile.TemporaryDirectory()
        directory = directory_directory.name
        try:
            root = Path(directory)
            entries = image.assemble([self.archive(root, "dev", "0.2.4")], root / "image")
            self.assertEqual([item["entry"] for item in entries], ["dev"])
            self.assertFalse((root / "image/latest").exists())
            self.assertIn('url=/dev/"', (root / "image/index.html").read_text(encoding="utf-8"))
        finally:
            directory_directory.cleanup()

    def test_assembly_refuses_duplicate_lines_and_foreign_entries(self) -> None:
        directory_directory = tempfile.TemporaryDirectory()
        directory = directory_directory.name
        try:
            root = Path(directory)
            cases = (
                [self.archive(root, "0.2", "0.2.3"), self.archive(root, "0.2", "0.2.2")],
                [self.archive(root, "0.3", "0.2.3")],
                [self.archive(root, "dev", "0.2.3"), self.archive(root, "dev", "0.2.4")],
            )
            for archives in cases:
                with self.subTest(archives=[path.name for path in archives]), self.assertRaises(policy.ToolError):
                    image.assemble(archives, root / "image")
        finally:
            directory_directory.cleanup()

    def test_download_selects_the_newest_stable_patch_per_line_and_the_nightly(self) -> None:
        def release(tag: str, *, prerelease: bool = False, draft: bool = False, name: str | None = None) -> dict:
            asset = name or f"harmos-docs-{tag.removeprefix('v')}.zip"
            return {"tag_name": tag, "prerelease": prerelease, "draft": draft, "assets": [{"name": asset}]}

        releases = [
            release("v0.2.3"),
            release("v0.2.10"),
            release("v0.3.0", prerelease=True),
            release("v0.4.0", draft=True),
            release("v0.1.7", name="harmos-0.1.7.zip"),
            release("v0.1.6"),
            release("nightly", prerelease=True, name="harmos-docs-nightly.zip"),
        ]
        directory_directory = tempfile.TemporaryDirectory()
        directory = directory_directory.name
        try:
            with (
                mock.patch.object(image.policy, "policy", return_value=SETTINGS),
                mock.patch.object(image.registry, "pages", return_value=releases),
                mock.patch.object(image.notices, "download", return_value=b"archive") as download,
            ):
                archives = image.download(Path(directory))
                self.assertEqual(
                    sorted(path.name for path in archives), ["harmos-docs-0.1.6.zip", "harmos-docs-0.2.10.zip", "harmos-docs-nightly.zip"]
                )
                self.assertEqual(download.call_count, 3)
        finally:
            directory_directory.cleanup()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    unittest.main()
