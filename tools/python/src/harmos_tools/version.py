# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Check And Synchronize Declared Static Package Versions"""

# === Imports === #

# Standard Library Imports
import json
import re
import tomllib
from pathlib import Path

# Third Party Imports

# Local Imports
from . import _policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

VERSION = r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?"

# === Types === #

# === Classes === #

# === Public Functions === #


def versions(text: str, name: str) -> list[str]:
    if name.endswith("package.json"):
        value = json.loads(text).get("version")
        return [value] if isinstance(value, str) else []
    if name.endswith(".toml"):
        value = tomllib.loads(text)
        tables = [
            value.get("package", {}),
            value.get("project", {}),
            value.get("workspace", {}).get("package", {}),
        ]
        return [
            table["version"]
            for table in tables
            if isinstance(table.get("version"), str)
        ]
    found = re.search(r"^## \[?v?(" + VERSION + r")\]?", text, re.MULTILINE)
    return [found.group(1)] if found else []


def run(directory: Path, verb: str, *, value: str | None = None) -> None:
    settings = _policy.policy(directory, "version")
    target = value or settings.get("value")
    if not isinstance(target, str) or re.fullmatch(VERSION, target) is None:
        raise _policy.ToolError("version.value must declare a semantic version")
    paths = settings.get("files", ["Cargo.toml"])
    pending = []
    for name in paths:
        path = _policy.inside(directory, name)
        text = path.read_text(encoding="utf-8")
        current = versions(text, name)
        if not current:
            raise _policy.ToolError(f"no static version found: {name}")
        if verb == "check" and any(value != target for value in current):
            raise _policy.ToolError(f"{name}: versions {current} differ from {target}")
        if verb == "sync":
            if name.endswith("package.json"):
                document = json.loads(text)
                document["version"] = target
                text = json.dumps(document, indent=2) + "\n"
            elif name.endswith(".toml"):
                sections = re.split(r"(?m)(^\[.+?\]\s*$)", text)
                for index in range(1, len(sections), 2):
                    if sections[index].strip() in [
                        "[package]",
                        "[project]",
                        "[workspace.package]",
                    ]:
                        sections[index + 1] = re.sub(
                            r'(?m)^(version\s*=\s*)["\'][^"\']+["\']',
                            lambda match: match.group(1) + json.dumps(target),
                            sections[index + 1],
                        )
                text = "".join(sections)
            else:
                text = re.sub(
                    r"(?m)^(## \[?v?)" + VERSION,
                    lambda match: match.group(1) + target,
                    text,
                    count=1,
                )
            if versions(text, name) != [target] * len(current):
                raise _policy.ToolError(f"could not synchronize version: {name}")
            pending.append((path, text))
    for path, text in pending:
        path.write_text(text, encoding="utf-8")


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(_policy.main("version", ["check", "sync"], run))
