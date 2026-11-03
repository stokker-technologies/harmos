# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Prove Development Selection And Release Publication Boundaries."""

# === Imports === #

# Standard Library Imports
import copy
import hashlib
import itertools
import json
import os
import runpy
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.ci import impact, simulate
from harmos_tools.release import edge, layout, pins, policy, registry, run

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[3]

# === Types === #

# === Classes === #


class ImpactTests(unittest.TestCase):
    def test_dev_and_review(self):
        plan = simulate.PLAN["impact"]
        full = impact.select(plan, {"Cargo.toml"})
        development = impact.scope(plan, full, "package")
        self.assertTrue({"package:rust", "build:rust:tests"} <= development)
        self.assertFalse(any(job.startswith(("test:", "release:", "bench:", "prepare:")) for job in development))
        self.assertTrue(any(job.startswith("test:") for job in impact.scope(plan, full)))
        for job in development:
            self.assertTrue(set(plan["dependencies"][job]) <= development)
        action = runpy.run_path(str(ROOT / ".github/actions/plan-impact/run.py"))
        self.assertEqual(action["scope"](plan, action["select"](plan, {"Cargo.toml"}), "package"), development)
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        authored = next(line.removeprefix("        plan: ") for line in workflow.splitlines() if line.startswith("        plan: "))
        hosted_plan = json.loads(json.loads(authored))
        self.assertEqual(impact.scope(hosted_plan, impact.select(hosted_plan, {"Cargo.toml"}), "package"), development)
        print("Dev Impact:", ", ".join(sorted(development)))
        print("PR Impact:", ", ".join(sorted(full)))

    def test_guidance_scope(self):
        plan = simulate.PLAN["impact"]
        selected = impact.select(plan, {"CONTRIBUTING.md"})
        self.assertEqual(impact.scope(plan, selected, "package"), set())

    def test_conservative_scope(self):
        plan = {"dependencies": {"package:rust": ["producer"], "producer": []}, "rules": []}
        self.assertEqual(impact.close(plan["dependencies"], {"unknown"}), set(plan["dependencies"]))
        self.assertEqual(impact.scope(plan, {"package:rust"}, "package"), {"package:rust", "producer"})
        with self.assertRaises(ValueError):
            impact.scope(plan, set(), "typo")

    def test_pipeline_workloads(self):
        plan = simulate.PLAN
        full = impact.select(plan["impact"], None)
        for pipeline in ("dev", "pr", "nightly", "release"):
            with self.subTest(pipeline=pipeline):
                contexts = {pipeline, "pr"} if pipeline == "release" else {pipeline}
                selected = impact.scope(plan["impact"], full, "package" if pipeline == "dev" else "")
                jobs = [
                    job
                    for job in plan["jobs"]
                    if contexts.intersection(job["pipelines"])
                    and (not job["release_review_only"] or pipeline == "release")
                    and job["id"] in selected
                ]
                ids = {job["id"] for job in jobs}
                self.assertEqual("release:publish:dry" in ids, pipeline in ("nightly", "release"))
                self.assertEqual("test:rust:unit" in ids, pipeline != "dev")
                self.assertEqual("test:rust:starter" in ids, pipeline == "nightly")
                self.assertEqual("bench:rust:unit" in ids, pipeline == "nightly")
                for producer in ("build", "build:rust:tests", "package:rust"):
                    self.assertEqual({job["platform"] for job in jobs if job["id"] == producer}, {"unix", "windows"})
                keys = {job["key"] for job in jobs}
                for job in jobs:
                    dependencies = job.get("pipeline_needs", {}).get("release-review" if pipeline == "release" else pipeline, job["needs"])
                    self.assertTrue(set(dependencies) <= keys, (job["key"], set(dependencies) - keys))


