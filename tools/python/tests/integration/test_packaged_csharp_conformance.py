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
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

from harmos_tools.build.paths import output
from harmos_tools.csharp import run as csharp
from harmos_tools.process import spawn_flags

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class PackagedCSharpConformance(unittest.TestCase):
    def test_nuget_package_serves_the_public_fixture(self) -> None:
        root = Path(__file__).resolve().parents[4]
        dotnet_root = os.environ.get("DOTNET_ROOT")
        dotnet = str(Path(dotnet_root) / ("dotnet.exe" if os.name == "nt" else "dotnet")) if dotnet_root else "dotnet"
        environment = dict(os.environ, DOTNET_NOLOGO="1", DOTNET_CLI_TELEMETRY_OPTOUT="1")
        environment.pop("UV_PROJECT_ENVIRONMENT", None)
        with tempfile.TemporaryDirectory() as name:
            scratch = Path(name)
            feed = scratch / "feed"
            # Pack Only the SDK Assembly and Product Messages.
            self.assertEqual(csharp(root, "build"), 0)
            self.assertEqual(csharp(root, "package"), 0)
            feed.mkdir()
            for package in (root / "sdk/csharp/artifacts/packages").glob("*.nupkg"):
                shutil.copyfile(package, feed / package.name)
            package = next(feed.glob("Harmos.*.nupkg"))
            with zipfile.ZipFile(package) as archive:
                manifest = ET.fromstring(archive.read("Harmos.nuspec"))
                metadata = manifest.find("{*}metadata")
                assert metadata is not None
                version = metadata.findtext("{*}version")
            dependencies = ET.parse(root / "sdk/csharp/Directory.Packages.props")
            grpc_tools = next(
                item.attrib["Version"] for item in dependencies.iter("PackageVersion") if item.attrib["Include"] == "Grpc.Tools"
            )
            # Restore Only the Published Package into an Independent Application.
            consumer = scratch / "consumer"
            consumer.mkdir()
            project = consumer / "Consumer.csproj"
            project.write_text(
                f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><TreatWarningsAsErrors>true</TreatWarningsAsErrors></PropertyGroup><ItemGroup><PackageReference Include="Harmos" Version="{version}" /><PackageReference Include="Grpc.Tools" Version="{grpc_tools}" PrivateAssets="All" /><Protobuf Include="proto/harmos/conformance/v1/fixture.proto" ProtoRoot="proto" GrpcServices="None" /></ItemGroup></Project>',
                encoding="utf-8",
            )
            shutil.copyfile(root / "sdk/csharp/tests/Harmos.Conformance/Fixture.cs", consumer / "Fixture.cs")
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
            result = subprocess.run(
                [dotnet, "msbuild", str(project), "-getProperty:TargetPath"],
                cwd=consumer,
                env=environment,
                check=True,
                capture_output=True,
                text=True,
                creationflags=spawn_flags(),
            )
            plugin = Path(result.stdout.strip()).with_suffix(suffix)
            exported = output(root, "build", os.environ.get("TEST_PROFILE", "dev")) / f"bin/harmos-conformance{suffix}"
            local = root / f"target/debug/harmos-conformance{suffix}"
            host = Path(os.environ.get("HARMOS_CONFORMANCE_HOST", local if local.is_file() else exported))
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
            self.assertEqual(len([line for line in result.stdout.splitlines() if line.endswith("| PASS |")]), 31, result.stdout)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
