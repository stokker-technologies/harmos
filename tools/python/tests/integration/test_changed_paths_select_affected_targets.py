# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Select Changed Stories And Product Consumers Without Building Other Targets."""

# === Imports === #

# Standard Library Imports
import tempfile
import unittest
from pathlib import Path

# Third Party Imports

# Local Imports
from harmos_tools.test import focused, policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class TestSelection(unittest.TestCase):
    def test_changed_story_support_and_product_select_their_consumers(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary).resolve()
            packages = []
            for name, dependencies in (("model", []), ("host", ["model"]), ("other", [])):
                root = directory / name
                (root / "tests/integration").mkdir(parents=True)
                (root / "tests/system").mkdir()
                (root / "tests/support").mkdir()
                (root / "tests/integration/main.rs").write_text('#[path = "../support/fixture.rs"]\nmod fixture;\nmod save_and_reopen;\n')
                (root / "tests/system/main.rs").write_text('#[path = "../support/fixture.rs"]\nmod fixture;\n')
                (root / "tests/support/fixture.rs").write_text("")
                (root / "tests/integration/save_and_reopen.rs").write_text("")
                targets = [
                    {"kind": ["lib"], "name": name, "test": True, "src_path": str(root / "src/lib.rs")},
                    {"kind": ["test"], "name": "integration", "src_path": str(root / "tests/integration/main.rs")},
                    {"kind": ["test"], "name": "system", "src_path": str(root / "tests/system/main.rs")},
                ]
                packages.append(
                    {
                        "id": name,
                        "name": name,
                        "manifest_path": str(root / "Cargo.toml"),
                        "dependencies": [{"name": dependency, "path": str(directory / dependency)} for dependency in dependencies],
                        "targets": targets,
                    }
                )
            metadata = {"packages": packages, "workspace_members": [item["id"] for item in packages]}

            # Select One Story, Then Every Target Sharing Its Fixture.
            self.assertEqual(
                focused.select(directory, metadata, [Path("model/tests/integration/save_and_reopen.rs")]), {"model": ["integration"]}
            )
            self.assertEqual(
                focused.select(directory, metadata, [Path("model/tests/support/fixture.rs")]), {"model": ["integration", "system"]}
            )

            # A Cross-Package Module Remains Owned By Every Consumer.
            (directory / "host/tests/integration/main.rs").write_text('#[path = "../../../model/tests/support/fixture.rs"]\nmod fixture;\n')
            self.assertEqual(
                focused.select(directory, metadata, [Path("model/tests/support/fixture.rs")]),
                {"host": ["integration"], "model": ["integration", "system"]},
            )

            # Path-Attributed Modules May Declare Sibling Modules.
            (directory / "model/tests/support/fixture.rs").write_text("mod nested;\n")
            (directory / "model/tests/support/nested.rs").write_text("")
            self.assertEqual(
                focused.select(directory, metadata, [Path("model/tests/support/nested.rs")]),
                {"host": ["integration"], "model": ["integration", "system"]},
            )

            # Product Changes Reach Local Consumers, Including Development Dependencies.
            self.assertEqual(
                focused.select(directory, metadata, [Path("model/src/lib.rs")]),
                {"host": ["integration", "lib", "system"], "model": ["integration", "lib", "system"]},
            )
            self.assertEqual(set(focused.select(directory, metadata, [Path("Cargo.lock")])), {"model", "host", "other"})

            # Compiler Contracts Consume Prepared Libraries Outside Cargo's Graph.
            helper = directory / "crates/harmos-runtime/tests/support/fixtures.rs"
            helper.parent.mkdir(parents=True)
            helper.write_text("")
            (directory / "other/tests/integration/main.rs").write_text(
                '#[path = "../../../crates/harmos-runtime/tests/support/fixtures.rs"]\nmod fixtures;\n'
            )
            self.assertEqual(
                focused.select(directory, metadata, [Path("model/src/lib.rs")]),
                {"host": ["integration", "lib", "system"], "model": ["integration", "lib", "system"], "other": ["integration"]},
            )

            # An Excluded Fixture Workspace Must Not Masquerade As Its Parent Package.
            nested = directory / "model/tests/fixtures/guest"
            nested.mkdir(parents=True)
            (nested / "Cargo.toml").write_text("")
            with self.assertRaises(policy.TestError):
                focused.select(directory, metadata, [nested / "src/lib.rs"])
            for path in (Path("README.md"), directory.parent / "outside.rs"):
                with self.assertRaises(policy.TestError):
                    focused.select(directory, metadata, [path])


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
