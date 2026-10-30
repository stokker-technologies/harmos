"""Prepare Native Tools And Stable Toolchain Roots Before Mise."""

# === Imports === #

# Standard Library Imports
import argparse
import contextlib
import ctypes
import hashlib
import json
import os
import re
import runpy
import shlex
import shutil
import subprocess
import sys
import urllib.request
import zipfile
from pathlib import Path

# Third Party Imports
# Local Imports
# Type Checking Imports

# === Public Exports === #

# === Constants === #

spawn_flags = runpy.run_path(str(Path(__file__).resolve().parents[1] / "process.py"))["spawn_flags"]

ZSTD_URL = "https://github.com/facebook/zstd/releases/download/v1.5.7/zstd-v1.5.7-win64.zip"
ZSTD_SHA256 = "acb4e8111511749dc7a3ebedca9b04190e37a17afeb73f55d4425dbf0b90fad9"
MISE_URL = "https://github.com/jdx/mise/releases/download/v2026.10.17/mise-v2026.10.17-windows-x64.zip"
MISE_SHA256 = "32bcfda5f72bf7cb189e18ba93c2798bffcb4a678fd004e3127780363f2d8c8c"
EXPORTED = {}

# === Types === #

# === Classes === #

# === Public Functions === #


@contextlib.contextmanager
def group(title):
    print(f"::group::{title}", flush=True)
    try:
        yield
    finally:
        print("::endgroup::", flush=True)


def roots(environment, workspace, home):
    """Keep Installation Identity Independent Of The Runner's Per-Job Temp Directory."""
    data = Path(environment.get("MISE_DATA_DIR") or home / ".local/share/mise")
    cargo = Path(environment.get("CARGO_HOME") or home / ".cargo")
    rustup = Path(environment.get("RUSTUP_HOME") or home / ".rustup")
    return {
        "MISE_DATA_DIR": str(data),
        "CARGO_HOME": str(cargo),
        "RUSTUP_HOME": str(rustup),
        "MISE_RUSTUP_HOME": str(rustup),
        "MISE_CARGO_HOME": str(cargo),
        "MISE_ROOT_ID": hashlib.sha256(str(data.resolve()).encode()).hexdigest(),
        "WORKSPACE_ROOT_ID": hashlib.sha256(str(workspace.resolve()).encode()).hexdigest(),
        "RUNNER_IMAGE_ID": environment.get("ImageOS", os.name) + "-" + environment.get("ImageVersion", "self"),
    }


def export(values):
    EXPORTED.update(values)
    os.environ.update(values)
    if destination := os.environ.get("GITHUB_ENV"):
        with Path(destination).open("a", encoding="utf-8") as stream:
            for name, value in values.items():
                if "\n" in value or "\r" in value:
                    raise ValueError(f"invalid environment value: {name}")
                stream.write(f"{name}={value}\n")


