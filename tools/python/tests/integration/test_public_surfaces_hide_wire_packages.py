# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Public Surfaces Hide Wire Packages"""

# === Imports === #

# Standard Library Imports
import tempfile
import unittest
from pathlib import Path

# Third Party Imports
# Local Imports
from harmos_tools.source.protocol import check_public_surfaces

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


class PublicSurfacesHideWirePackages(unittest.TestCase):
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
            self.assertRaises(ValueError, check_public_surfaces, root)
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
                self.assertRaises(
                    ValueError,
                    check_public_surfaces,
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
