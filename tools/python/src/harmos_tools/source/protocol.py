# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Check Protocol Caller Contracts"""

# === Imports === #

# Standard Library Imports
import ast
import re
import subprocess
import tempfile
from pathlib import Path
from typing import Any, cast

# Third Party Imports
from google.protobuf.descriptor_pb2 import FileDescriptorSet
from google.protobuf.descriptor_pool import DescriptorPool
from google.protobuf.message_factory import GetMessageClass

# Local Imports
from ..process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def caller_options(descriptors: FileDescriptorSet) -> dict[str, str]:
    """Read Custom Options Without Depending on Generated SDK Imports."""
    pool = _descriptor_inventory(descriptors)
    return _read_callers(descriptors, pool)


def main() -> None:
    check_public_surfaces(Path(__file__).resolve().parents[5])
    scratch_directory = tempfile.TemporaryDirectory()
    scratch = scratch_directory.name
    try:
        output = Path(scratch) / "descriptors.bin"
        # Run the Native Command
        command = ["buf", "build", "--as-file-descriptor-set", "-o", str(output)]
        subprocess.run(command, check=True, creationflags=spawn_flags())
        methods = caller_options(FileDescriptorSet.FromString(output.read_bytes()))
        print(f"Protocol Callers: {len(methods)} Annotated Operations")
    finally:
        scratch_directory.cleanup()


def check_public_surfaces(
    root: Path,
    *,
    namespace: str = "harmos",
    csharp_namespace: str | None = None,
    python_package: Path | None = None,
    cpp_include: Path = Path("sdk/cpp/include"),
    cpp_examples: Path = Path("sdk/cpp/examples"),
    csharp_source: Path | None = None,
    rust_source: Path = Path("sdk/rust/src"),
) -> None:
    """Keep Protocol Implementation Names Out of Consumer Entry Points.

    Allow internal generated directories (detail, _generated, _wire) and canonical
    gRPC method paths. CI documentation checking owns the guide surface.
    """
    python_package = root / (python_package or Path(f"sdk/python/src/{namespace}"))
    csharp_namespace = csharp_namespace or namespace.title()
    csharp_source = root / (csharp_source or Path(f"sdk/csharp/src/{namespace.title()}"))
    wire = re.compile(
        r"\b_wire\b|\w+_wire\b|\b" + re.escape(csharp_namespace) + r"\.Internal\b|\b(?:[A-Za-z_]\w*(?:\.|::))+v\d+\b|[/\\]v\d+[/\\]"
    )
    grpc_path = re.compile(r"/?[a-zA-Z_]\w*(?:\.\w+)*\.v\d+(?:\.\w+)*/[A-Za-z_]\w*")
    candidates = sorted((root / cpp_include).rglob("*.hpp"))
    candidates += sorted((root / cpp_examples).glob("*/generated/*.hpp"))
    candidates += [
        path
        for path in python_package.rglob("*")
        if path.suffix in {".py", ".pyi"}
        and not any(part.startswith("_") for part in path.relative_to(python_package).parts[:-1])
        and (not path.name.startswith("_") or path.name == "__init__.py")
    ]
    findings = []
    for path in candidates:
        relative = path.relative_to(root)
        if "detail" in relative.parts:
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            line = grpc_path.sub("", line)
            if wire.search(line) or re.search(r"\bv\d+\b", line):
                findings.append(f"{relative.as_posix()}:{number}: Public Surface Names a Wire Package")
    exports: dict[str, Path] = {}
    for name in ("__init__.py", "client.py", "plugin/__init__.py"):
        path = python_package / name
        if not path.is_file():
            continue
        for node in ast.walk(ast.parse(path.read_text(encoding="utf-8"))):
            if not isinstance(node, ast.Assign) or not any(
                isinstance(target, ast.Name) and target.id == "__all__" for target in node.targets
            ):
                continue
            if not isinstance(node.value, (ast.List, ast.Tuple)):
                raise ValueError(
                    f"Check Public Exports: {path} Must Declare Its Names Explicitly; Correct the source protocol declarations or explicit public exports before regenerating"
                )
            for item in node.value.elts:
                if not isinstance(item, ast.Constant) or not isinstance(item.value, str):
                    raise ValueError(
                        f"Check Public Exports: {path} Must Declare String Names; Correct the source protocol declarations or explicit public exports before regenerating"
                    )
                if item.value in exports:
                    findings.append(f"{path.relative_to(root)}: Duplicate Public Export {item.value}")
                exports[item.value] = path
    for path in sorted(csharp_source.rglob("*.cs")):
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if re.search(r"^namespace .*\.V\d+\b", line):
                findings.append(f"{path.relative_to(root)}:{number}: Public Wire Namespace")
    for path in sorted((root / rust_source).rglob("*.rs")):
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if re.search(r"\bpub\s+mod\s+(?:wire|v\d+)\b", line):
                findings.append(f"{path.relative_to(root).as_posix()}:{number}: Public Wire Module")
    if findings:
        raise ValueError(
            "Check Public Surfaces: Correct the source protocol declarations or explicit public exports before regenerating\n"
            + "\n".join(findings)
        )
    print("Public Surfaces: Headers, Stubs and Exports Passed")


