# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build and Prove the C# Plugin SDK Through the Shared Reference Host."""

# === Imports === #

# Standard Library Imports
import argparse
import os
import subprocess
from pathlib import Path

# Third Party Imports

# Local Imports
from .process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def run(
    root: Path, command: str, profile: str = "dev", *, check: bool = False, dependencies: bool = False, layer: str | None = None
) -> int:
    from .build.resource_roots import generate

    if command == "generate":
        generate(root)
    directory = root / "sdk/csharp"
    configuration = "Release" if profile == "release" else "Debug"
    environment = os.environ.copy()
    environment["DOTNET_CLI_TELEMETRY_OPTOUT"] = "1"
    environment["DOTNET_NOLOGO"] = "1"
    environment["DOTNET_GENERATE_ASPNET_CERTIFICATE"] = "false"
    # Mise's Windows dotnet Entry Is a Command Wrapper; Subprocess Uses the Native Host.
    dotnet_root = environment.get("DOTNET_ROOT")
    dotnet = str(Path(dotnet_root) / ("dotnet.exe" if os.name == "nt" else "dotnet")) if dotnet_root else "dotnet"
    projects = ("Harmos/Harmos.csproj", "Conformance/Conformance.csproj", "Tests/Tests.csproj")

    # Regenerate Each Configured Protocol with the SDK's Pinned Compiler.
    if command == "generate":
        for project in projects:
            result = subprocess.run(
                [dotnet, "restore", project, "--force-evaluate" if dependencies else "--locked-mode"],
                cwd=directory,
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                creationflags=spawn_flags(),
            )
            print(result.stdout, flush=True)
            result.check_returncode()
            result = subprocess.run(
                [dotnet, "build", project, "--no-restore", "--configuration", configuration, "-target:Protobuf_Clean,Protobuf_Compile"],
                cwd=directory,
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                creationflags=spawn_flags(),
            )
            print(result.stdout, flush=True)
            result.check_returncode()
        return 0

    # Build the SDK, Managed Fixture and Story as Explicit Producer Outputs.
    if command == "build":
        for project in projects:
            result = subprocess.run(
                [dotnet, "restore", project, "--locked-mode"],
                cwd=directory,
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                creationflags=spawn_flags(),
            )
            print(result.stdout, flush=True)
            result.check_returncode()
            result = subprocess.run(
                [dotnet, "build", project, "--no-restore", "--configuration", configuration],
                cwd=directory,
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                creationflags=spawn_flags(),
            )
            print(result.stdout, flush=True)
            result.check_returncode()
        return 0

    # Formatting Checks Consume Restored Projects and Never Run Other Language Checks.
    if command in ("format", "check"):
        for project in projects:
            arguments = [dotnet, "format", project, "--no-restore"]
            if command == "check" or check:
                arguments.append("--verify-no-changes")
            result = subprocess.run(
                arguments,
                cwd=directory,
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                creationflags=spawn_flags(),
            )
            print(result.stdout, flush=True)
            result.check_returncode()
        return 0

    # Run Only the Built C# Story Against the Separately Built Reference Host.
    suffix = ".exe" if os.name == "nt" else ""
    environment["HARMOS_CONFORMANCE_HOST"] = str(
        root / "target" / ("release" if profile == "release" else "debug") / f"harmos-conformance{suffix}"
    )
    environment["HARMOS_CSHARP_PLUGIN"] = str(
        directory / "Conformance/bin" / configuration / "net10.0" / f"harmos-csharp-conformance-plugin{suffix}"
    )
    result = subprocess.run(
        [
            dotnet,
            "test",
            "Tests/Tests.csproj",
            "--no-build",
            "--no-restore",
            "--configuration",
            configuration,
            *(
                ["--filter", "FullyQualifiedName~Harmos.Sdk.Tests.Unit|FullyQualifiedName~Harmos.Tests.ResourceRoots"]
                if layer == "unit"
                else []
            ),
            "--logger",
            "console;verbosity=detailed",
        ],
        cwd=directory,
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        creationflags=spawn_flags(),
    )
    print(result.stdout, flush=True)
    result.check_returncode()
    return 0


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build", "test", "format", "check", "generate"))
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--dependencies", action="store_true")
    parser.add_argument("--layer", choices=("unit", "integration"))
    options = parser.parse_args()
    raise SystemExit(
        run(
            Path(__file__).resolve().parents[4],
            options.command,
            options.profile,
            check=options.check,
            dependencies=options.dependencies,
            layer=options.layer,
        )
    )


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
