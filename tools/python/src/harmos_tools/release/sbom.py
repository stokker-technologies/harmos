# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Generate Locked Source Inventories And License Notices For Release Packages"""

# === Imports === #

# Standard Library Imports
import hashlib
import json
import tomllib
from pathlib import Path

from ..package import run as packages

# Third Party Imports
# Local Imports
from . import notices as licenses
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def generate(directory: Path, abi: str, output: Path) -> None:
    """Inventory Cargo's Locked Target Graph And The SDK's Locked Python Dependencies."""
    command = ["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", abi]
    metadata = json.loads(policy.execute(directory, command, capture=True))
    nodes = metadata["resolve"]["nodes"]
    selected = {node["id"] for node in nodes}
    references = {
        item["id"]: f"cargo:{item['name']}@{item['version']}:"
        + hashlib.sha256((item.get("source") or "workspace").encode()).hexdigest()[:12]
        for item in metadata["packages"]
        if item["id"] in selected
    }
    components = []
    notices = []
    for item in sorted(metadata["packages"], key=lambda item: item["id"]):
        if item["id"] not in selected:
            continue
        component = {"type": "library", "bom-ref": references[item["id"]], "name": item["name"], "version": item["version"]}
        if item.get("source", "") and item["source"].startswith("registry+"):
            component["purl"] = f"pkg:cargo/{item['name']}@{item['version']}"
        if item.get("license"):
            component["licenses"] = [{"expression": item["license"]}]
        components.append(component)
        notices.extend([f"{item['name']} {item['version']}", item.get("license") or "See Package License"])
        root = Path(item["manifest_path"]).parent
        sources = {
            path for path in root.iterdir() if path.is_file() and path.name.lower().startswith(("license", "licence", "notice", "copying"))
        }
        if item.get("license_file"):
            sources.add(root / item["license_file"])
        for source in sorted(sources):
            notices.append(source.read_text(encoding="utf-8", errors="replace"))
    document = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "name": "harmos",
                "properties": [
                    {
                        "name": "harmos:inventory-scope",
                        "value": f"Conservative locked Cargo source graph for {abi}; includes build and development dependencies",
                    }
                ],
            }
        },
        "components": components,
        "dependencies": [
            {"ref": references[node["id"]], "dependsOn": [references[dependency] for dependency in node["dependencies"]]} for node in nodes
        ],
    }
    policy.write_json(output / "crates" / abi / "workspace.cdx.json", document)
    lock = tomllib.loads(packages.regular(directory, "sdk/python/uv.lock").read_text(encoding="utf-8"))
    python = []
    for item in lock["package"]:
        if "registry" not in item.get("source", {}):
            continue
        python.append(
            {"type": "library", "name": item["name"], "version": item["version"], "purl": f"pkg:pypi/{item['name']}@{item['version']}"}
        )
        notices.append(f"Python: {item['name']} {item['version']} ({item['source']['registry']})")
    python_notices, _ = licenses.python_notices(lock)
    notices.append(python_notices)
    policy.write_json(
        output / "python/any/sdk.cdx.json", {"bomFormat": "CycloneDX", "specVersion": "1.5", "version": 1, "components": python}
    )
    (output / "NOTICE").write_text(packages.regular(directory, "LICENSE").read_text(encoding="utf-8"), encoding="utf-8")
    (output / "THIRD-PARTY-NOTICES").write_text("\n\n".join(notices) + "\n", encoding="utf-8")


# === Private Functions === #

# === Entry Point === #
