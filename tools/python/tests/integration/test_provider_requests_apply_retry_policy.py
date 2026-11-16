# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Provider Requests Apply Retry Policy."""

# === Imports === #

# Standard Library Imports
import email.message
import io
import os
import unittest
import urllib.error
import urllib.parse
import urllib.request
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.release import policy, registry

# Type Checking Imports


# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #


class FakeOpener:
    """Answer Each Provider Request With The Next Scripted Status."""

    def __init__(self, codes: list[int]) -> None:
        self.codes = list(codes)
        self.methods: list[str] = []

    def open(self, request: urllib.request.Request, timeout: float) -> io.BytesIO:
        self.methods.append(request.get_method())
        code = self.codes.pop(0)
        if code != 200:
            raise urllib.error.HTTPError(request.full_url, code, "failure", email.message.Message(), io.BytesIO(b"{}"))
        return io.BytesIO(b"{}")


class ProviderRetryTests(unittest.TestCase):
    def test_transient_responses(self) -> None:
        cases = (
            ("DELETE", [502, 404], None, 2),
            ("GET", [503, 503, 200], b"{}", 3),
            ("POST", [429, 200], b"{}", 2),
            ("POST", [502], policy.ToolError, 1),
            ("PATCH", [502, 502, 502, 502], policy.ToolError, registry.RETRY_ATTEMPTS),
            ("PUT", [404], policy.ToolError, 1),
        )
        for method, codes, expected, calls in cases:
            with self.subTest(method=method, codes=codes):
                opener = FakeOpener(codes)
                pauses: list[float] = []
                with (
                    patch.dict(os.environ, {"GH_TOKEN": "token"}, clear=True),
                    patch.object(registry.urllib.request, "build_opener", return_value=opener),
                    patch.object(registry.time, "sleep", side_effect=pauses.append),
                ):
                    settings = {"provider": "github", "repository": "owner/repo"}
                    if expected is policy.ToolError:
                        with self.assertRaises(policy.ToolError):
                            registry.request(settings, "releases/1", method)
                    else:
                        self.assertEqual(registry.request(settings, "releases/1", method), expected)
                self.assertEqual(opener.methods, [method] * calls)
                self.assertEqual(pauses, [registry.RETRY_PAUSE_SECONDS * 2**index for index in range(calls - 1)])


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
