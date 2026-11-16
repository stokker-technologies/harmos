# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Bootstrap Downloads Verify Pinned Archives."""

# === Imports === #

# Standard Library Imports
import contextlib
import email.message
import hashlib
import io
import runpy
import tempfile
import unittest
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

# Third Party Imports
# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[4]


# === Types === #

# === Classes === #


class SetupDownloadTests(unittest.TestCase):
    URL = "https://example.com/archive.zip"
    CHECKSUM = hashlib.sha256(b"archive").hexdigest()

    def setUp(self) -> None:
        self.download = runpy.run_path(str(ROOT / "tools/python/src/harmos_tools/ci/bootstrap.py"))["download"]
        self.calls: list[str] = []
        self.pauses: list[float] = []

    def fetch(self, codes: list[int], checksum: str = CHECKSUM) -> bytes:
        def opener(url: str, timeout: float) -> io.BytesIO:
            self.calls.append(url)
            code = codes.pop(0)
            if code != 200:
                raise urllib.error.HTTPError(url, code, "failure", email.message.Message(), io.BytesIO(b""))
            return io.BytesIO(b"archive")

        directory_directory = tempfile.TemporaryDirectory()
        directory = directory_directory.name
        try:
            with contextlib.redirect_stdout(io.StringIO()):
                destination = Path(directory) / "archive.zip"
                try:
                    content = self.download(self.URL, checksum, destination, opener=opener, sleep=self.pauses.append)
                    self.assertEqual(destination.read_bytes(), content)
                    return content
                finally:
                    self.assertEqual([path.name for path in Path(directory).iterdir() if path != destination], [])
        finally:
            directory_directory.cleanup()

    def test_transient_status_retries_then_returns_bytes(self) -> None:
        self.assertEqual(self.fetch([503, 200]), b"archive")
        self.assertEqual(len(self.calls), 2)
        self.assertEqual(len(self.pauses), 1)

    def test_missing_archive_fails_without_retry(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "HTTP 404"):
            self.fetch([404])
        self.assertEqual(self.calls, [self.URL])

    def test_checksum_mismatch_fails_without_retry(self) -> None:
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            self.fetch([200, 200], checksum="0" * 64)
        self.assertEqual(self.calls, [self.URL])

    def test_exhausted_attempts_name_url_and_count(self) -> None:
        with self.assertRaises(RuntimeError) as raised:
            self.fetch([504] * 6)
        self.assertEqual(len(self.calls), 6)
        self.assertEqual(len(self.pauses), 5)
        for fragment in (self.URL, "after 6 attempts", "HTTP 504"):
            self.assertIn(fragment, str(raised.exception))


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
