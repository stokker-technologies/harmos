# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Installable Wheels Keep Their Declared Audience."""

# === Imports === #

# Standard Library Imports

import subprocess
import sys
import tempfile
import unittest
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path


# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Selection:
    def set_exclude_all(self):
        pass


class ProtocolAudiences(unittest.TestCase):
    def test_generated_plugin_client_and_host_packages_keep_their_service_stubs(self):
        from unittest.mock import patch

        root = Path(__file__).resolve().parents[4] / "sdk/python"
        spec = spec_from_file_location("harmos_audience_build", root / "hatch_build.py")
        assert spec and spec.loader
        module = module_from_spec(spec)
        spec.loader.exec_module(module)
        for audience in ("plugin", "client", "host"):
            hook = module.CustomBuildHook(str(root), {}, Selection(), None, "", "wheel")
            data = {"force_include": {}}
            with patch.dict("os.environ", HARMOS_AUDIENCE=audience):
                hook.initialize("standard", data)
            package = next(iter(data["force_include"]))
            program = """
import importlib.util
import sys
sys.path.insert(0, sys.argv[1])
import harmos
audience = sys.argv[2]
assert hasattr(harmos, 'Client') == (audience != 'plugin')
assert hasattr(harmos, 'Plugin') == (audience != 'client')
if audience != 'client':
    from harmos.plugin.v1 import instance_pb2_grpc as instance
    assert hasattr(instance, 'InstanceServicer')
    assert hasattr(instance, 'InstanceStub') == (audience == 'host')
    assert importlib.util.find_spec('harmos.context.v1.time_pb2_grpc') is None
    from harmos.context.v1 import storage_pb2 as context_storage
    from harmos.v1 import storage_types_pb2 as payload
    context_storage.StorageCreateFileProductionRequest(metadata=payload.FileMetadata(id='upload'))
else:
    assert importlib.util.find_spec('harmos.plugin') is None
    assert importlib.util.find_spec('harmos.context') is None
if audience != 'plugin':
    from harmos.v1 import invocations_pb2_grpc as invocations
    assert hasattr(invocations, 'InvocationsStub')
    assert hasattr(invocations, 'InvocationsServicer') == (audience == 'host')
else:
    assert importlib.util.find_spec('harmos.v1.invocations_pb2') is None
    assert importlib.util.find_spec('harmos.v1.storage_pb2') is None
    assert importlib.util.find_spec('harmos.v1.access') is None
"""
            try:
                with tempfile.TemporaryDirectory() as directory:
                    subprocess.run([sys.executable, "-c", program, str(Path(package).parent), audience], cwd=directory, check=True)
            finally:
                hook.finalize("standard", data, "")


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
