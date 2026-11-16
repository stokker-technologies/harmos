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
import shutil
import tempfile
from pathlib import Path

# Third Party Imports


# Local Imports


# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Directory:
    """Own Locked Scratch Storage Until close() Releases It."""

    def __init__(self) -> None:
        self.root = Path(tempfile.mkdtemp(prefix="project-tmp-"))
        try:
            self.owner = (self.root / "owner.lock").open("x+b")
        except BaseException:
            shutil.rmtree(self.root)
            raise
        try:
            if os.name == "nt":
                import msvcrt

                msvcrt.locking(self.owner.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl

                fcntl.flock(self.owner, fcntl.LOCK_EX | fcntl.LOCK_NB)
            self.owner.write(b"project-temporary-v1\n")
            self.owner.flush()
            self.path = self.root / "contents"
            self.path.mkdir()
        except BaseException:
            self.close()
            raise

    def close(self) -> None:
        self.owner.close()
        shutil.rmtree(self.root)


# === Public Functions === #


# === Private Functions === #

# === Entry Point === #