def download(url, checksum, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    if not destination.is_file() or hashlib.sha256(destination.read_bytes()).hexdigest() != checksum:
        with urllib.request.urlopen(url, timeout=120) as response:
            destination.write_bytes(response.read())
    if hashlib.sha256(destination.read_bytes()).hexdigest() != checksum:
        raise ValueError("pinned bootstrap archive checksum mismatch")


def windows(workspace):
    vswhere = Path(os.environ["ProgramFiles(x86)"]) / "Microsoft Visual Studio/Installer/vswhere.exe"
    installation = subprocess.check_output(
        [
            str(vswhere),
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ],
        text=True,
        creationflags=spawn_flags(),
    ).strip()
    if not installation:
        raise RuntimeError("the Windows image requires Visual Studio C++ tools")
    vcvars = Path(installation) / "VC/Auxiliary/Build/vcvars64.bat"
    # Preserve Cmd Quoting Through CreateProcess.
    output = subprocess.check_output(f'cmd.exe /d /s /c ""{vcvars}" >nul && set"', text=True, creationflags=spawn_flags())
    values = dict(line.split("=", 1) for line in output.splitlines() if "=" in line and not line.startswith("="))
    export({name.upper(): value for name, value in values.items() if name.upper() in {"PATH", "INCLUDE", "LIB", "LIBPATH"}})

    archive = workspace / ".cache/bootstrap/zstd-1.5.7/zstd.zip"
    download(ZSTD_URL, ZSTD_SHA256, archive)
    with zipfile.ZipFile(archive) as bundle:
        bundle.extractall(archive.parent)
    compressor = archive.parent / "zstd-v1.5.7-win64"
    candidates = [Path(item.strip('"')) / "tar.exe" for item in os.environ["PATH"].split(os.pathsep)]
    git = shutil.which("git")
    if git:
        candidates += [Path(git).parent.parent / "usr/bin/tar.exe"]
    tar = next(
        (
            path
            for path in candidates
            if path.is_file()
            and "GNU tar"
            in subprocess.run([str(path), "--version"], capture_output=True, text=True, check=False, creationflags=spawn_flags()).stdout
        ),
        None,
    )
    if tar is None:
        raise RuntimeError("GNU tar is required from the standard Git image")
    compiler = shutil.which("cl")
    if compiler is None:
        raise RuntimeError("MSVC compiler is required")
    export({"PATH": os.pathsep.join([str(Path(compiler).parent), str(compressor), str(tar.parent), os.environ["PATH"]])})
    if not shutil.which("cl") or not shutil.which("link"):
        raise RuntimeError("MSVC compiler and linker are required")

    if shutil.which("mise") is None:
        archive = workspace / ".cache/bootstrap/mise-2026.10.17/mise.zip"
        download(MISE_URL, MISE_SHA256, archive)
        with zipfile.ZipFile(archive) as bundle:
            bundle.extractall(archive.parent)
        binary = archive.parent / "mise/bin"
        export({"PATH": os.pathsep.join([str(binary), os.environ["PATH"]])})
        subprocess.run(["mise", "--version"], check=True, stdout=sys.stderr, creationflags=spawn_flags())

    if os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted" and ctypes.windll.shell32.IsUserAnAdmin():
        exclusions = [
            str(workspace),
            str(Path(os.environ.get("CARGO_TARGET_DIR", str(workspace / "target"))).resolve()),
            os.environ["CARGO_HOME"],
            os.environ["MISE_DATA_DIR"],
        ]
        script = (
            "Add-MpPreference -ExclusionPath @("
            + ",".join("'" + path.replace("'", "''") + "'" for path in exclusions)
            + ") -ErrorAction Stop"
        )
        # Hosted Image Provisioning Is The Only Place That Changes Defender Policy.
        subprocess.run(
            ["powershell", "-NoProfile", "-NonInteractive", "-Command", script], check=True, stdout=sys.stderr, creationflags=spawn_flags()
        )


def unix():
    tools = os.environ.get("MISE_ENABLE_TOOLS", "rust,node").split(",")
    compiler = "rust" in tools or any(tool.startswith("cargo:") for tool in tools)
    required = ["tar", "zstd", "unzip", "zip"] + (["cc", "c++", "make", "pkg-config"] if compiler else [])
    missing = any(shutil.which(tool) is None for tool in required)
    if compiler and shutil.which("pkg-config"):
        missing |= subprocess.run(["pkg-config", "--exists", "openssl"], check=False, creationflags=spawn_flags()).returncode != 0
    atomic = "node" in tools or any(tool.startswith("npm:") for tool in tools)
    if atomic:
        missing |= not atomic_available()
    if missing:
        elevate = [] if os.geteuid() == 0 else ["sudo", "-n"]
        if shutil.which("apt-get"):
            packages = (
                ["tar", "zstd", "unzip", "zip"]
                + (["build-essential", "pkg-config", "libssl-dev"] if compiler else [])
                + (["libatomic1"] if atomic else [])
            )
            subprocess.run([*elevate, "apt-get", "update"], check=True, creationflags=spawn_flags())
            subprocess.run(
                [*elevate, "apt-get", "install", "-y", "--no-install-recommends", *packages], check=True, creationflags=spawn_flags()
            )
        elif shutil.which("apk"):
            packages = (
                ["tar", "zstd", "unzip", "zip"]
                + (["build-base", "pkgconf", "openssl-dev"] if compiler else [])
                + (["libatomic"] if atomic else [])
            )
            subprocess.run([*elevate, "apk", "add", "--no-cache", *packages], check=True, creationflags=spawn_flags())
        else:
            raise RuntimeError("provide native CI prerequisites through the declared image or bootstrap")
    if "GNU tar" not in subprocess.check_output(["tar", "--version"], text=True, creationflags=spawn_flags()):
        raise RuntimeError("GNU tar is required")


def atomic_available():
    if shutil.which("ldconfig"):
        result = subprocess.run(["ldconfig", "-p"], check=False, capture_output=True, text=True, creationflags=spawn_flags())
        if "libatomic.so.1" in result.stdout:
            return True
    directories = [Path(path) for path in ("/lib", "/lib64", "/usr/lib", "/usr/lib64")]
    return any((directory / "libatomic.so.1").is_file() or any(directory.glob("*/libatomic.so.1")) for directory in directories)


def rustup():
    # Mise Reuses The Existing Rustup Installation Instead Of Bootstrapping A Competing One.
    proxies = Path(os.environ["CARGO_HOME"]) / "bin"
    if not (proxies / ("rustup.exe" if os.name == "nt" else "rustup")).is_file() and shutil.which("rustup"):
        existing = Path(shutil.which("rustup")).resolve()
        proxies.mkdir(parents=True, exist_ok=True)
        for name in ("rustup", "cargo", "rustc", "rustdoc", "rustfmt", "cargo-clippy", "clippy-driver"):
            filename = name + (".exe" if os.name == "nt" else "")
            destination = proxies / filename
            if not destination.exists():
                shutil.copy2(existing, destination)
    rustup = proxies / ("rustup.exe" if os.name == "nt" else "rustup")
    if rustup.is_file() and not (Path(os.environ["RUSTUP_HOME"]) / "settings.toml").is_file():
        subprocess.run([str(rustup), "set", "profile", "default"], check=True, stdout=sys.stderr, creationflags=spawn_flags())
    export({"PATH": os.pathsep.join([str(proxies), os.environ["PATH"]])})


def glab(version):
    if re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version) is None:
        raise ValueError("GitLab CLI bootstrap requires an exact version")
    # This Explicit CI Tool Is Outside The Project Lock; Project Installs Remain Locked.
    subprocess.run(
        ["mise", "install", "--yes", f"glab@{version}"],
        env=os.environ | {"MISE_LOCKED": "0", "MISE_ENABLE_TOOLS": "glab"},
        stdout=sys.stderr,
        creationflags=spawn_flags(),
        check=True,
    )
    export({"MISE_GLAB_VERSION": version})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--platform", choices=("unix", "windows"), default=os.environ.get("SETUP_PLATFORM", "windows" if os.name == "nt" else "unix")
    )
    parser.add_argument("--roots-only", action="store_true")
    parser.add_argument("--emit-shell", action="store_true")
    parser.add_argument("--glab-version")
    options = parser.parse_args()
    if options.glab_version:
        glab(options.glab_version)
        return
    workspace = Path.cwd()
    with group("Toolchain Roots"):
        export(roots(os.environ, workspace, Path.home()))
        export({name: os.environ[name] for name in ("COMPILER_CACHE_BACKEND", "COMPILER_CACHE_MODE") if os.environ.get(name)})
        rustup()
        if destination := os.environ.get("GITHUB_OUTPUT"):
            with Path(destination).open("a", encoding="utf-8") as stream:
                for name, cache in json.loads(os.environ.get("CACHE_PLAN", "{}")).items():
                    for field in ("path", "key", "restore-keys"):
                        value = cache.get(field, "")
                        for variable in ("MISE_DATA_DIR", "MISE_ROOT_ID", "WORKSPACE_ROOT_ID", "RUNNER_IMAGE_ID"):
                            value = value.replace("<" + variable + ">", os.environ[variable])
                        if field == "path" and not value.strip():
                            raise ValueError(f"cache path is empty: {name}")
                        stream.write(f"{name}-{field}<<CACHE_VALUE\n{value}\nCACHE_VALUE\n")
    if options.roots_only:
        return
    with group("Native Prerequisites"):
        windows(workspace) if options.platform == "windows" else unix()
    subprocess.run(["zstd", "--version"], check=True, stdout=sys.stderr, creationflags=spawn_flags())
    with group("Project Bootstrap"):
        shell = ["pwsh", "-NoProfile", "-NonInteractive", "-Command"] if os.name == "nt" else ["bash", "-eo", "pipefail", "-c"]
        for command in json.loads(os.environ.get("SETUP_BOOTSTRAP", "[]")):
            subprocess.run([*shell, command], check=True, stdout=sys.stderr, creationflags=spawn_flags())


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    if "--emit-shell" in sys.argv:
        with contextlib.redirect_stdout(sys.stderr):
            main()
        for name, value in EXPORTED.items():
            if os.name == "nt":
                escaped = value.replace("'", "''")
                print(f"$env:{name}='{escaped}'")
            else:
                print(f"export {name}={shlex.quote(value)}")
    else:
        main()
