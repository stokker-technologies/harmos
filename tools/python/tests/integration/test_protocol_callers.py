# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Protocol Caller Contract Checks"""

# === Imports === #

# Standard Library Imports
import tempfile
import unittest
from pathlib import Path

# Third Party Imports
from google.protobuf.descriptor_pb2 import FileDescriptorSet

# Local Imports
import harmos_tools.build.resources.descriptor
from harmos_tools.source.protocol import caller_options, check_public_surfaces

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


class ProtocolCallers(unittest.TestCase):
    def setUp(self) -> None:
        root = Path(__file__).resolve().parents[4]
        self.descriptors = FileDescriptorSet.FromString((root / "crates/harmos-protocol/src/descriptors.bin").read_bytes())

    def test_missing_callers_refuses_host_operations(self) -> None:
        descriptors = FileDescriptorSet.FromString(self.descriptors.SerializeToString())
        declared = caller_options(descriptors)
        plugin = next(file for file in descriptors.file if "/plugin/" in file.name and file.service)
        for service in plugin.service:
            for method in service.method:
                method.ClearField("options")
        self.assertEqual(caller_options(descriptors), declared)
        host = next(file for file in descriptors.file if "/host/" in file.name and file.service)
        host.service[0].method[0].ClearField("options")
        with self.assertRaises(ValueError):
            caller_options(descriptors)

    def test_shared_settings_has_nested_paths_and_optional_selector(self) -> None:
        methods = [
            method
            for method in harmos_tools.build.resources.descriptor.table(self.descriptors)
            if method["path"] == ["plugins", "settings", "list"]
        ]
        self.assertEqual({method["face"] for method in methods}, {"client", "context"})
        self.assertEqual(len({method["request"] for method in methods}), 1)
        self.assertEqual(len({method["response"] for method in methods}), 1)
        self.assertTrue(all(next(field for field in method["fields"] if field["name"] == "name")["optional"] for method in methods))
        self.assertFalse(
            any(
                method["face"] == "context" and method["path"] == ["plugins", "settings", "set"]
                for method in harmos_tools.build.resources.descriptor.table(self.descriptors)
            )
        )

    def test_public_surface_check_rejects_wire_imports_and_allows_internal_generation(self) -> None:
        scratch_directory = tempfile.TemporaryDirectory()
        scratch = scratch_directory.name
        try:
            root = Path(scratch)
            (root / "README.md").write_text("# SDK\n", encoding="utf-8")
            headers = root / "sdk/cpp/include/harmos"
            (headers / "detail").mkdir(parents=True)
            (headers / "detail/types.hpp").write_text("using Value = ::harmos::model::v1::Value;\n", encoding="utf-8")
            public = headers / "types.hpp"
            public.write_text('#include "detail/types.hpp"\n', encoding="utf-8")
            check_public_surfaces(root)
            public.write_text("using Value = ::harmos::model::v1::Value;\n", encoding="utf-8")
            with self.assertRaises(ValueError):
                check_public_surfaces(root)
        finally:
            scratch_directory.cleanup()

    def test_product_public_surfaces_use_configured_roots_and_namespace(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        for relative, content in (
            ("python/example/__init__.py", "from example._wire.values_pb2 import Value"),
            ("include/values.hpp", "using ::example::model::v1::Value;"),
            ("managed/Value.cs", "namespace ExampleSDK.V1;"),
            ("native/lib.rs", "pub mod v1;"),
        ):
            with self.subTest(path=relative):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")
                with self.assertRaises(ValueError):
                    check_public_surfaces(
                        root,
                        namespace="example",
                        csharp_namespace="ExampleSDK",
                        python_package=Path("python/example"),
                        cpp_include=Path("include"),
                        csharp_source=Path("managed"),
                        rust_source=Path("native"),
                    )
                path.unlink()
                check_public_surfaces(
                    root,
                    namespace="example",
                    csharp_namespace="ExampleSDK",
                    python_package=Path("python/example"),
                    cpp_include=Path("include"),
                    csharp_source=Path("managed"),
                    rust_source=Path("native"),
                )


# === Private Functions === #

# === Entry Point === #
