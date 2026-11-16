# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Nightly Publication Recovers Provider Failures."""

# === Imports === #

# Standard Library Imports
import contextlib
import copy
import hashlib
import tempfile
import unittest
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from unittest.mock import patch

from harmos_tools.release import nightly, policy, registry

# Third Party Imports
# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

OLD = "b" * 40

NEW = "a" * 40

LATER = "c" * 40


# === Types === #

# === Classes === #


class FakeGitHub:
    """An In-Memory Release Provider With GitHub's Draft, Tag And Asset Semantics And Injectable Faults."""

    def __init__(self, *, previous: bool, faults: set[str] | None = None, immutable: bool = False) -> None:
        self.faults = set(faults or ())
        self.immutable = immutable
        self.tag = OLD if previous else None
        self.dev = NEW
        self.workflows = {OLD: "previous-workflows", NEW: "build-workflows"}
        self.releases: dict[int, dict] = {}
        self.assets: dict[int, list[dict]] = {}
        self.data: dict[int, bytes] = {}
        self.next_id = 10
        if previous:
            self.releases[1] = {
                "id": 1,
                "name": nightly.TITLE,
                "tag_name": nightly.TAG,
                "target_commitish": OLD,
                "draft": False,
                "prerelease": True,
                "body": "",
            }
            self.assets[1] = [{"id": 1, "name": "old.zip", "size": 3, "state": "uploaded"}]

    def patched(self) -> contextlib.ExitStack:
        stack = contextlib.ExitStack()
        stack.enter_context(patch.object(registry, "document", side_effect=self.document))
        stack.enter_context(patch.object(registry, "pages", side_effect=self.pages))
        stack.enter_context(patch.object(registry, "request", side_effect=self.request))
        return stack

    def published(self) -> dict:
        (release,) = [item for item in self.releases.values() if item["tag_name"] == nightly.TAG and item["draft"] is False]
        return release

    def _fault(self, name: str, applies: bool = True, *, once: bool = True) -> None:
        if applies and name in self.faults:
            if once:
                self.faults.discard(name)
            raise policy.ToolError(f"injected {name}")

    def document(
        self, settings: dict, endpoint: str, method: str = "GET", body: dict | None = None, **kwargs: object
    ) -> dict | list[dict] | None:
        body = copy.deepcopy(body or {})
        if endpoint == f"releases/tags/{nightly.TAG}":
            matches = [item for item in self.releases.values() if item["tag_name"] == nightly.TAG and item["draft"] is False]
            self._fault("lookup", any(item["id"] >= 10 for item in matches))
            return copy.deepcopy(matches[0]) if matches else None
        if endpoint == "releases" and method == "POST":
            self._fault("create")
            identity, self.next_id = self.next_id, self.next_id + 1
            self.releases[identity] = {"id": identity, **body}
            self.assets[identity] = []
            return copy.deepcopy(self.releases[identity])
        if endpoint.startswith("releases/") and method == "PATCH":
            release = self.releases[int(endpoint.split("/")[1])]
            if release.get("immutable"):
                raise policy.ToolError("immutable release")
            if body.get("draft") is True:
                self._fault("draft-previous", release["id"] < 10)
                # GitHub Lists A Release Patched Back To Draft Under An Untagged Placeholder.
                release.update(body, tag_name=f"untagged-{release['id']}")
            else:
                self._fault("publish", release["id"] >= 10)
                others = (item for item in self.releases.values() if item["id"] != release["id"])
                if any(item["tag_name"] == body.get("tag_name") and not item["draft"] for item in others):
                    raise policy.ToolError("tag_name already_exists")
                release.update(body)
                if self.immutable:
                    release["immutable"] = True
            return copy.deepcopy(release)
        if endpoint == f"git/ref/tags/{nightly.TAG}":
            return {"object": {"sha": self.tag}} if self.tag else None
        if endpoint == "git/ref/heads/dev":
            return {"object": {"sha": self.dev}}
        if endpoint.startswith("contents/.github?ref="):
            revision = endpoint.split("=", 1)[1]
            return [
                {"name": "actions", "type": "dir", "sha": "actions"},
                {"name": "workflows", "type": "dir", "sha": self.workflows[revision]},
            ]
        if endpoint == "git/refs" and method == "POST":
            self._fault("tag-create", body["sha"] == NEW)
            if self.tag is not None:
                raise policy.ToolError("Reference already exists")
            self.tag = body["sha"]
            return {}
        raise AssertionError((endpoint, method))

    def pages(self, settings: dict, endpoint: str, **kwargs: object) -> list[dict]:
        if endpoint == "releases":
            return copy.deepcopy(list(self.releases.values()))
        assets = copy.deepcopy(self.assets[int(endpoint.split("/")[1])])
        for fault, change in (("size", {"size": 0}), ("pending", {"state": "new"}), ("digest", {"digest": "sha256:" + "0" * 64})):
            if fault in self.faults and assets:
                assets[0].update(change)
        if "no-digest" in self.faults:
            for asset in assets:
                asset.pop("digest", None)
        return assets

    def request(self, settings: dict, endpoint: str, method: str = "GET", data: object = None, **kwargs: object) -> bytes | None:
        if endpoint == f"git/refs/tags/{nightly.TAG}" and method == "DELETE":
            self.tag = None
            return
        if endpoint.startswith("releases/assets/") and method == "GET":
            identity = int(endpoint.rsplit("/", 1)[1])
            published = any(
                not release["draft"] and any(asset["id"] == identity for asset in self.assets[release["id"]])
                for release in self.releases.values()
            )
            if "bytes" in self.faults or "public-bytes" in self.faults and published:
                return b"!" * len(self.data[identity])
            return self.data[identity]
        if endpoint.startswith("releases/assets/") and method == "DELETE":
            identity = int(endpoint.rsplit("/", 1)[1])
            for assets in self.assets.values():
                assets[:] = [asset for asset in assets if asset["id"] != identity]
            return
        if method == "DELETE":
            identity = int(endpoint.split("/")[1])
            del self.releases[identity]
            del self.assets[identity]
            return
        release, name = int(endpoint.split("/")[1]), urllib.parse.unquote(endpoint.split("name=")[1])
        # A Persistent Upload Fault Outlasts Every Retry.
        self._fault(f"upload:{name}", once=False)
        assert isinstance(data, bytes)
        self.assets[release].append(
            {
                "id": self.next_id,
                "name": name,
                "size": len(data),
                "state": "uploaded",
                "digest": "sha256:" + hashlib.sha256(data).hexdigest(),
            }
        )
        self.data[self.next_id] = data
        self.next_id += 1
        # The Upload Lands But The Response Is Lost, Leaving A Same-Name Asset For The Retry To Replace.
        self._fault(f"partial:{name}")
        return