class ReleaseChecksTests(unittest.TestCase):
    def setUp(self):
        self.guard = runpy.run_path(str(ROOT / ".github/actions/review-release/checks.py"))
        self.sha = "a" * 40
        self.ci_run: dict = {
            "id": 7,
            "check_suite_id": 9,
            "head_sha": self.sha,
            "head_branch": "dev",
            "event": "push",
            "status": "completed",
            "conclusion": "success",
        }
        self.gate: dict = {
            "id": 12,
            "name": "CI / Required",
            "app": {"slug": "github-actions", "id": 8},
            "check_suite": {"id": 9},
            "status": "completed",
            "conclusion": "success",
        }
        self.runs = [self.ci_run]
        self.checks = [self.gate]
        self.rules = []
        self.statuses = []

    def pages(self, endpoint, key=None):
        if endpoint.startswith("actions/"):
            return self.runs
        if "/check-runs?" in endpoint:
            return self.checks
        if endpoint == "rules/branches/main":
            return self.rules
        if endpoint.endswith("/statuses"):
            return self.statuses
        raise AssertionError(endpoint)

    def test_exact_commit(self):
        self.assertTrue(self.guard["green"](self.sha, self.pages))
        self.ci_run["head_sha"] = "b" * 40
        self.assertFalse(self.guard["green"](self.sha, self.pages))
        self.ci_run["head_sha"] = self.sha
        self.gate["check_suite"]["id"] += 1
        self.assertFalse(self.guard["green"](self.sha, self.pages))

    def test_latest_failure(self):
        self.runs.append({**self.ci_run, "id": self.ci_run["id"] + 1, "conclusion": "failure"})
        with self.assertRaises(ValueError):
            self.guard["green"](self.sha, self.pages)
        self.runs[-1]["status"] = "in_progress"
        self.assertFalse(self.guard["green"](self.sha, self.pages))

    def test_required_contexts(self):
        self.rules = [{"type": "required_status_checks", "parameters": {"required_status_checks": [{"context": "security"}]}}]
        self.assertFalse(self.guard["green"](self.sha, self.pages))
        self.statuses = [{"id": 1, "context": "security", "state": "success"}]
        self.assertTrue(self.guard["green"](self.sha, self.pages))
        self.statuses.append({"id": 2, "context": "security", "state": "failure"})
        with self.assertRaises(ValueError):
            self.guard["green"](self.sha, self.pages)
        self.rules[0]["parameters"]["required_status_checks"][0]["integration_id"] = 4
        self.statuses = []
        self.checks.append({**self.gate, "name": "security", "id": 13, "app": {"id": 5}})
        self.assertFalse(self.guard["green"](self.sha, self.pages))
        self.checks[-1]["app"]["id"] = 4
        self.assertTrue(self.guard["green"](self.sha, self.pages))
        self.checks[-1]["conclusion"] = "neutral"
        with self.assertRaises(ValueError):
            self.guard["green"](self.sha, self.pages)

    def test_wait_and_timeout(self):
        self.runs = []

        def completed(_):
            self.runs = [self.ci_run]

        self.guard["wait"](self.sha, self.pages, sleep=completed, clock=lambda: 0)
        self.runs = []
        times = iter([0, 100000])
        with self.assertRaises(ValueError):
            self.guard["wait"](self.sha, self.pages, sleep=lambda _: None, clock=lambda: next(times))


class PinTests(unittest.TestCase):
    def test_release_and_nightly(self):
        packages = [
            {"name": "direct", "source": "git+https://example.test/direct#123"},
            {"name": "transitive", "source": "git+https://example.test/transitive#456"},
        ]
        with patch.object(policy, "execute", return_value=json.dumps({"packages": packages})), patch.dict(os.environ, {}, clear=True):
            with self.assertRaises(policy.ToolError):
                pins.check(ROOT)
            self.assertFalse(pins.check(ROOT, warn=True))
        with patch.object(policy, "execute", return_value=json.dumps({"packages": [{"name": "local", "source": None}]})):
            self.assertTrue(pins.check(ROOT))

    def test_nightly_skips(self):
        with (
            patch.object(pins, "check", return_value=False) as check,
            patch.dict(os.environ, {"GITHUB_WORKFLOW_REF": "owner/repo/.github/workflows/nightly.yml@refs/heads/dev"}, clear=True),
            patch.object(registry.storage, "preflight") as preflight,
        ):
            registry.rehearse_crates(ROOT)
            self.assertTrue(check.call_args.kwargs["warn"])
            preflight.assert_not_called()


