# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Prove Impact Planning and Required Results"""

# === Imports === #

# Standard Library Imports
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.ci import docs, impact, required

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class ImpactPlanning(unittest.TestCase):
    def test_changed_components_select_their_consumers_and_producers(self) -> None:
        docs_only = {"docs"}
        rust = {
            "docs",
            "check",
            "build_product",
            "build_tests",
            "check_product",
            "package_rust",
            "prepare_test_fixtures",
            "test_unit",
            "test",
        }
        sdk = {"docs", "check", "build_product", "build_tests", "check_product", "prepare_test_fixtures", "test"}
        cpp = sdk - {"check_product"}
        cases = [
            (["README.md"], docs_only),
            (["sdk/python/README.md"], docs_only),
            (["docs/assets/theme.css"], docs_only),
            (["openspec/changes/example/specs/api/spec.md"], docs_only),
            (["crates/harmos-domain/src/lib.rs"], rust),
            (
                ["sdk/rust/tests/fixtures/sandbox/src/lib.rs"],
                {"check", "build_product", "build_tests", "prepare_test_fixtures", "test", "test_unit"},
            ),
            (["sdk/python/src/harmos/client.py"], sdk),
            (["sdk/python/uv.lock"], sdk),
            (["sdk/csharp/src/Harmos/Client.cs"], sdk),
            (["sdk/csharp/packages.lock.json"], sdk),
            (["sdk/cpp/include/harmos/client.hpp"], cpp),
            (["sdk/cpp/vcpkg.json"], cpp),
            (["proto/harmos/host/v1/time.proto"], rust),
            (["sdk/tests/fixtures/conformance/proto/conformance/model/v1/fixture.proto"], rust),
            (["mise.toml"], rust),
            (["Cargo.lock"], rust),
            (["tools/python/src/harmos_tools/task/check.py"], rust),
            ([".github/actions/setup/action.yml"], rust),
            ([".github/workflows/ci.yml"], rust),
            (["unexpected/new.input"], rust),
            (None, rust),
            ([], rust),
            (["README.md", "sdk/cpp/src/client.cpp"], cpp),
        ]
        for paths, expected in cases:
            with self.subTest(paths=paths):
                result = impact.plan(impact.classify(paths), "review")
                self.assertEqual(set(result["selected"]), expected)
                self.assertFalse(set(result["selected"]) & set(result["skipped"]))
                self.assertEqual(
                    {name for name, selected in ((name, value) for name, value in result.items() if isinstance(value, bool)) if selected},
                    expected,
                )
        original = ["sdk/python/src/harmos/client.py", "sdk/cpp/include/harmos/client.hpp"]
        self.assertEqual(impact.classify(original), impact.classify([*reversed(original), *original]))
        self.assertEqual(impact.plan(impact.classify(original), "review"), impact.plan(frozenset({"cpp", "python"}), "review"))

    def test_event_policy_limits_dev_and_expands_release_and_nightly(self) -> None:
        all_jobs = {
            "docs",
            "check",
            "build_product",
            "build_tests",
            "check_product",
            "package_rust",
            "prepare_test_fixtures",
            "test_unit",
            "test",
            "release_dry",
            "release_rehearsal",
        }
        cases = [
            ("push", {"docs", "check", "build_product"}),
            ("draft", set()),
            ("closed", set()),
            ("release", all_jobs),
            ("full", all_jobs),
            ("nightly", all_jobs),
        ]
        for event, expected in cases:
            with self.subTest(event=event):
                result = impact.plan(frozenset({"all"}), event)
                self.assertEqual(set(result["selected"]), expected)
        for event in ("release", "full", "nightly"):
            self.assertEqual(set(impact.plan(frozenset({"docs"}), event)["selected"]), all_jobs)
        self.assertEqual(impact.plan(frozenset({"docs"}), "push")["selected"], ["docs"])

    def test_selected_matrices_preserve_platform_pairs(self) -> None:
        for path in ("crates/harmos-domain/src/lib.rs", "sdk/python/uv.lock", "sdk/csharp/mise.toml", "sdk/cpp/vcpkg.json", "unknown"):
            selected = impact.plan(impact.classify([path]), "review")
            checks = selected["check_matrix"]["include"]
            languages = {row["language"] for row in checks} - {"repository"}
            for language in languages:
                self.assertEqual({row["platform"] for row in checks if row["language"] == language}, {"unix", "windows"})
            tests = selected["test_matrix"]["include"]
            for layer in {row["layer"] for row in tests}:
                self.assertEqual({row["platform"] for row in tests if row["layer"] == layer}, {"unix", "windows"})

    def test_provider_events_route_ready_draft_closed_and_release_reviews(self) -> None:
        cases = [
            ("push", {}, "push"),
            ("workflow_dispatch", {}, "full"),
            ("schedule", {}, "full"),
            ("pull_request", {"action": "opened", "pull_request": {}}, "review"),
            ("pull_request", {"action": "closed", "pull_request": {}}, "closed"),
            ("pull_request", {"pull_request": {"draft": True}}, "draft"),
            ("pull_request", {"pull_request": {"head": {"ref": "release/v1.0.0"}}}, "release"),
        ]
        for name, event, expected in cases:
            with self.subTest(name=name, event=event):
                self.assertEqual(impact._event(name, event), expected)

    def test_diff_parser_retains_deleted_and_both_renamed_paths(self) -> None:
        data = b"M\0docs/guide.md\0D\0sdk/python/deleted.py\0R100\0docs/old.md\0crates/new.rs\0C075\0sdk/cpp/a.cpp\0sdk/cpp/b.cpp\0"
        self.assertEqual(
            impact.diff_paths(data),
            ["docs/guide.md", "sdk/python/deleted.py", "docs/old.md", "crates/new.rs", "sdk/cpp/a.cpp", "sdk/cpp/b.cpp"],
        )
        for malformed in (b"", b"M\0truncated", b"R100\0old\0", b"Q\0path\0", b"M\0\0", b"M\0\xff\0"):
            with self.subTest(data=malformed), self.assertRaises(ValueError):
                impact.diff_paths(malformed)

    def test_diff_adapter_uses_merge_base_and_falls_back_on_incomplete_inputs(self) -> None:
        root = Path.cwd()
        before, after, base = "1" * 40, "2" * 40, "3" * 40
        review = {"pull_request": {"base": {"sha": before}, "head": {"sha": after}}}
        calls = []

        def git(command: list[str], **options: object) -> subprocess.CompletedProcess:
            calls.append(command)
            output = (base + "\n").encode("ascii") if command[1] == "merge-base" else b"D\0deleted.rs\0R100\0old.md\0new.rs\0"
            return subprocess.CompletedProcess(command, 0, output)

        for event, payload, comparison in [("pull_request", review, base), ("push", {"before": before, "after": after}, before)]:
            calls.clear()
            with self.subTest(event=event), patch.object(impact.subprocess, "run", side_effect=git):
                self.assertEqual(impact.changed_paths(payload, event, root), ["deleted.rs", "old.md", "new.rs"])
            self.assertEqual(calls[-1][-3:], [comparison, after, "--"])
            self.assertEqual(any(command[1] == "merge-base" for command in calls), event == "pull_request")
        for payload in ({}, {"before": "0" * 40, "after": after}, {"before": "bad", "after": after}):
            self.assertIsNone(impact.changed_paths(payload, "push", root))
        with patch.object(impact.subprocess, "run", side_effect=subprocess.CalledProcessError(1, ["git"])):
            self.assertIsNone(impact.changed_paths(review, "pull_request", root))
        for data in (b"", b"R100\0old\0"):
            with patch.object(impact.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, data)):
                self.assertIsNone(impact.changed_paths({"before": before, "after": after}, "push", root))

    def test_required_accepts_only_success_and_planned_skips(self) -> None:
        planned = impact.plan(impact.classify(["README.md"]), "review")
        passing = {name: {"result": "success" if name in planned["selected"] else "skipped"} for name in impact.JOBS}
        passing["impact"] = {
            "result": "success",
            "outputs": {"selected": json.dumps(planned["selected"]), "skipped": json.dumps(planned["skipped"])},
        }
        self.assertEqual(required.failures(passing), [])
        for job in passing:
            for result in ("failure", "cancelled", "skipped", "missing"):
                if job in planned["skipped"] and result == "skipped":
                    continue
                results = {name: dict(value) for name, value in passing.items()}
                if result == "missing":
                    del results[job]
                else:
                    results[job]["result"] = result
                with self.subTest(job=job, result=result):
                    self.assertEqual(required.failures(results), [job])
        for selected, skipped in [
            (["docs", "check"], planned["skipped"]),
            (["docs", "docs"], planned["skipped"]),
            (["docs"], []),
            ([], []),
        ]:
            results = {
                **passing,
                "impact": {"result": "success", "outputs": {"selected": json.dumps(selected), "skipped": json.dumps(skipped)}},
            }
            with self.subTest(selected=selected, skipped=skipped):
                self.assertEqual(required.failures(results), ["impact"])

    def test_cli_writes_complete_outputs_and_falls_back_without_diff_metadata(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        try:
            root = Path(scratch.name)
            payload, output = root / "event.json", root / "output"
            payload.write_text(json.dumps({"pull_request": {"draft": False}, "action": "opened"}), encoding="utf-8")
            environment = {"GITHUB_EVENT_NAME": "pull_request", "GITHUB_EVENT_PATH": str(payload), "GITHUB_OUTPUT": str(output)}
            with patch.dict(os.environ, environment):
                impact.main([])
            values = dict(line.split("=", 1) for line in output.read_text().splitlines())
            selected = json.loads(values["selected"])
            skipped = json.loads(values["skipped"])
            self.assertFalse(set(selected) & set(skipped))
            self.assertTrue(all(values[name] == "true" for name in selected))
            self.assertTrue(all(values[name] == "false" for name in skipped))
            self.assertEqual(values["test"], "true")
        finally:
            scratch.cleanup()

    def test_guide_and_link_checks_refuse_invalid_public_names_and_missing_targets(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        try:
            root = Path(scratch.name)
            (root / "docs").mkdir()
            (root / "README.md").write_text("# Product\n[Guide](docs/guide.md#usage)\n", encoding="utf-8")
            (root / "docs/guide.md").write_text("[Home](../README.md)\n[Web](https://example.test)\n", encoding="utf-8")
            self.assertEqual(docs.check_links(root, ["README.md", "docs/guide.md"]), [])
            self.assertEqual(docs.check_guides(root), [])
            (root / "docs/guide.md").write_text("import sample.v1.Client\n[Missing](missing.md)\n", encoding="utf-8")
            self.assertEqual(len(docs.check_guides(root)), 1)
            self.assertEqual(len(docs.check_links(root, ["docs/guide.md"])), 1)
        finally:
            scratch.cleanup()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
