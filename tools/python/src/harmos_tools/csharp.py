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
    root: Path, command: str, profile: str = "dev", *, check: bool = False, dependencies: bool = False, audience: str | None = None
) -> int:
    from .build.resource_roots import generate

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
        selections = (
            [("Harmos/Harmos.csproj", audience.title())]
            if audience
            else [(project, "Plugin") for project in projects] + [("Harmos/Harmos.csproj", "Client"), ("Harmos/Harmos.csproj", "Host")]
        )
        for project, selected_audience in selections:
            result = subprocess.run(
                [dotnet, "restore", project, "--locked-mode", f"-p:HarmosAudience={selected_audience}"],
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
                [dotnet, "build", project, "--no-restore", "--configuration", configuration, f"-p:HarmosAudience={selected_audience}"],
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


# === Private Functions === #

# === Entry Point === #
