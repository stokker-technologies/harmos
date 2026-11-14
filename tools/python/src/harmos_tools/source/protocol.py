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
from google.protobuf import descriptor_pool, message_factory
from google.protobuf.descriptor_pb2 import FileDescriptorSet

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
    pool = descriptor_pool.DescriptorPool()
    pending = {file.name: file for file in descriptors.file}
    while pending:
        ready = [file for file in pending.values() if all(name not in pending for name in file.dependency)]
        if not ready:
            raise ValueError("Protocol Callers: Unresolved Descriptor Dependencies")
        for file in ready:
            pool.AddSerializedFile(file.SerializeToString())
            del pending[file.name]
    result = {}
    for file in descriptors.file:
        if "host" not in Path(file.name).parts[:-1]:
            continue
        for service in file.service:
            for method in service.method:
                identity = f"{file.package}.{service.name}/{method.name}"
                try:
                    extension = pool.FindExtensionByName("harmos.model.v1.callers")
                except KeyError as error:
                    raise ValueError(f"Protocol Callers: {identity} Has No Callers Declaration") from error
                if extension.enum_type is None:
                    raise ValueError("Protocol Callers: Option Must Be an Enum")
                options = cast(Any, message_factory.GetMessageClass(pool.FindMessageTypeByName("google.protobuf.MethodOptions"))())
                options.ParseFromString(method.options.SerializeToString())
                value = options.Extensions[extension]
                allowed = extension.enum_type.values_by_number.get(value)
                if not options.HasExtension(extension) or allowed is None or allowed.name not in {"CLIENTS", "PLUGINS", "ALL"}:
                    raise ValueError(f"Protocol Callers: {identity} Requires CLIENTS, PLUGINS or ALL")
                result[identity] = allowed.name
    return result


def narrow_callers(descriptors: FileDescriptorSet, overrides: dict[str, str]) -> None:
    """Apply Product Assembly Restrictions to the Generator's Descriptor Copy."""
    declared = caller_options(descriptors)
    audiences = {"CLIENTS": {"CLIENTS"}, "PLUGINS": {"PLUGINS"}, "ALL": {"CLIENTS", "PLUGINS"}}
    for identity, selected in overrides.items():
        original = declared.get(identity)
        if original is None or selected not in audiences or not audiences[selected] <= audiences[original]:
            raise ValueError(f"Narrow {identity}: Callers Must Be a Subset of the Declaration")
    # The Extension Number and Its Definition Are Owned by the Descriptor, Not This Tool.
    pool = descriptor_pool.DescriptorPool()
    pending = {file.name: file for file in descriptors.file}
    while pending:
        ready = [file for file in pending.values() if all(name not in pending for name in file.dependency)]
        for file in ready:
            pool.AddSerializedFile(file.SerializeToString())
            del pending[file.name]
    extension = pool.FindExtensionByName("harmos.model.v1.callers")
    if extension.enum_type is None:
        raise ValueError("Protocol Callers: Option Must Be an Enum")
    options_type = message_factory.GetMessageClass(pool.FindMessageTypeByName("google.protobuf.MethodOptions"))
    for file in descriptors.file:
        for service in file.service:
            for method in service.method:
                selected = overrides.get(f"{file.package}.{service.name}/{method.name}")
                if selected is None:
                    continue
                options = cast(Any, options_type.FromString(method.options.SerializeToString()))
                options.Extensions[extension] = extension.enum_type.values_by_name[selected].number
                method.options.ParseFromString(options.SerializeToString())


def main() -> None:
    check_public_surfaces(Path(__file__).resolve().parents[5])
    with tempfile.TemporaryDirectory() as scratch:
        output = Path(scratch) / "descriptors.bin"
        subprocess.run(["buf", "build", "--as-file-descriptor-set", "-o", str(output)], check=True, creationflags=spawn_flags())
        methods = caller_options(FileDescriptorSet.FromString(output.read_bytes()))
        print(f"Protocol Callers: {len(methods)} Annotated Operations")


def check_public_surfaces(root: Path) -> None:
    """Keep Protocol Implementation Names Out of Consumer Entry Points.

    Allow internal generated directories (detail, _generated, _wire), canonical
    gRPC method paths, and protocol documentation that maps packages
    (proto/**/*.md and docs/architecture.md). Markdown link destinations refer
    to source files and are not SDK imports.
    """
    wire = re.compile(r"\b_wire\b|\w+_wire\b|\bHarmos\.Internal\b|\b(?:[A-Za-z_]\w*(?:\.|::))+v\d+\b|[/\\]v\d+[/\\]")
    grpc_path = re.compile(r"/?[a-zA-Z_]\w*(?:\.\w+)*\.v\d+(?:\.\w+)*/[A-Za-z_]\w*")
    candidates = [root / "README.md", *sorted((root / "docs").glob("*.md"))]
    candidates += sorted((root / "sdk/cpp/include").rglob("*.hpp"))
    candidates += sorted((root / "sdk/cpp/examples").glob("*/generated/*.hpp"))
    candidates += [
        path
        for path in (root / "sdk/python/src/harmos").rglob("*")
        if path.suffix in {".py", ".pyi"}
        and not any(part.startswith("_") for part in path.relative_to(root / "sdk/python/src/harmos").parts[:-1])
        and (not path.name.startswith("_") or path.name == "__init__.py")
    ]
    findings = []
    for path in candidates:
        relative = path.relative_to(root)
        if relative.as_posix() == "docs/architecture.md" or "detail" in relative.parts:
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            line = grpc_path.sub("", line)
            if path.suffix == ".md":
                line = re.sub(r"\]\([^)]*\)", "]()", line)
            if wire.search(line) or (path.suffix != ".md" and re.search(r"\bv\d+\b", line)):
                findings.append(f"{relative.as_posix()}:{number}: Public Surface Names a Wire Package")
    exports: dict[str, Path] = {}
    for name in ("__init__.py", "client.py", "plugin/__init__.py"):
        path = root / "sdk/python/src/harmos" / name
        if not path.is_file():
            continue
        for node in ast.walk(ast.parse(path.read_text(encoding="utf-8"))):
            if not isinstance(node, ast.Assign) or not any(
                isinstance(target, ast.Name) and target.id == "__all__" for target in node.targets
            ):
                continue
            if not isinstance(node.value, (ast.List, ast.Tuple)):
                raise ValueError(f"Check Public Exports: {path} Must Declare Its Names Explicitly")
            for item in node.value.elts:
                if not isinstance(item, ast.Constant) or not isinstance(item.value, str):
                    raise ValueError(f"Check Public Exports: {path} Must Declare String Names")
                if item.value in exports:
                    findings.append(f"{path.relative_to(root)}: Duplicate Public Export {item.value}")
                exports[item.value] = path
    for path in sorted((root / "sdk/csharp/src/Harmos").rglob("*.cs")):
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if re.search(r"^namespace .*\.V\d+\b", line):
                findings.append(f"{path.relative_to(root)}:{number}: Public Wire Namespace")
    for path in sorted((root / "sdk/rust/src").rglob("*.rs")):
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if re.search(r"\bpub\s+mod\s+(?:wire|v\d+)\b", line):
                findings.append(f"{path.relative_to(root).as_posix()}:{number}: Public Wire Module")
    if findings:
        raise ValueError("Check Public Surfaces:\n" + "\n".join(findings))
    print("Public Surfaces: Headers, Stubs, Exports and Guides Passed")


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
