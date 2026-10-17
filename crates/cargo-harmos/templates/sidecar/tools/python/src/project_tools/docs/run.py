# Rendered by stkr project update from templates/python/docs/run.py; sha256:306ee49dc335e0d1540f32f3ddcbc6f0fd014bc1c34aea9c4e2d6c4e1e56706e
# ***
#
# __PRODUCT_NAME__
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build, Verify, Assemble, Smoke, And Publish Versioned Documentation"""

# === Imports === #

# Standard Library Imports
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

# Third Party Imports
# Local Imports
from ..release import policy
from . import artifacts, registry, site

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def settings(directory: Path) -> dict:
    config = policy.config(directory)
    docs = policy.policy(directory, "docs")
    if any(name in docs for name in ["build", "publish", "directory"]):
        raise policy.ToolError(
            "docs.build/publish/directory command wrappers are retired; configure guides/api and native mise build tasks"
        )
    if not docs.get("guides", False) and not docs.get("api", False):
        return docs
    value = config.get("version", {}).get("value")
    values = set([value] if value else [])
    if (directory / "Cargo.toml").is_file():
        metadata = json.loads(
            policy.execute(
                directory,
                [
                    "cargo",
                    "metadata",
                    "--locked",
                    "--offline",
                    "--no-deps",
                    "--format-version",
                    "1",
                ],
                capture=True,
            )
        )
        members = set(metadata["workspace_members"])
        values.update(package["version"] for package in metadata["packages"] if package["id"] in members)
        docs["packages"] = [package for package in metadata["packages"] if package["id"] in members]
    elif not value:
        raise policy.ToolError("non-Cargo documentation requires version.value")
    if len(values) != 1:
        raise policy.ToolError("documentation requires coherent project/package versions")
    docs["version"] = values.pop()
    if not artifacts.eligible(docs, docs["version"]):
        raise policy.ToolError("documentation is prohibited by [docs] policy")
    base = docs.get("base", "/")
    if not isinstance(base, str) or not re.fullmatch(r"/(?:[A-Za-z0-9-]+/)*", base):
        raise policy.ToolError("docs.base must be an absolute directory URL with a trailing slash")
    docs["base"] = base
    docs["title"] = docs.get("title", config.get("project", {}).get("name", "Documentation"))
    if "value" in config.get("version", {}):
        policy.execute(directory, ["mise", "run", "version:check"])
    if docs.get("api"):
        crates = docs.get("crates")
        if not isinstance(crates, list) or not crates or any(not isinstance(name, str) for name in crates):
            raise policy.ToolError("docs.crates must explicitly select Cargo workspace packages")
        packages = {package["name"]: package for package in docs.get("packages", [])}
        if any(name not in packages for name in crates):
            raise policy.ToolError("docs.crates contains a non-workspace package")
        features = docs.get("features", [])
        if not isinstance(features, list) or any(not isinstance(name, str) or not name for name in features):
            raise policy.ToolError("docs.features must contain feature names")
        for feature in features:
            if "/" in feature:
                owner, name = feature.split("/", 1)
                known = owner in crates and name in packages[owner].get("features", {})
            else:
                known = any(feature in packages[name].get("features", {}) for name in crates)
            if not known:
                raise policy.ToolError(f"unknown selected Cargo documentation feature: {feature}")
        if not isinstance(docs.get("no_default_features", False), bool):
            raise policy.ToolError("docs.no_default_features must be a boolean")
    return docs


def selected(directory: Path, docs: dict, selector: str | None) -> tuple[str, str]:
    revision = policy.execute(directory, ["git", "rev-parse", "HEAD"], capture=True).strip()
    selector = selector or os.environ.get("DOCS_SELECTOR") or os.environ.get("CI_COMMIT_TAG")
    if not selector:
        raise policy.ToolError("select exact vX.Y.Z or explicitly enabled development with --selector/DOCS_SELECTOR")
    if selector == "development":
        if not docs.get("development", False):
            raise policy.ToolError("docs.development must explicitly enable development publication")
        main = docs.get("main_ref", "origin/main")
        main_revision = policy.execute(directory, ["git", "rev-parse", main], capture=True).strip()
        if main_revision != revision:
            raise policy.ToolError("development documentation must be built from current main")
    else:
        if selector != "v" + docs["version"]:
            raise policy.ToolError("release selector must match the project version")
        tag = policy.execute(
            directory,
            ["git", "rev-parse", "--verify", f"refs/tags/{selector}^" + "{commit}"],
            capture=True,
        ).strip()
        if tag != revision:
            raise policy.ToolError("release documentation must be built from its exact source tag")
    if policy.execute(
        directory,
        ["git", "status", "--porcelain", "--untracked-files=all"],
        capture=True,
    ).strip():
        raise policy.ToolError("documentation build/publication requires a clean source checkout")
    return selector, revision


def inputs(directory: Path) -> str:
    names = policy.execute(directory, ["git", "ls-files", "-z"], capture=True).split("\0")
    values = {}
    for name in sorted(filter(None, names)):
        path = policy.inside(directory, name)
        if path.is_symlink():
            raise policy.ToolError(f"linked documentation source input: {name}")
        values[name] = artifacts.digest(path.read_bytes())
    return artifacts.digest(json.dumps(values, sort_keys=True).encode())


def tools(directory: Path, docs: dict) -> dict[str, str]:
    names = ["node", "npm"] if docs.get("guides") else []
    if docs.get("api"):
        names += ["cargo", "rustdoc"]
    result = {name: policy.execute(directory, [name, "--version"], capture=True).strip() for name in names}
    if docs.get("guides"):
        package = json.loads((directory / "docs/package.json").read_text(encoding="utf-8"))
        for name in ["node", "npm"]:
            expected = package.get("engines", {}).get(name)
            if not isinstance(expected, str) or result[name].removeprefix("v") != expected:
                raise policy.ToolError(f"Starlight requires an exact installed {name} pin matching docs/package.json")
    return result


def stage(directory: Path) -> Path:
    return policy.inside(directory, "target/documentation")


def build(directory: Path, docs: dict, profile: str, selector: str | None) -> None:
    tasks = json.loads(policy.execute(directory, ["mise", "tasks", "ls", "--json"], capture=True))
    if f"build:docs:{profile}" not in {entry["name"] for entry in tasks}:
        raise policy.ToolError("documentation requires a native build:docs:<profile> task")
    selector, revision = selected(directory, docs, selector)
    before = inputs(directory)
    versions = tools(directory, docs)
    output = stage(directory)
    if output.exists():
        shutil.rmtree(output)
    folder = output / "files"
    folder.mkdir(parents=True)
    base = docs["base"] + selector + "/"
    if docs.get("guides"):
        site.sources(directory)
        source = policy.inside(directory, "docs")
        if not (source / "package-lock.json").is_file():
            raise policy.ToolError("Starlight requires a committed package-lock.json")
        policy.execute(source, ["npm", "ci", "--ignore-scripts"])
        environment = os.environ.copy()
        environment.update(
            {
                "DOCS_BASE": base,
                "DOCS_OUTPUT": str(folder),
                "DOCS_TITLE": docs["title"],
                "DOCS_METADATA": docs["base"] + "versions.json",
                "DOCS_NAVIGATION": docs["base"] + "versions.js",
            }
        )
        result = subprocess.run(["npm", "run", "build"], cwd=source, env=environment, check=False)
        if result.returncode:
            raise policy.ToolError("Starlight build failed")
    home = "index.html"
    if docs.get("api"):
        packages = docs.get("crates")
        if not isinstance(packages, list) or not packages or any(not isinstance(name, str) for name in packages):
            raise policy.ToolError("docs.crates must explicitly select nonempty Cargo packages")
        declared = {package["name"]: package for package in docs.get("packages", [])}
        if any(name not in declared for name in packages):
            raise policy.ToolError("docs.crates contains a non-workspace package")
        features = docs.get("features", [])
        if not isinstance(features, list) or any(not isinstance(name, str) for name in features):
            raise policy.ToolError("docs.features must be an array of feature names")
        target = output / "cargo"
        command = [
            "cargo",
            "doc",
            "--locked",
            "--offline",
            "--no-deps",
            "--target-dir",
            str(target),
        ]
        for name in packages:
            command += ["--package", name]
        if features:
            command += ["--features", ",".join(features)]
        if docs.get("no_default_features", False):
            command += ["--no-default-features"]
        if profile != "dev":
            command += ["--profile", profile]
        policy.execute(directory, command)
        api = folder / "api"
        shutil.copytree(target / "doc", api)
        home = "index.html" if docs.get("guides") else "api/" + packages[0].replace("-", "_") + "/index.html"
        navigation = f'<div data-docs-navigation data-metadata="{docs["base"]}versions.json"></div><script src="{docs["base"]}versions.js" defer></script>'
        for page in api.rglob("*.html"):
            text = page.read_text(encoding="utf-8")
            text = re.sub(r"(<body\b[^>]*>)", lambda match: match[1] + navigation, text, count=1)
            page.write_text(text, encoding="utf-8")
        for name in packages:
            if not (api / name.replace("-", "_") / "index.html").is_file():
                raise policy.ToolError(f"Rustdoc package has no reference output: {name}")
    if not docs.get("guides"):
        (folder / "index.html").write_text(
            f'<html><head><meta http-equiv="refresh" content="0;url={home}"></head><body><a href="{home}">API Reference</a></body></html>',
            encoding="utf-8",
        )
    pages = site.static(folder, base, shared=True)
    if not (folder / home).is_file():
        raise policy.ToolError("documentation home is missing")
    if inputs(directory) != before:
        raise policy.ToolError("documentation source changed during build")
    manifest = {
        "schema": 1,
        "selector": selector,
        "version": docs["version"],
        "revision": revision,
        "inputs": before,
        "tools": versions,
        "profile": profile,
        "base": docs["base"],
        "home": home,
        "pages": pages,
        "files": artifacts.inventory(folder),
        "built_at": time.time_ns(),
    }
    artifacts.verify(folder, manifest)
    policy.write_json(output / "manifest.json", manifest)


def verified(directory: Path, docs: dict) -> dict:
    output = stage(directory)
    manifest = json.loads((output / "manifest.json").read_text(encoding="utf-8"))
    artifacts.verify(output / "files", manifest)
    _, revision = selected(directory, docs, manifest["selector"])
    if manifest["revision"] != revision or manifest["inputs"] != inputs(directory) or manifest["tools"] != tools(directory, docs):
        raise policy.ToolError("documentation artifact is stale for the current source/tools")
    if manifest["version"] != docs["version"] or manifest["base"] != docs["base"]:
        raise policy.ToolError("documentation artifact differs from current policy")
    return manifest


def assemble(directory: Path, docs: dict) -> None:
    manifest = verified(directory, docs)
    output = stage(directory)
    retained = output / "retained"
    if retained.exists():
        shutil.rmtree(retained)
    retained.mkdir()
    remote = docs.get("registry", {})
    selected_artifacts = []
    for folder, previous in registry.retained(remote, retained):
        if previous["selector"] == manifest["selector"]:
            if manifest["selector"] != "development" and previous != manifest:
                raise policy.ToolError("immutable documentation version collision during assembly")
            continue
        selected_artifacts.append((folder, previous))
    selected_artifacts.append((output / "files", manifest))
    destination = policy.inside(directory, "site")
    catalog = artifacts.assemble(destination, selected_artifacts, docs["base"], docs.get("roadmap"))
    receipt = {
        "revision": manifest["revision"],
        "inputs": manifest["inputs"],
        "artifact": artifacts.digest(json.dumps(manifest, sort_keys=True).encode()),
        "files": artifacts.inventory(destination),
        "catalog": catalog,
    }
    policy.write_json(output / "assembled.json", receipt)


def assembled(directory: Path, docs: dict) -> dict:
    manifest = verified(directory, docs)
    receipt = json.loads((stage(directory) / "assembled.json").read_text(encoding="utf-8"))
    if receipt["files"] != artifacts.inventory(directory / "site") or receipt["artifact"] != artifacts.digest(
        json.dumps(manifest, sort_keys=True).encode()
    ):
        raise policy.ToolError("assembled documentation is stale or modified")
    return receipt


def probe(url: str, expected: bytes, status: int = 200) -> None:
    try:
        with urllib.request.urlopen(url, timeout=5) as response:
            actual, body = response.status, response.read()
    except urllib.error.HTTPError as error:
        actual, body = error.code, error.read()
    if actual != status or (expected and expected not in body):
        raise policy.ToolError(f"documentation smoke content/status differs: {url}")


def smoke(directory: Path, docs: dict) -> None:
    receipt = assembled(directory, docs)
    output = stage(directory)
    (output / "smoke.json").unlink(missing_ok=True)
    image = docs.get("image")
    if not isinstance(image, str) or not image:
        raise policy.ToolError("docs.image must declare the assembled container image")
    context = output / "container"
    if context.exists():
        shutil.rmtree(context)
    (context / "docker").mkdir(parents=True)
    for name in ["docs.Dockerfile", "docs.Caddyfile"]:
        shutil.copyfile(policy.inside(directory, "docker/" + name), context / "docker" / name)
    shutil.copytree(directory / "site", context / "site")
    policy.execute(
        directory,
        [
            "docker",
            "build",
            "--file",
            str(context / "docker/docs.Dockerfile"),
            "--tag",
            image,
            str(context),
        ],
    )
    image_id = policy.execute(
        directory,
        ["docker", "image", "inspect", "--format", "{" + "{.Id}" + "}", image],
        capture=True,
    ).strip()
    container = policy.execute(
        directory,
        ["docker", "run", "--detach", "--rm", "--publish", "127.0.0.1::8000", image_id],
        capture=True,
    ).strip()
    try:
        port = policy.execute(directory, ["docker", "port", container, "8000/tcp"], capture=True).strip().split(":")[-1]
        address = f"http://127.0.0.1:{int(port)}"
        deadline = time.monotonic() + 20
        while True:
            try:
                probe(address + "/health", b"ok")
                break
            except (OSError, policy.ToolError):
                if time.monotonic() >= deadline:
                    raise
                time.sleep(0.2)
        probe(
            address + docs["base"] + "versions.json",
            json.dumps(receipt["catalog"]["latest"]).encode(),
        )
        probe(address + docs["base"] + "missing-documentation-page", b"", 404)
        for entry in receipt["catalog"]["versions"]:
            url = docs["base"] + entry["selector"] + "/" + entry["home"]
            expected = (directory / "site" / url.lstrip("/")).read_bytes()
            probe(address + url, expected)
            prefix = docs["base"].strip("/")
            version_prefix = "/".join(filter(None, [prefix, entry["selector"]])) + "/"
            assets = [name for name in receipt["files"] if name.startswith(version_prefix) and name.endswith((".css", ".js"))]
            if not assets:
                raise policy.ToolError("documentation smoke requires actual built CSS/JS assets")
            for name in assets:
                probe(address + "/" + name, (directory / "site" / name).read_bytes())
        probe(address + docs["base"], b"Documentation")
        if docs["base"] != "/":
            probe(address + docs["base"].rstrip("/"), b"Documentation")
        if receipt["catalog"]["latest"]:
            probe(address + docs["base"] + "latest/", b"Latest Stable Documentation")
    finally:
        policy.execute(directory, ["docker", "stop", container])
    if assembled(directory, docs) != receipt:
        raise policy.ToolError("documentation assembly changed during smoke")
    policy.write_json(
        output / "smoke.json",
        {
            "assembled": receipt,
            "image": image,
            "image_id": image_id,
            "docker": policy.execute(directory, ["docker", "--version"], capture=True).strip(),
        },
    )


def publish(directory: Path, docs: dict) -> None:
    manifest = verified(directory, docs)
    receipt = assembled(directory, docs)
    smoke_receipt = json.loads((stage(directory) / "smoke.json").read_text(encoding="utf-8"))
    image = docs.get("image")
    if not isinstance(image, str) or not image:
        raise policy.ToolError("docs.image must declare the assembled container image")
    identity = policy.execute(
        directory,
        ["docker", "image", "inspect", "--format", "{" + "{.Id}" + "}", image],
        capture=True,
    ).strip()
    if (
        smoke_receipt["assembled"] != receipt
        or smoke_receipt["image"] != image
        or smoke_receipt["image_id"] != identity
        or smoke_receipt["docker"] != policy.execute(directory, ["docker", "--version"], capture=True).strip()
    ):
        raise policy.ToolError("publication requires current successful container smoke evidence")
    remote = stage(directory) / "publication-retained"
    if remote.exists():
        shutil.rmtree(remote)
    remote.mkdir()
    retained = registry.retained(docs.get("registry", {}), remote)
    published_versions = {item["selector"]: item for item in receipt["catalog"]["versions"]}
    for _, previous in retained:
        if previous["selector"] == manifest["selector"]:
            if manifest["selector"] != "development" and previous != manifest:
                raise policy.ToolError("immutable documentation version collision before publication")
            continue
        expected = {key: previous[key] for key in ["selector", "version", "revision", "pages", "home"]}
        if published_versions.get(previous["selector"]) != expected:
            raise policy.ToolError("registry versions changed since assembly; assemble and smoke again")
    remote_selectors = {previous["selector"] for _, previous in retained} | {manifest["selector"]}
    if set(published_versions) != remote_selectors:
        raise policy.ToolError("registry history changed since assembly; preserve the current registry history")
    archive_digest = registry.publish(docs.get("registry", {}), stage(directory) / "files", manifest)
    if assembled(directory, docs) != receipt:
        raise policy.ToolError("source or assembled content changed during registry publication")
    policy.execute(directory, ["docker", "push", image])
    digests = json.loads(
        policy.execute(
            directory,
            [
                "docker",
                "image",
                "inspect",
                "--format",
                "{" + "{json .RepoDigests}" + "}",
                image,
            ],
            capture=True,
        )
    )
    if not digests:
        raise policy.ToolError("published container has no registry digest")
    repository = image.rsplit(":", 1)[0] if ":" in image.rsplit("/", 1)[-1] else image
    matching = [
        value
        for value in digests
        if isinstance(value, str) and value.startswith(repository + "@sha256:") and re.fullmatch(r"[0-9a-f]{64}", value.rsplit(":", 1)[-1])
    ]
    if not matching:
        raise policy.ToolError("published container digest does not identify the declared repository")
    published = matching[0]
    policy.execute(
        directory,
        ["docker", "buildx", "imagetools", "inspect", published],
        capture=True,
    )
    policy.write_json(
        stage(directory) / "published.json",
        {
            "revision": manifest["revision"],
            "artifact_sha256": archive_digest,
            "image": published,
            "assembled": receipt,
        },
    )
    print(json.dumps({"image": published, "artifact_sha256": archive_digest}))


def run(
    directory: Path,
    verb: str,
    *,
    profile: str | None = None,
    selector: str | None = None,
) -> None:
    docs = settings(directory)
    if not docs.get("guides", False) and not docs.get("api", False):
        if verb == "check":
            return
        raise policy.ToolError("documentation is disabled; no site or publication is produced")
    if verb == "check":
        if docs.get("guides"):
            site.sources(directory)
        if (stage(directory) / "manifest.json").is_file():
            verified(directory, docs)
    elif verb == "build":
        if not profile:
            raise policy.ToolError("build requires --profile")
        build(directory, docs, profile, selector)
    elif verb == "assemble":
        assemble(directory, docs)
    elif verb == "smoke":
        smoke(directory, docs)
    elif verb == "publish":
        publish(directory, docs)
    else:
        raise policy.ToolError("unknown documentation operation")


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Versioned Documentation")
    parser.add_argument("verb", choices=["check", "build", "assemble", "smoke", "publish"])
    parser.add_argument("--profile")
    parser.add_argument("--selector")
    options = parser.parse_args()
    try:
        run(
            policy.root(),
            options.verb,
            profile=options.profile,
            selector=options.selector,
        )
    except (policy.ToolError, OSError, ValueError, KeyError, TypeError) as error:
        print(f"docs: {error}", file=sys.stderr)
        raise SystemExit(1) from error