class PublicationTests(unittest.TestCase):
    def test_publication_replaces_previous(self) -> None:
        for previous, metadata in ((True, True), (False, True), (True, False)):
            with self.subTest(previous=previous, metadata=metadata):
                temporary_directory = tempfile.TemporaryDirectory()
                temporary = temporary_directory.name
                try:
                    directory, files = _staged_files(Path(temporary))
                    github = FakeGitHub(previous=previous, faults=set() if metadata else {"no-digest"})
                    github.releases[3] = {"id": 3, "name": "v0.1.2", "tag_name": "v0.1.2", "draft": False, "prerelease": False}
                    github.releases[4] = {"id": 4, "name": nightly.TITLE, "tag_name": "untagged-4", "draft": True, "prerelease": True}
                    github.assets.update({3: [], 4: []})
                    with github.patched():
                        nightly._github(directory, {}, files, NEW, "7", "notes")
                    published = github.published()
                    self.assertEqual(published["target_commitish"], NEW)
                    self.assertTrue(published["prerelease"])
                    self.assertEqual(github.tag, NEW)
                    self.assertEqual({asset["name"] for asset in github.assets[published["id"]]}, {item["name"] for item in files})
                    self.assertEqual(set(github.releases), {3, published["id"]})
                finally:
                    temporary_directory.cleanup()

    def test_failure_before_swap_keeps_previous(self) -> None:
        for fault in ("create", "upload:linux.zip", "upload:SHA256SUMS", "size", "pending", "digest", "bytes"):
            with self.subTest(fault=fault):
                temporary_directory = tempfile.TemporaryDirectory()
                temporary = temporary_directory.name
                try:
                    directory, files = _staged_files(Path(temporary))
                    github = FakeGitHub(previous=True, faults={fault})
                    before = copy.deepcopy((github.releases, github.assets, github.tag))
                    with github.patched(), self.assertRaises(policy.ToolError):
                        nightly._github(directory, {}, files, NEW, "7", "notes")
                    self.assertEqual((github.releases, github.assets, github.tag), before)
                finally:
                    temporary_directory.cleanup()

    def test_transient_upload_failure_retries(self) -> None:
        temporary_directory = tempfile.TemporaryDirectory()
        temporary = temporary_directory.name
        try:
            directory, files = _staged_files(Path(temporary))
            github = FakeGitHub(previous=True, faults={"partial:windows.zip"})
            with github.patched():
                nightly._github(directory, {}, files, NEW, "7", "notes")
            names = [asset["name"] for asset in github.assets[github.published()["id"]]]
            self.assertEqual(sorted(names), sorted(item["name"] for item in files))
        finally:
            temporary_directory.cleanup()

    def test_failure_during_swap_restores_previous(self) -> None:
        for fault in ("tag-create", "draft-previous", "publish", "lookup", "public-bytes"):
            with self.subTest(fault=fault):
                temporary_directory = tempfile.TemporaryDirectory()
                temporary = temporary_directory.name
                try:
                    directory, files = _staged_files(Path(temporary))
                    github = FakeGitHub(previous=True, faults={fault})
                    before = copy.deepcopy((github.releases, github.assets, github.tag))
                    with github.patched(), self.assertRaises(policy.ToolError):
                        nightly._github(directory, {}, files, NEW, "7", "notes")
                    self.assertEqual((github.releases, github.assets, github.tag), before)
                finally:
                    temporary_directory.cleanup()

    def test_superseded_and_immutable_previous(self) -> None:
        for state in ("superseded", "immutable"):
            with self.subTest(state=state):
                temporary_directory = tempfile.TemporaryDirectory()
                temporary = temporary_directory.name
                try:
                    directory, files = _staged_files(Path(temporary))
                    github = FakeGitHub(previous=True)
                    if state == "superseded":
                        github.releases[1]["body"] = "| CI Run | [9](https://example.invalid/9) |"
                    else:
                        github.releases[1]["immutable"] = True
                    before = copy.deepcopy((github.releases, github.assets, github.tag))
                    with github.patched():
                        if state == "superseded":
                            nightly._github(directory, {}, files, NEW, "7", "notes")
                        else:
                            with self.assertRaisesRegex(policy.ToolError, "Enable release immutability"):
                                nightly._github(directory, {}, files, NEW, "7", "notes")
                    self.assertEqual((github.releases, github.assets, github.tag), before)
                finally:
                    temporary_directory.cleanup()

    def test_dev_workflow_change_supersedes(self) -> None:
        for workflows, published in (("changed", False), ("same", True)):
            with self.subTest(workflows=workflows):
                temporary_directory = tempfile.TemporaryDirectory()
                temporary = temporary_directory.name
                try:
                    directory, files = _staged_files(Path(temporary))
                    github = FakeGitHub(previous=True)
                    github.dev = LATER
                    github.workflows[LATER] = "changed-workflows" if workflows == "changed" else github.workflows[NEW]
                    before = copy.deepcopy((github.releases, github.assets, github.tag))
                    with github.patched():
                        nightly._github(directory, {}, files, NEW, "7", "notes")
                    if published:
                        self.assertEqual(github.published()["target_commitish"], NEW)
                    else:
                        self.assertEqual((github.releases, github.assets, github.tag), before)
                finally:
                    temporary_directory.cleanup()

    def test_repository_immutability_is_reported(self) -> None:
        temporary_directory = tempfile.TemporaryDirectory()
        temporary = temporary_directory.name
        try:
            directory, files = _staged_files(Path(temporary))
            github = FakeGitHub(previous=True, immutable=True)
            with github.patched(), self.assertRaisesRegex(policy.ToolError, "Enable release immutability"):
                nightly._github(directory, {}, files, NEW, "7", "notes")
            self.assertEqual(github.published()["target_commitish"], NEW)
        finally:
            temporary_directory.cleanup()


# === Public Functions === #

# === Private Functions === #


def _staged_files(directory: Path) -> tuple[Path, list[dict]]:
    files = []
    for name in ("linux.zip", "windows.zip", "SHA256SUMS"):
        data = name.encode()
        (directory / name).write_bytes(data)
        files.append({"name": name, "path": name, "size": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    return directory, files


# === Entry Point === #
