# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Published Rust Crates Serve a Test-Owned Conformance Fixture"""

# === Imports === #

# Standard Library Imports
import os
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import unittest
from pathlib import Path

from harmos_tools.build.paths import output
from harmos_tools.process import spawn_flags

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class PackagedRustConformance(unittest.TestCase):
    def test_published_crates_serve_the_public_fixture(self) -> None:
        root = Path(__file__).resolve().parents[4]
        version = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
        environment = dict(os.environ, CARGO_TARGET_DIR=str(root / "target"), CARGO_BUILD_WARNINGS="deny")
        environment.pop("UV_PROJECT_ENVIRONMENT", None)
        environment["RUSTUP_TOOLCHAIN"] = tomllib.loads((root / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
        with tempfile.TemporaryDirectory() as name:
            scratch = Path(name)
            packages = {
                "harmos-protocol": root / "crates/harmos-protocol",
                "harmos-sdk-derive": root / "sdk/rust/derive",
                "harmos": root / "sdk/rust",
            }
            patches = []
            for package, path in packages.items():
                patches += ["--config", f'patch.crates-io.{package}.path="{path.as_posix()}"']
            # Build the Three Published Crate Archives Without Workspace Path Dependencies.
            for package in packages:
                result = subprocess.run(
                    ["cargo", "package", "--locked", "--offline", "--allow-dirty", "--no-verify", "--package", package, *patches],
                    cwd=root,
                    env=environment,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True,
                    creationflags=spawn_flags(),
                )
                self.assertEqual(result.returncode, 0, result.stdout)
                archive = root / f"target/package/{package}-{version}.crate"
                with tarfile.open(archive) as bundle:
                    bundle.extractall(scratch, filter="data")
            # Build an Independent Consumer Whose Only Product Dependency Is the Packaged SDK.
            consumer = scratch / "consumer"
            (consumer / "src").mkdir(parents=True)
            shutil.copytree(root / "sdk/rust/tests/fixtures/conformance", consumer / "src/conformance")
            manifest = f'[package]\nname = "packaged-conformance"\nversion = "0.0.0"\nedition = "2024"\n[dependencies]\nharmos = "={version}"\nharmos-protocol = "={version}"\ntokio = {{ version = "1", features = ["macros", "rt", "sync"] }}\nprost = "0.14"\nprost-types = "0.14"\nfutures-util = "0.3"\n[patch.crates-io]\n'
            for package in packages:
                manifest += f'{package} = {{ path = "../{package}-{version}" }}\n'
            manifest += '[profile.dev]\ndebug = "line-tables-only"\n[profile.dev.package."*"]\ndebug = false\n'
            (consumer / "Cargo.toml").write_text(manifest, encoding="utf-8")
            (consumer / "src/main.rs").write_text(
                "mod conformance;\nfn main() -> harmos::Result<()> { conformance::plugin().serve() }\n", encoding="utf-8"
            )
            result = subprocess.run(
                ["cargo", "build", "--offline", "--manifest-path", str(consumer / "Cargo.toml")],
                cwd=consumer,
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                creationflags=spawn_flags(),
            )
            self.assertEqual(result.returncode, 0, result.stdout)
            # Run the Reference Host Against This Package-Only Consumer.
            suffix = ".exe" if os.name == "nt" else ""
            exported = output(root, "build", os.environ.get("TEST_PROFILE", "dev")) / f"bin/harmos-conformance{suffix}"
            local = root / f"target/debug/harmos-conformance{suffix}"
            host = Path(os.environ.get("HARMOS_CONFORMANCE_HOST", exported if exported.is_file() else local))
            result = subprocess.run(
                [str(host), "--plugin", str(root / f"target/debug/packaged-conformance{suffix}")],
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