class EdgeTests(unittest.TestCase):
    def test_publish_lookup(self):
        publishers = (edge._github,)
        faults = (
            None,
            "missing",
            "id",
            "tag_name",
            "target_commitish",
            "draft",
            "prerelease",
            "missing-asset",
            "extra-asset",
            "duplicate-asset",
            "wrong-size",
            "pending-asset",
        )
        for publisher, missing_release, fault in itertools.product(publishers, (False, True), faults):
            with self.subTest(publisher=publisher.__module__, missing_release=missing_release, fault=fault):
                with tempfile.TemporaryDirectory() as temporary:
                    directory = Path(temporary)
                    data = b"checksum"
                    (directory / "SHA256SUMS").write_bytes(data)
                    files = [{"name": "SHA256SUMS", "path": "SHA256SUMS", "size": len(data), "sha256": hashlib.sha256(data).hexdigest()}]
                    release = {} if missing_release else {"id": 17, "tag_name": edge.TAG, "draft": False}
                    assets = []
                    published = False
                    verified = []

                    def document(settings, endpoint, method="GET", body=None, **kwargs):
                        nonlocal published
                        if endpoint.startswith("actions/"):
                            return {"workflow_runs": []}
                        if endpoint == "releases/tags/edge":
                            if published:
                                verified.append(endpoint)
                                if fault == "missing":
                                    return None
                                result = copy.deepcopy(release)
                                changes = {
                                    "id": 99,
                                    "tag_name": "untagged-placeholder",
                                    "target_commitish": "b" * 40,
                                    "draft": True,
                                    "prerelease": False,
                                }
                                if fault in changes:
                                    result[fault] = changes[fault]
                                return result if result["tag_name"] == edge.TAG else None
                            return copy.deepcopy(release) if release else None
                        if endpoint == "releases" and method == "POST":
                            release.update({"id": 17, **body, "tag_name": "untagged-placeholder"})
                            return copy.deepcopy(release)
                        if endpoint == "releases/17" and method == "PATCH":
                            release.update(body)
                            if body.get("draft") is True:
                                release["tag_name"] = "untagged-placeholder"
                            if body.get("draft") is False:
                                self.assertEqual(body["tag_name"], edge.TAG)
                                self.assertEqual(body["target_commitish"], "a" * 40)
                                self.assertTrue(body["prerelease"])
                                self.assertEqual(body["make_latest"], "false")
                                published = True
                            return copy.deepcopy(release)
                        if endpoint == "git/ref/tags/edge":
                            return None if missing_release else {"object": {"sha": "b" * 40}}
                        if endpoint.startswith("git/"):
                            return {}
                        raise AssertionError((endpoint, method))

                    def pages(settings, endpoint, **kwargs):
                        if endpoint == "releases":
                            return [copy.deepcopy(release)] if release else []
                        self.assertEqual(endpoint, "releases/17/assets")
                        result = copy.deepcopy(assets)
                        if not published:
                            return result
                        if fault == "missing-asset":
                            return []
                        if fault == "extra-asset":
                            result.append({"id": 99, "name": "obsolete.zip", "size": 3, "state": "uploaded"})
                        if fault == "duplicate-asset":
                            result.append(copy.deepcopy(result[0]))
                        if fault == "wrong-size":
                            result[0]["size"] += 1
                        if fault == "pending-asset":
                            result[0]["state"] = "new"
                        return result

                    def request(settings, endpoint, method, data=None, **kwargs):
                        if method == "DELETE":
                            assets.clear()
                            return
                        assets.append(
                            {
                                "id": 2,
                                "name": "SHA256SUMS",
                                "size": len(data),
                                "state": "uploaded",
                                "digest": "sha256:" + hashlib.sha256(data).hexdigest(),
                            }
                        )

                    with (
                        patch.object(registry, "document", side_effect=document),
                        patch.object(registry, "pages", side_effect=pages),
                        patch.object(registry, "request", side_effect=request),
                    ):
                        if fault:
                            with self.assertRaisesRegex(policy.ToolError, "edge"):
                                publisher(directory, {}, files, "a" * 40, "7", "notes")
                        else:
                            publisher(directory, {}, files, "a" * 40, "7", "notes")
                    self.assertEqual(verified, ["releases/tags/edge"])

    def test_atomic_replacement(self):
        for failing_upload, missing_release in itertools.product(
            (None, "linux.zip", "windows.zip", "SHA256SUMS", "corrupt", "missing", "no-digest"), (False, True)
        ):
            with self.subTest(failing_upload=failing_upload, missing_release=missing_release), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                files = []
                for name in ("linux.zip", "windows.zip", "SHA256SUMS"):
                    data = name.encode()
                    (directory / name).write_bytes(data)
                    files.append({"name": name, "path": name, "size": len(data), "sha256": hashlib.sha256(data).hexdigest()})
                release: dict = {} if missing_release else {"id": 17, "draft": False, "body": "", "tag_name": edge.TAG}
                assets: list[dict] = [{"id": 1, "name": "obsolete.zip", "size": 3}]
                tag_moves = []
                published = []

                def document(
                    settings,
                    endpoint,
                    method="GET",
                    body=None,
                    *,
                    release=release,
                    assets=assets,
                    files=files,
                    tag_moves=tag_moves,
                    published=published,
                    missing_release=missing_release,
                    **kwargs,
                ):
                    if endpoint.startswith("actions/"):
                        return {"workflow_runs": []}
                    if endpoint == "releases/tags/edge":
                        return None if not release or release["draft"] else copy.deepcopy(release)
                    if endpoint == "releases" and method == "POST":
                        assert isinstance(body, dict)
                        self.assertEqual(body["tag_name"], "edge")
                        self.assertTrue(body["draft"])
                        release.update({"id": 17, **body})
                        return copy.deepcopy(release)
                    if endpoint == "git/ref/tags/edge":
                        return None if missing_release else {"object": {"sha": "b" * 40}}
                    if (endpoint == "git/refs/tags/edge" and method == "PATCH") or (endpoint == "git/refs" and method == "POST"):
                        assert isinstance(body, dict)
                        self.assertTrue(release["draft"])
                        self.assertEqual({item["name"] for item in assets}, {item["name"] for item in files})
                        if method == "POST":
                            self.assertEqual(body["ref"], "refs/tags/edge")
                        tag_moves.append(body["sha"])
                        return {}
                    if endpoint == "releases/17" and method == "PATCH":
                        assert isinstance(body, dict)
                        if body.get("draft") is False:
                            published.append(copy.deepcopy(assets))
                        release.update(body)
                        return copy.deepcopy(release)
                    raise AssertionError((endpoint, method))

                failure = [failing_upload]

                def request(settings, endpoint, method="GET", data=None, *, release=release, assets=assets, failure=failure, **kwargs):
                    self.assertTrue(release["draft"], "asset mutation exposed a partial public channel")
                    if method == "GET":
                        asset = next(item for item in assets if endpoint.endswith("/" + str(item["id"])))
                        return asset["name"].encode()
                    if method == "DELETE":
                        assets.clear()
                        return
                    name = endpoint.split("name=")[1]
                    if name == failure[0]:
                        raise policy.ToolError("upload interrupted")
                    assert isinstance(data, bytes)
                    assets.append(
                        {
                            "id": len(assets) + 2,
                            "name": name,
                            "size": len(data),
                            "state": "uploaded",
                            "digest": None if failure[0] == "no-digest" else "sha256:" + hashlib.sha256(data).hexdigest(),
                        }
                    )

                def pages(settings, endpoint, *, release=release, assets=assets, failure=failure, **kwargs):
                    if endpoint == "releases":
                        return [copy.deepcopy(release)] if release else []
                    result = copy.deepcopy(assets)
                    if len(result) == 3 and failure[0] == "corrupt":
                        result[-1]["digest"] = "sha256:" + "0" * 64
                    if len(result) == 3 and failure[0] == "missing":
                        result.pop()
                    return result

                with (
                    patch.object(registry, "document", side_effect=document),
                    patch.object(registry, "pages", side_effect=pages),
                    patch.object(registry, "request", side_effect=request),
                ):
                    if failing_upload and failing_upload != "no-digest":
                        with self.assertRaises(policy.ToolError):
                            edge._github(directory, {}, files, "a" * 40, "7", "notes")
                        self.assertTrue(release["draft"])
                        self.assertEqual(tag_moves, [])
                        self.assertEqual(published, [])
                        failure[0] = None
                    edge._github(directory, {}, files, "a" * 40, "7", "notes")
                self.assertFalse(release["draft"])
                self.assertEqual(tag_moves, ["a" * 40])
                self.assertEqual(len(published), 1)
                self.assertEqual({item["name"] for item in published[0]}, {item["name"] for item in files})