# === Private Functions === #


def _read_callers(descriptors: FileDescriptorSet, pool: DescriptorPool) -> dict[str, str]:
    methods = [
        (f"{file.package}.{service.name}/{method.name}", method)
        for file in descriptors.file
        if "host" in Path(file.name).parts[:-1]
        or (
            file.package.startswith("harmos.plugin.v1")
            and file.name.endswith(("/time.proto", "/operations.proto", "/invocations.proto", "/settings.proto"))
        )
        for service in file.service
        for method in service.method
    ]
    if not methods:
        return {}

    # Resolve the Shared Option Schema Once for the Entire Descriptor Inventory.
    try:
        extension = pool.FindExtensionByName("harmos.model.v1.callers")
    except KeyError as error:
        identity = methods[0][0]
        raise ValueError(
            f"Protocol Callers: {identity} Has No Callers Declaration; Correct the source protocol declarations or explicit public exports before regenerating"
        ) from error
    if extension.enum_type is None:
        raise ValueError(
            "Protocol Callers: Option Must Be an Enum; Correct the source protocol declarations or explicit public exports before regenerating"
        )
    options_type = GetMessageClass(pool.FindMessageTypeByName("google.protobuf.MethodOptions"))

    result = {}
    for identity, method in methods:
        options = cast(Any, options_type.FromString(method.options.SerializeToString()))
        value = options.Extensions[extension]
        allowed = extension.enum_type.values_by_number.get(value)
        if allowed is None:
            raise ValueError(
                f"Protocol Callers: {identity} Requires CLIENTS, PLUGINS or ALL; Correct the source protocol declarations or explicit public exports before regenerating"
            )
        declared = options.HasExtension(extension)
        valid = allowed.name in {"CLIENTS", "PLUGINS", "ALL"}
        if not declared or not valid:
            raise ValueError(
                f"Protocol Callers: {identity} Requires CLIENTS, PLUGINS or ALL; Correct the source protocol declarations or explicit public exports before regenerating"
            )
        result[identity] = allowed.name
    return result


def _descriptor_inventory(descriptors: FileDescriptorSet) -> DescriptorPool:
    """Load Dependencies Before Their Users Without Losing Custom Options."""
    pool = DescriptorPool()
    pending = {file.name: file for file in descriptors.file}
    while pending:
        ready = [file for file in pending.values() if all(name not in pending for name in file.dependency)]
        if not ready:
            raise ValueError(
                "Protocol Callers: Supply Resolvable Descriptor Dependencies; Correct the source protocol declarations or explicit public exports before regenerating"
            )
        for file in ready:
            pool.AddSerializedFile(file.SerializeToString())
            del pending[file.name]
    return pool


# === Entry Point === #

if __name__ == "__main__":
    main()
