# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Compiler Cache Keys Follow Locked Inputs."""

# === Imports === #

# Standard Library Imports
import contextlib
import io
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.ci import bootstrap

# Type Checking Imports


# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #


class CompilerCacheKeyTests(unittest.TestCase):
    def test_tool_changes_preserve_compiler_cache_and_lock_changes_restore(self) -> None:
        temporary_directory = tempfile.TemporaryDirectory()
        temporary = temporary_directory.name
        try:
            root = Path(temporary)
            output = root / "outputs"
            environment = {
                "GITHUB_OUTPUT": str(output),
                "RUNNER_OS": "Windows",
                "RUNNER_ARCH": "X64",
                "MISE_DATA_DIR": str(root / "mise"),
                "COMPILER_CACHE_BACKEND": "disk",
                "PROFILE": "release",
            }
            keys = []
            for rust, tools, lock in [
                ("1.2.3", "first", "first"),
                ("1.2.3", "second", "first"),
                ("1.2.3", "second", "second"),
                ("1.2.4", "second", "second"),
            ]:
                (root / "mise.toml").write_text('[tools]\nrust = { version = "' + rust + '" }\n')
                (root / "mise.lock").write_text(tools)
                (root / "uv.lock").write_text(tools)
                (root / "Cargo.lock").write_text(lock)
                output.write_text("")
                with (
                    patch.dict(os.environ, environment, clear=True),
                    patch("sys.argv", ["bootstrap", "--roots-only"]),
                    patch.object(bootstrap.Path, "cwd", return_value=root),
                    patch.object(bootstrap.Path, "home", return_value=root),
                    patch.object(bootstrap, "rustup"),
                    contextlib.redirect_stdout(io.StringIO()),
                ):
                    bootstrap.main()
                fields = output.read_text().split("CACHE_VALUE\n")
                key = next(fields[i + 1].strip() for i in range(len(fields) - 1) if fields[i].endswith("sccache-key<<"))
                prefix = next(fields[i + 1].strip() for i in range(len(fields) - 1) if fields[i].endswith("sccache-restore-keys<<"))
                keys.append((key, prefix))
            self.assertEqual(keys[0], keys[1])
            self.assertNotEqual(keys[1][0], keys[2][0])
            self.assertEqual(keys[1][1], keys[2][1])
            self.assertNotEqual(keys[2][1], keys[3][1])
        finally:
            temporary_directory.cleanup()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
