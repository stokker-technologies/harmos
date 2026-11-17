# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build or Preview One Documentation Entry from the Current Checkout"""

# === Imports === #

# Standard Library Imports
import argparse
import os
import re
import shutil
import subprocess
import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

# isort: split

# Third Party Imports

# isort: split

# Local Imports
from ..process import spawn_flags
from ..release import policy
from . import operations

# Type Checking Imports

# === Public Exports === #

# === Constants === #

SITE = "site"
OUTPUT = "target/docs"
ARCHIVE = "harmos-docs.zip"
STAMP = "docs.json"
FAVICON = "site/assets/logos/mark.svg"
DEVELOPMENT = "dev"
# Fixed Member Time Keeps Archives Of The Same Pages Byte-Identical.
EPOCH = (1980, 1, 1, 0, 0, 0)

# === Types === #

# === Classes === #


class _Collector(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.identifiers: set[str] = set()
        self.references: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        values = dict(attrs)
        if values.get("id"):
            self.identifiers.add(values["id"] or "")
        if tag in {"a", "link"} and values.get("href"):
            self.references.append(values["href"] or "")
        if tag in {"img", "script"} and values.get("src"):
            self.references.append(values["src"] or "")


# === Public Functions === #


def entry(directory: Path, *, release: bool) -> str:
    """The Release Line of the Current Version, or the Development Entry."""
    if not release:
        return DEVELOPMENT
    value = policy.policy(directory, "version").get("value", "")
    matched = re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", value)
    if matched is None:
        raise policy.ToolError("Build Documentation: version.value Must Be a Stable MAJOR.MINOR.PATCH Version")
    return f"{matched[1]}.{matched[2]}"


def build(directory: Path, *, release: bool = False, archive: bool = False) -> Path:
    """Generate References, Build the Entry into target/docs/<entry>/ and Check Its Links."""
    selected = entry(directory, release=release)
    operations.generate(directory)
    output = policy.inside(directory, f"{OUTPUT}/{selected}")
    if output.exists():
        shutil.rmtree(output)
    _astro(directory, selected, "build")
    shutil.copyfile(directory / FAVICON, output / "favicon.svg")
    broken = links(output, f"/{selected}/")
    if broken:
        raise policy.ToolError("Build Documentation: Broken Links:\n" + "\n".join(broken))
    version = policy.policy(directory, "version")["value"]
    policy.write_json(output / STAMP, {"entry": selected, "version": version, "revision": _revision(directory)})
    if archive:
        pack(output, policy.inside(directory, f"{OUTPUT}/{ARCHIVE}"))
    return output


def serve(directory: Path) -> None:
    """Preview the Development Entry with Live Reload of the Maintained Guides."""
    operations.generate(directory)
    _astro(directory, DEVELOPMENT, "dev", "--host", "127.0.0.1")


def links(output: Path, base: str) -> list[str]:
    """Every Same-Site Link or Fragment Whose Page or Heading Does Not Exist."""
    pages: dict[Path, set[str]] = {}

    def identifiers(page: Path) -> set[str]:
        if page not in pages:
            collector = _Collector()
            collector.feed(page.read_text(encoding="utf-8"))
            pages[page] = collector.identifiers
        return pages[page]

    broken = []
    for page in sorted(output.rglob("*.html")):
        collector = _Collector()
        collector.feed(page.read_text(encoding="utf-8"))
        for reference in collector.references:
            parsed = urlsplit(reference)
            if parsed.scheme or parsed.netloc or not (parsed.path or parsed.fragment):
                continue
            target = page
            if parsed.path:
                path = unquote(parsed.path)
                if path.startswith("/") and not path.startswith(base):
                    broken.append(f"{page.relative_to(output).as_posix()}: {reference} Leaves the Entry")
                    continue
                resolved = output / path.removeprefix(base) if path.startswith("/") else page.parent / path
                target = resolved / "index.html" if path.endswith("/") or resolved.is_dir() else resolved
            missing_page = not target.is_file()
            missing_heading = (
                not missing_page
                and bool(parsed.fragment)
                and target.suffix == ".html"
                and unquote(parsed.fragment) not in identifiers(target)
            )
            if missing_page or missing_heading:
                broken.append(f"{page.relative_to(output).as_posix()}: {reference}")
    return broken


def pack(output: Path, destination: Path) -> None:
    """Archive One Built Entry with Its Pages at the Archive Root."""
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_suffix(".tmp")
    archive = ZipFile(temporary, "w", compression=ZIP_DEFLATED)
    try:
        for path in sorted(output.rglob("*")):
            if path.is_symlink():
                raise policy.ToolError(f"Archive Documentation: Linked Output: {path}")
            if path.is_file():
                member = ZipInfo(path.relative_to(output).as_posix(), EPOCH)
                member.compress_type = ZIP_DEFLATED
                member.external_attr = 0o644 << 16
                archive.writestr(member, path.read_bytes())
    finally:
        archive.close()
    temporary.replace(destination)


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build or Preview the Documentation Site")
    parser.add_argument("operation", choices=("build", "serve"))
    parser.add_argument("--release", action="store_true", help="Build the Release Line Entry of the Current Version")
    parser.add_argument("--archive", action="store_true", help="Pack the Built Entry Into target/docs/harmos-docs.zip")
    options = parser.parse_args(arguments)
    directory = policy.root()
    try:
        if options.operation == "serve":
            serve(directory)
        else:
            print(build(directory, release=options.release, archive=options.archive))
    except (policy.ToolError, OSError, ValueError, KeyError) as error:
        print(f"Documentation: {error}", file=sys.stderr)
        return 1
    return 0


# === Private Functions === #


def _astro(directory: Path, selected: str, *arguments: str) -> None:
    """Run Astro for One Entry; the Site Configuration Writes It to target/docs/<entry>/."""
    executable = shutil.which("pnpm")
    if executable is None:
        raise policy.ToolError("Build Documentation: pnpm Is Not Installed; Run mise install")
    settings = policy.policy(directory, "release")
    if settings.get("provider") != "github":
        raise policy.ToolError("Build Documentation: Source Links Support the GitHub Provider Only")
    version = policy.policy(directory, "version")["value"]
    reference = _revision(directory) if selected == DEVELOPMENT else f"v{version}"
    environment = {
        **os.environ,
        "ASTRO_TELEMETRY_DISABLED": "1",
        "DOCS_ENTRY": selected,
        # A Release Line Links To Its Tag; Review Checkouts Are Merge Commits That Do Not Outlive The Review.
        "DOCS_SOURCE": f"https://github.com/{settings['repository']}/blob/{reference}/",
    }
    command = [executable, "--dir", SITE, "exec", "astro", *arguments]
    result = subprocess.run(command, cwd=directory, env=environment, check=False, creationflags=spawn_flags())
    if result.returncode:
        raise policy.ToolError(f"Build Documentation: astro {arguments[0]} Failed With Exit Code {result.returncode}")


def _revision(directory: Path) -> str:
    return policy.execute(directory, ["git", "rev-parse", "HEAD"], capture=True).strip()


# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
