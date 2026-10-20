# Rendered by stkr project update from templates/python/temporary/__init__.py; sha256:256bc5c9625edba17a53057058a85cce06de12c1fa419fac25c3a913b696dfe8
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Own Disposable Directories In Normal System Temporary Storage"""

# === Imports === #

# Standard Library Imports
import os
import tempfile
from collections.abc import Generator
from contextlib import contextmanager
from pathlib import Path

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


@contextmanager
def directory() -> Generator[Path]:
    with tempfile.TemporaryDirectory(prefix="stkr-tmp-") as name:
        path = Path(name)
        with (path / "owner.lock").open("x+b") as owner:
            if os.name == "nt":
                import msvcrt

                msvcrt.locking(owner.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl

                fcntl.flock(owner, fcntl.LOCK_EX | fcntl.LOCK_NB)
            owner.write(b"stkr-temporary-v1\n")
            owner.flush()
            contents = path / "contents"
            contents.mkdir()
            yield contents


# === Private Functions === #

# === Entry Point === #
