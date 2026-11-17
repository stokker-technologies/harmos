# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Prove Protocol Documentation Generation"""

# === Imports === #

# Standard Library Imports
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import TestCase, main
from unittest.mock import patch

# Third Party Imports
from google.protobuf.descriptor_pb2 import FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet

# Local Imports
from harmos_tools.docs import operations
from harmos_tools.release.policy import ToolError

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class OperationsReference(TestCase):
    def test_generation_excludes_retired_host_services_and_replaces_stale_pages(self) -> None:
        descriptors = FileDescriptorSet()
        for package in ("harmos.model.v1", "harmos.plugin.v1", "harmos.host.v1", "schematic.host.v1"):
            file = descriptors.file.add(name=package.replace(".", "/") + "/operations.proto", package=package)
            file.message_type.add(name="Request")
            file.message_type.add(name="Response")
            if package != "harmos.model.v1":
                service = file.service.add(name="Operations")
                service.method.add(name="Run", input_type=f".{package}.Request", output_type=f".{package}.Response")

        def build(directory: Path, arguments: list[str]) -> str:
            Path(arguments[-1]).write_bytes(descriptors.SerializeToString())
            return ""

        scratch = TemporaryDirectory()
        try:
            directory = Path(scratch.name)
            output = directory / operations.OUTPUT
            output.mkdir(parents=True)
            (output / "obsolete.md").write_text("obsolete", encoding="utf-8")
            with patch.object(operations.policy, "execute", side_effect=build):
                operations.generate(directory)
            self.assertEqual(
                {path.name for path in output.iterdir()},
                {"overview.md", "harmos.model.v1.md", "harmos.plugin.v1.md", "schematic.host.v1.md"},
            )
            generated = (output / "harmos.plugin.v1.md").read_text(encoding="utf-8")
            self.assertIn("`Operations.Run` | [Request](#request) | [Response](#response)", generated)
        finally:
            scratch.cleanup()

    def test_messages_preserve_map_optional_oneof_and_cross_package_links(self) -> None:
        file = FileDescriptorProto(name="sample.proto", package="sample.v1")
        message = file.message_type.add(name="Request")
        message.oneof_decl.add(name="choice")
        message.field.add(name="value", number=1, type=FieldDescriptorProto.TYPE_MESSAGE, type_name=".shared.v1.Value", oneof_index=0)
        message.field.add(name="count", number=2, type=FieldDescriptorProto.TYPE_UINT64, proto3_optional=True)
        entry = message.nested_type.add(name="LabelsEntry")
        entry.options.map_entry = True
        entry.field.add(name="key", number=1, type=FieldDescriptorProto.TYPE_STRING)
        entry.field.add(name="value", number=2, type=FieldDescriptorProto.TYPE_STRING)
        message.field.add(
            name="labels",
            number=3,
            type=FieldDescriptorProto.TYPE_MESSAGE,
            type_name=".sample.v1.Request.LabelsEntry",
            label=FieldDescriptorProto.LABEL_REPEATED,
        )
        owners = {".shared.v1.Value": ("shared.v1", "Value")}
        generated = operations.page(file.package, [file], owners, 1)
        self.assertIn("[shared.v1.Value](../shared.v1/#value)", generated)
        self.assertIn("| `count` | 2 | optional uint64 |", generated)
        self.assertIn("| `labels` | 3 | map<string, string> |", generated)

    def test_heading_collisions_refuse_ambiguous_type_links(self) -> None:
        file = FileDescriptorProto(name="collision.proto", package="sample.v1")
        file.message_type.add(name="AB")
        file.message_type.add(name="Ab")
        with self.assertRaises(ToolError):
            operations.page(file.package, [file], {}, 1)

    def test_product_reference_renders_configured_descriptor_packages_and_output(self) -> None:
        descriptors = FileDescriptorSet()
        for package in ("example.model.v1", "example.plugin.v1", "other.model.v1"):
            file = descriptors.file.add(name=package.replace(".", "/") + "/values.proto", package=package)
            file.message_type.add(name="Value")
        scratch = TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        directory = Path(scratch.name)
        image = directory / "schema.bin"
        image.write_bytes(descriptors.SerializeToString())
        for source in (descriptors, image):
            with self.subTest(source=type(source).__name__):
                output = operations.generate(
                    directory,
                    descriptors=source,
                    output=Path("reference"),
                    namespaces=("example",),
                    exclude=(),
                    namespace="example",
                    overview_paragraphs=(),
                )
                self.assertEqual({path.name for path in output.iterdir()}, {"overview.md", "example.model.v1.md", "example.plugin.v1.md"})
                self.assertIn("### Value", (output / "example.model.v1.md").read_text())


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
