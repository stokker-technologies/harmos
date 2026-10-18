# Rendered by stkr project update from templates/python/test/cache.py; sha256:25c47d607b7d30827308b4f1d79385aad6d0ad8befad4d1cd056fa9b987cfb15
# ***
#
# __PRODUCT_NAME__
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Start The Compiler Cache With A Short Notification Socket Path."""

# === Imports === #

# Standard Library Imports
import os
import shutil
import subprocess
import sys
from pathlib import Path

# Third Party Imports

# Local Imports
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def prepare() -> None:
    """Start Or Contact Sccache Without Changing The Compiler's Temporary Directory."""
    wrapper = os.environ.get("RUSTC_WRAPPER", "")
    if os.name == "nt" or Path(wrapper).name != "sccache" or not shutil.which(wrapper):
        return
    runtime = os.environ.get("SCCACHE_STARTUP_DIR") or os.environ.get("XDG_RUNTIME_DIR") or "/tmp"
    # Sccache Appends Its Random Directory And /sock; Leave Room Below BSD's SUN_LEN.
    if not Path(runtime).is_dir() or len(os.fsencode(runtime)) + 32 >= 104:
        raise policy.TestError("sccache requires a short existing runtime directory in SCCACHE_STARTUP_DIR")
    environment = dict(os.environ, TMPDIR=runtime)
    result = subprocess.run([wrapper, "--dist-status"], env=environment, stdout=subprocess.DEVNULL, check=False)
    if result.returncode:
        raise policy.TestError("could not prepare the compiler cache server")


def main() -> int:
    try:
        prepare()
        arguments = sys.argv[1:]
        if arguments[:1] == ["--"]:
            arguments = arguments[1:]
        return subprocess.run(arguments, check=False).returncode
    except (policy.TestError, OSError) as error:
        print(f"cache: {error}", file=sys.stderr)
        return 1


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
