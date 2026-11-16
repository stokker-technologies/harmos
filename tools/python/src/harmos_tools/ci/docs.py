# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Check Guides and Local Links"""

# === Imports === #

# Standard Library Imports
import re
import subprocess
from pathlib import Path
from urllib.parse import unquote, urlsplit

# Third Party Imports
# Local Imports
from ..process import spawn_flags

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def check_guides(root: Path) -> list[str]:
    wire = re.compile(r"\b_wire\b|\w+_wire\b|\bHarmos\.Internal\b|\b(?:[A-Za-z_]\w*(?:\.|::))+v\d+\b|[/\\]v\d+[/\\]")
    grpc_path = re.compile(r"/?[a-zA-Z_]\w*(?:\.\w+)*\.v\d+(?:\.\w+)*/[A-Za-z_]\w*")
    candidates = [root / "README.md", *sorted((root / "docs").rglob("*.md"))]
    findings = []
    for path in candidates:
        if not path.is_file() or path == root / "docs/architecture.md":
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            line = grpc_path.sub("", line)
            line = re.sub(r"\]\([^)]*\)", "]()", line)
            if wire.search(line):
                findings.append(f"{path.relative_to(root).as_posix()}:{number}: Public Guide Names a Wire Package")
    return findings


def check_links(root: Path, paths: list[str]) -> list[str]:
    findings = []
    for relative in paths:
        path = root / relative
        if path.suffix.lower() != ".md" or not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        text = re.sub(r"```.*?```|~~~.*?~~~", "", text, flags=re.DOTALL)
        links = re.findall(r"!?\[[^]\n]*\]\(<?([^\s)>]+)>?(?:\s+[^)]*)?\)", text)
        links += re.findall(r"^\s*\[[^]\n]+\]:\s*<?([^\s>]+)>?", text, flags=re.MULTILINE)
        for link in links:
            parsed = urlsplit(link)
            if parsed.scheme or parsed.netloc or not parsed.path:
                continue
            destination = unquote(parsed.path)
            target = root / destination.lstrip("/") if destination.startswith("/") else path.parent / destination
            if not target.exists():
                findings.append(f"{relative}: Missing Local Link {link}; Correct the link or restore its destination")
    return findings


def main() -> None:
    root = Path.cwd()
    command = ["git", "ls-files", "-z", "--", "*.md"]
    result = subprocess.run(command, check=True, capture_output=True, creationflags=spawn_flags())
    paths = result.stdout.decode("utf-8").rstrip("\0").split("\0")
    findings = check_guides(root) + check_links(root, paths)
    if findings:
        raise ValueError("Check Documentation:\n" + "\n".join(findings))
    print("Documentation: Guides and Local Links Passed")


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
