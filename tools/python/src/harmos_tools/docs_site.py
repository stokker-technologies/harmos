# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Validate Guide Sources And Static Documentation Links"""

# === Imports === #

# Standard Library Imports
import re
import urllib.parse
from html.parser import HTMLParser
from pathlib import Path

# Third Party Imports
# Local Imports
from . import _policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

SECTIONS = [
    "index",
    "getting-started/index",
    "tutorials/index",
    "reference/index",
    "contributing/index",
]

# === Types === #

# === Classes === #


class Links(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.urls: list[str] = []
        self.ids: set[str] = set()

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        attributes = dict(attrs)
        for key in ["href", "src"]:
            value = attributes.get(key)
            if value:
                self.urls.append(value)
        if attributes.get("id"):
            self.ids.add(str(attributes["id"]))


# === Public Functions === #


def sources(directory: Path) -> None:
    content = _policy.inside(directory, "docs/src/content/docs")
    pages = sorted([*content.rglob("*.md"), *content.rglob("*.mdx")])
    if not pages:
        raise _policy.ToolError("enabled guides require nonempty pages")
    for section in SECTIONS:
        if not any(
            (content / f"{section}.{extension}").is_file()
            for extension in ["md", "mdx"]
        ):
            raise _policy.ToolError(f"missing documentation section: {section}")
    for page in pages:
        text = page.read_text(encoding="utf-8")
        front = re.match(r"\A---\n(.*?)\n---\n(.*)", text, re.DOTALL)
        if not front or any(
            not re.search(rf"(?m)^{key}:\s*\S", front[1])
            for key in ["title", "description"]
        ):
            raise _policy.ToolError(f"{page}: title and description are required")
        opening = re.split(r"(?m)^#{1,6}\s", front[2])[0]
        opening = re.sub(r"(?s)import .*?;", "", opening).strip()
        if not opening:
            raise _policy.ToolError(f"{page}: opening paragraph is required")
        fences = re.findall(r"(?m)^\s*(`{3,}|~{3,})([^\n]*)", front[2])
        active = None
        for marker, metadata in fences:
            if active is None:
                if not metadata.strip():
                    raise _policy.ToolError(f"{page}: code fences require a language")
                active = marker[0]
            elif marker[0] == active:
                active = None
        if active:
            raise _policy.ToolError(f"{page}: unclosed code fence")


def static(directory: Path, base: str, *, shared: bool = False) -> list[str]:
    pages = sorted(directory.rglob("*.html"))
    if not pages:
        raise _policy.ToolError("documentation output has no HTML pages")
    parsed = {}
    for page in pages:
        parser = Links()
        text = page.read_text(encoding="utf-8")
        if not text.strip():
            raise _policy.ToolError(f"empty documentation page: {page}")
        parser.feed(text)
        parsed[page] = parser
    for page, parser in parsed.items():
        for url in parser.urls:
            parts = urllib.parse.urlsplit(url)
            if parts.scheme or parts.netloc:
                continue
            raw = urllib.parse.unquote(parts.path)
            if shared and raw in [
                base.rsplit("/", 2)[0] + "/versions.js",
                base.rsplit("/", 2)[0] + "/versions.json",
            ]:
                continue
            if raw.startswith("/"):
                if not raw.startswith(base):
                    raise _policy.ToolError(
                        f"{page}: link escapes documentation base: {url}"
                    )
                relative = raw.removeprefix(base)
                if shared and relative in ["versions.js", "versions.json"]:
                    continue
                target = _policy.inside(directory, relative)
            else:
                target = (
                    _policy.inside(
                        directory, str(page.parent.relative_to(directory) / raw)
                    )
                    if raw
                    else page
                )
            if target.is_dir():
                target /= "index.html"
            if not target.is_file():
                raise _policy.ToolError(f"{page}: missing local asset or page: {url}")
            if (
                parts.fragment
                and target in parsed
                and urllib.parse.unquote(parts.fragment) not in parsed[target].ids
            ):
                raise _policy.ToolError(f"{page}: missing anchor: {url}")
    return [page.relative_to(directory).as_posix() for page in pages]


# === Private Functions === #

# === Entry Point === #
