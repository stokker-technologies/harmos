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
from harmos_tools.build.resource_roots import rust, rust_host, table
from harmos_tools.source.protocol import narrow_callers

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
        methods = table(self.descriptors)
        rust(self.output, [method for method in methods if method["face"] == "client"])
        rust_host(self.output, [method for method in methods if method["face"] == "context"])
        client = (self.output / "sdk/rust/src/connection/resources.rs").read_text()
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

    def test_narrowing_removes_only_the_disallowed_rust_surface(self) -> None:
        for callers, absent, present in [("PLUGINS", "client", "context"), ("CLIENTS", "context", "client")]:
            descriptors = FileDescriptorSet.FromString(self.descriptors.SerializeToString())
            narrow_callers(descriptors, {"harmos.host.v1.Time/Get": callers})
            methods = table(descriptors)
            matching = [method["face"] for method in methods if method["request"] == "harmos.host.v1.TimeGetRequest"]
            self.assertNotIn(absent, matching)
            self.assertIn(present, matching)
            rust(self.output, [method for method in methods if method["face"] == "client"])
            rust_host(self.output, [method for method in methods if method["face"] == "context"])
            client = (self.output / "sdk/rust/src/connection/resources.rs").read_text()
            context = (self.output / "sdk/rust/src/plugin/host.rs").read_text()
            self.assertEqual("use harmos_protocol::host::TimeGetRequest;" in client, callers == "CLIENTS")
            self.assertEqual("use harmos_protocol::host::TimeGetRequest;" in context, callers == "PLUGINS")
            self.assertIn("use harmos_protocol::host::TimeSleepRequest;", client)
            self.assertIn("use harmos_protocol::host::TimeSleepRequest;", context)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