class CatalogTests(unittest.TestCase):
    def test_distribution_inventory(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            root = directory / run.ASSETS
            shared = root / run.SHARED
            shared.mkdir(parents=True)
            (shared / "CHANGELOG.md").write_bytes(b"notes")
            (shared / run.SUMS).write_bytes(b"initial")
            layout.upload_manifest(directory, root, [run._describe(root, path) for path in shared.iterdir()])
            targets = ["linux-x86_64", "windows-x86_64"]
            for target in targets:
                packaged = directory / "target/package/dev" / target
                assets = {}
                for relative in (f"rust/tool-{target}.zip", f"rust/schematic-{target}.zip", "producer/plugin.harmos"):
                    source = packaged / relative
                    source.parent.mkdir(parents=True, exist_ok=True)
                    data = (target + relative).encode()
                    source.write_bytes(data)
                    assets[relative] = {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
                (packaged / "catalog.json").write_text(json.dumps({"profile": "dev", "assets": assets}), encoding="utf-8")
            edge._include_packages(directory, targets)
            files = layout.uploads(directory, staged=True)
            names = {item["name"] for item in files}
            expected = {"CHANGELOG.md", "SHA256SUMS"}
            for target in targets:
                expected.update({f"tool-{target}.zip", f"schematic-{target}.zip", f"{target}-plugin.harmos"})
            self.assertEqual(names, expected)
            inventory = {item["name"]: item for item in files}
            run._checksums(shared / run.SUMS, inventory)
            source.write_bytes(b"changed")
            with self.assertRaises(policy.ToolError):
                edge._include_packages(directory, targets)

    def test_missing_platform(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            with patch.object(layout, "uploads", return_value=[]), self.assertRaises(FileNotFoundError):
                edge._include_packages(directory, ["windows-x86_64"])


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    unittest.main()
