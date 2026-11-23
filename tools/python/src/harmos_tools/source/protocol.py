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
import sys
import tempfile
from pathlib import Path

# Third Party Imports
# Local Imports
from ..build import codegen
from ..process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    check_public_surfaces(Path(__file__).resolve().parents[5])
    scratch_directory = tempfile.TemporaryDirectory()
    scratch = scratch_directory.name
    try:
        output = Path(scratch) / "descriptors.bin"
        # Run the Native Command
        command = ["buf", "build", "--as-file-descriptor-set", "-o", str(output)]
        subprocess.run(command, check=True, stdout=sys.stdout, stderr=sys.stderr, creationflags=spawn_flags())
        # Check the Caller Contracts With the Harmos CLI
        command = [str(codegen.cli()), "proto", "check", "--descriptors", str(output)]
        subprocess.run(command, check=True, stdout=sys.stdout, stderr=sys.stderr, creationflags=spawn_flags())
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

    Allow internal generated directories (detail, _generated) and canonical
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
            # Barrels Bind Public Names From the Private Generated Package.
            if re.match(r"from \.+_generated\.", line):
                continue
            line = grpc_path.sub("", line)
            if wire.search(line) or re.search(r"\bv\d+\b", line):
                findings.append(f"{relative.as_posix()}:{number}: Public Surface Names a Wire Package")
    exports: dict[str, Path] = {}
    for name in ("__init__.py", "client.py", "model.py", "plugin/__init__.py"):
        path = python_package / name
        if not path.is_file():
            continue
        for node in ast.parse(path.read_text(encoding="utf-8")).body:
            targets = [target.id for target in node.targets if isinstance(target, ast.Name)] if isinstance(node, ast.Assign) else []
            for target in targets:
                if target == "__all__":
                    findings.append(f"{path.relative_to(root)}: Bind Public Names by Assignment Instead of __all__")
                elif target in exports:
                    findings.append(f"{path.relative_to(root)}: Duplicate Public Export {target}")
                exports[target] = path
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


# === Entry Point === #

if __name__ == "__main__":
    main()
