# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Rust SDK Caller Generation Stories"""

# === Imports === #

# Standard Library Imports
import tempfile
import unittest
from pathlib import Path

# Third Party Imports
from google.protobuf.descriptor_pb2 import FileDescriptorSet

# Local Imports
import harmos_tools.build.resource_roots
import harmos_tools.build.resources.descriptor
import harmos_tools.build.resources.rust

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class RustSdkCallers(unittest.TestCase):
    def setUp(self) -> None:
        self.root = Path(__file__).resolve().parents[4]
        self.descriptors = FileDescriptorSet.FromString((self.root / "crates/harmos-protocol/src/descriptors.bin").read_bytes())
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.output = Path(self.scratch.name)
        (self.output / "sdk/rust/src/connection").mkdir(parents=True)
        (self.output / "sdk/rust/src/client").mkdir()
        (self.output / "sdk/rust/src/plugin").mkdir()

    def test_client_and_context_share_settings_with_caller_specific_selectors(self) -> None:
        methods = harmos_tools.build.resources.descriptor.table(self.descriptors)
        harmos_tools.build.resources.rust.client(self.output, [method for method in methods if method["face"] == "client"])
        harmos_tools.build.resources.rust.host(self.output, [method for method in methods if method["face"] == "context"])
        self.assertFalse((self.output / "sdk/rust/src/connection/resources.rs").exists())
        client_path = self.output / "client.rs"
        harmos_tools.build.resource_roots.compose(
            self.root, [self.root / "sdk/rust/tests/fixtures/client/client-descriptors.bin"], client_path, "fixture", language="rust"
        )
        client = client_path.read_text()
        context = (self.output / "sdk/rust/src/plugin/host.rs").read_text()
        self.assertIn("pub async fn list(&self, name: impl Into<String>)", client)
        self.assertIn("pub async fn list(&self) -> Result<SettingsListResponse>", context)
        self.assertIn("pub name: Option<String>", context)
        self.assertIn("use harmos_protocol::host::plugins::SettingsListRequest;", client)
        self.assertIn("use harmos_protocol::host::plugins::SettingsListRequest;", context)
        self.assertNotIn("HostPluginsSettingsSet", context)
        self.assertNotIn("HostInvocationsCreate", context)
        self.assertNotIn("use harmos_protocol::host::StatusGetRequest;", context)
        self.assertIn("use harmos_protocol::host::StatusGetRequest;", client)
        self.assertIn("pub async fn create(", client)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
