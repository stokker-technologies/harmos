# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""A Published NuGet SDK Serves Its Importable Conformance Fixture"""

# === Imports === #

# Standard Library Imports
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from harmos_tools.process import spawn_flags
from harmos_tools.build.paths import output

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class PackagedCSharpConformance(unittest.TestCase):
    def test_nuget_package_serves_the_public_fixture(self):
        root = Path(__file__).resolve().parents[4]
        dotnet_root = os.environ.get("DOTNET_ROOT")
        dotnet = str(Path(dotnet_root) / ("dotnet.exe" if os.name == "nt" else "dotnet")) if dotnet_root else "dotnet"
        environment = dict(os.environ, DOTNET_NOLOGO="1", DOTNET_CLI_TELEMETRY_OPTOUT="1")
        environment.pop("UV_PROJECT_ENVIRONMENT", None)
        with tempfile.TemporaryDirectory() as name:
            scratch = Path(name)
            feed = scratch / "feed"
            # Pack Only the SDK Assembly and Product Messages.
            result = subprocess.run(
                [dotnet, "pack", str(root / "sdk/csharp/Harmos/Harmos.csproj"), "--configuration", "Debug", "--output", str(feed)],
                cwd=root,
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                creationflags=spawn_flags(),
            )
            self.assertEqual(result.returncode, 0, result.stdout)
            # Restore Only the Published Package into an Independent Application.
            consumer = scratch / "consumer"
            consumer.mkdir()
            project = consumer / "Consumer.csproj"
            project.write_text(
                '<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><PackageReference Include="Harmos" Version="0.1.0" /><PackageReference Include="Grpc.Tools" Version="2.76.0" PrivateAssets="All" /><Protobuf Include="proto/harmos/conformance/v1/fixture.proto" ProtoRoot="proto" GrpcServices="None" /></ItemGroup></Project>',
                encoding="utf-8",
            )
            shutil.copyfile(root / "sdk/csharp/Conformance/Fixture.cs", consumer / "Fixture.cs")
            shutil.copytree(root / "sdk/tests/fixtures/conformance/proto", consumer / "proto")
            (consumer / "Program.cs").write_text("await new Harmos.Conformance.Fixture().Build().ServeAsync();\n", encoding="utf-8")
            config = consumer / "NuGet.Config"
            config.write_text(
                f'<configuration><packageSources><clear /><add key="fixture" value="{feed.as_posix()}" /><add key="nuget" value="https://api.nuget.org/v3/index.json" /></packageSources></configuration>',
                encoding="utf-8",
            )
            packages = scratch / "packages"
            result = subprocess.run(
                [dotnet, "restore", str(project), "--packages", str(packages), "--configfile", str(config)],
                cwd=consumer,
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                creationflags=spawn_flags(),
            )
            self.assertEqual(result.returncode, 0, result.stdout)
            result = subprocess.run(
                [dotnet, "build", str(project), "--no-restore"],
                cwd=consumer,
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                creationflags=spawn_flags(),
            )
            self.assertEqual(result.returncode, 0, result.stdout)
            # Run All Reference-Host Cases Against the Package-Only Plugin.
            suffix = ".exe" if os.name == "nt" else ""
            plugin = consumer / f"bin/Debug/net10.0/Consumer{suffix}"
            exported = output(root, "build", os.environ.get("TEST_PROFILE", "dev")) / f"bin/harmos-conformance{suffix}"
            local = root / f"target/debug/harmos-conformance{suffix}"
            host = Path(os.environ.get("HARMOS_CONFORMANCE_HOST", exported if exported.is_file() else local))
            result = subprocess.run(
                [str(host), "--plugin", str(plugin)],
                cwd=consumer,
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                timeout=120,
                creationflags=spawn_flags(),
            )
            self.assertEqual(result.returncode, 0, result.stdout)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
