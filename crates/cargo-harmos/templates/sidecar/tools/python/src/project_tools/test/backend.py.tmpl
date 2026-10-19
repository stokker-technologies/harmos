# Rendered by stkr project update from templates/python/test/backend.py; sha256:95266f426cd314cb457a0c69b3e494f0086409b515688050445d12cac97faf21
# ***
#
# __PRODUCT_NAME__
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Bind Prepared Python Backend Inputs"""

# === Imports === #

# Standard Library Imports
import json
import os
import subprocess
import tomllib
from collections.abc import Callable
from pathlib import Path

# Third Party Imports

# Local Imports
from . import policy

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# Probe The Selected Interpreter Rather Than The Tooling Process's Environment.
PROBE = r"""
import importlib.metadata as metadata
import importlib.util
import json
import sys
from pathlib import Path
from packaging.requirements import Requirement

requirements = json.loads(sys.argv[1])
plugins = sorted((entry.group, entry.name, entry.value, entry.dist.metadata['Name'])
                 for entry in metadata.entry_points() if entry.group not in ('console_scripts', 'gui_scripts'))
pending = [(text, ('',)) for text in requirements]
pending.append(('packaging==' + metadata.version('packaging'), ('',)))
pending.extend((name + '==' + metadata.version(name), ('',)) for _, _, _, name in plugins)
selected = {}
extras = {}
roots = set()
modules = {}
while pending:
    text, context = pending.pop()
    requirement = Requirement(text)
    if requirement.marker and not any(requirement.marker.evaluate({'extra': extra}) for extra in context):
        continue
    distribution = metadata.distribution(requirement.name)
    if not requirement.specifier.contains(distribution.version, prereleases=True):
        raise ValueError('prepared backend requirement mismatch: ' + str(requirement))
    key = distribution.metadata['Name'].lower().replace('_', '-')
    if key in selected and requirement.extras.issubset(extras[key]):
        continue
    selected[key] = distribution.version
    extras.setdefault(key, set()).update(requirement.extras)
    if not distribution.files:
        raise ValueError('prepared backend distribution has no file inventory: ' + key)
    for file in distribution.files:
        path = Path(distribution.locate_file(file)).resolve()
        if '.dist-info' in str(file):
            for parent in path.parents:
                if parent.name.endswith('.dist-info'):
                    roots.add(str(parent))
                    break
        top = Path(file).parts[0]
        if top.isidentifier() or top.endswith('.py') and top[:-3].isidentifier():
            root = Path(distribution.locate_file(top)).resolve()
            if root.exists():
                roots.add(str(root))
                modules.setdefault(top.removesuffix('.py'), set()).add(root)
    pending.extend((text, ('', *extras[key])) for text in distribution.requires or ())

backend = importlib.util.find_spec(sys.argv[2].split(':')[0])
if backend is None or backend.origin is None or not any(Path(backend.origin).resolve().is_relative_to(Path(root)) for root in roots if Path(root).is_dir()):
    raise ValueError('build-backend does not belong to the captured installed closure')
for name, expected in modules.items():
    specification = importlib.util.find_spec(name)
    if specification is None:
        raise ValueError('prepared backend module is unavailable: ' + name)
    locations = list(specification.submodule_search_locations or ())
    if specification.origin not in (None, 'namespace', 'built-in', 'frozen'):
        locations.append(specification.origin)
    if not locations or any(not any(Path(location).resolve() == root or Path(location).resolve().is_relative_to(root) for root in expected) for location in locations):
        raise ValueError('prepared backend import shadow: ' + name)

search = [str(Path(path or '.').resolve()) for path in sys.path]
shared = set()
for name in search:
    directory = Path(name)
    if directory.is_dir():
        shared.update(str(path) for path in directory.glob('*.pth'))
        for module in ('sitecustomize', 'usercustomize'):
            for candidate in (directory / (module + '.py'), directory / (module + '.pyc'), directory / module):
                if candidate.exists():
                    shared.add(str(candidate))
for name in (Path(sys.prefix) / 'pyvenv.cfg', Path(sys.base_prefix) / 'pyvenv.cfg'):
    if name.is_file():
        shared.add(str(name))
print(json.dumps({'distributions': selected, 'roots': sorted(roots), 'shared': sorted(shared),
                  'sys_path': search, 'plugins': plugins, 'python': sys.version,
                  'prefix': sys.prefix, 'base_prefix': sys.base_prefix}))
"""


# === Types === #

# === Classes === #

# === Public Functions === #


def capture(directory: Path, python: str, manifest: Path, digest: Callable[[Path], str]) -> dict:
    """Snapshot Only The Prepared Backend Closure And Shared Import Inputs."""
    manifest = manifest if manifest.is_absolute() else directory / manifest
    document = tomllib.loads(manifest.read_text(encoding="utf-8"))
    build = document.get("build-system", {})
    requirements = build.get("requires", [])
    if not requirements or not build.get("build-backend") or build.get("backend-path"):
        raise policy.TestError("prepared backend requires declared installed distributions; local backend-path is unsupported")
    try:
        result = subprocess.run(
            [python, "-c", PROBE, json.dumps(requirements), build["build-backend"]],
            cwd=manifest.parent,
            capture_output=True,
            text=True,
            check=False,
            env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
        )
    except OSError as error:
        raise policy.TestError(f"prepared backend interpreter is unavailable: {error}") from error
    if result.returncode != 0:
        raise policy.TestError("prepared backend closure is unavailable or shadowed: " + result.stderr.strip())
    selected = json.loads(result.stdout)
    files = {str(manifest.resolve()): digest(manifest)}
    for name in (*selected["roots"], *selected["shared"]):
        root = Path(name)
        paths = [root] if root.is_file() else list(root.rglob("*"))
        for path in paths:
            if path.is_symlink():
                raise policy.TestError(f"unbounded symbolic link in prepared backend: {path}")
            if path.is_file():
                files[str(path)] = digest(path)
    return {"manifest": str(manifest.resolve()), "interpreter": python, **selected, "files": files}


# === Private Functions === #

# === Entry Point === #
