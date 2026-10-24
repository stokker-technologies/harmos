# Rendered by stkr project update from templates/python/ci/simulate.py; sha256:1df96bb5bbb475c45b5dc2fea9240adef8070f47b9ad297fda8e3563cf450e3c
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Execute The Rendered CI Graph In Isolated Native And Docker Jobs."""

# === Imports === #

# Standard Library Imports
import argparse
import concurrent.futures
import ctypes
import glob
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
import uuid
from pathlib import Path

# Third Party Imports
# Local Imports
from . import environment as step_environment
from . import impact
from . import summary as reporting
from .resources import Budget, NativeLimit, SimulationError, managed_image

# Type Checking Imports


# === Public Exports === #

# === Constants === #

PLAN = json.loads(
    '{"platforms":["linux","windows"],"jobs":[{"key":"source","id":"check:repository","platform":"linux","independent":true,"pipelines":["pr","dev","release","nightly"],"needs":[],"inputs":[],"outputs":{},"commands":[{"run":"mise run check:repository","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:taiki-e/cargo-llvm-cov,github:bnjbvr/cargo-machete,sccache,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,npm:@fission-ai/openspec,taplo,git-cliff","skip":null,"image":"managed:ubuntu-24.04","minutes":120},{"key":"check_python","id":"check:python","platform":"linux","independent":true,"pipelines":["pr","dev","release","nightly"],"needs":[],"inputs":[],"outputs":{},"commands":[{"run":"mise run check:python","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":null,"image":"managed:ubuntu-24.04","minutes":120},{"key":"check_rust_windows","id":"check:rust","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":[],"inputs":[],"outputs":{},"commands":[{"run":"mise run check:rust","cached":false,"shell":null}],"bootstrap":[],"setup":"$ErrorActionPreference = \'Stop\'\\n# === Setup === #\\n$vswhere = Join-Path ${env:ProgramFiles(x86)} \'Microsoft Visual Studio/Installer/vswhere.exe\'\\nif (!(Test-Path -LiteralPath $vswhere -PathType Leaf)) { throw \'The standard Windows image requires installed Visual Studio Build Tools\' }\\n$installation = & $vswhere -latest -products \'*\' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath\\nif ($LASTEXITCODE -ne 0 -or !$installation) { throw \'The standard Windows image requires installed Visual Studio C++ tools\' }\\nImport-Module (Join-Path $installation \'Common7/Tools/Microsoft.VisualStudio.DevShell.dll\')\\nEnter-VsDevShell -VsInstallPath $installation -SkipAutomaticLocation -DevCmdArguments \'-arch=x64 -host_arch=x64\' | Out-Null\\n$compiler = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'cl.exe\' } | Where-Object { (Test-Path -LiteralPath $_ -PathType Leaf) -and $_.StartsWith($installation, [StringComparison]::OrdinalIgnoreCase) }) | Select-Object -First 1\\nif (!$compiler -or !(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $compiler) \'link.exe\') -PathType Leaf)) { throw \'MSVC compiler and linker are required from the selected installation\' }\\n$archiveRoot = Join-Path $PWD \'.cache/bootstrap/zstd-1.5.7\'\\n$archive = Join-Path $archiveRoot \'zstd.zip\'\\nNew-Item -ItemType Directory -Force $archiveRoot | Out-Null\\n$checksum = \'acb4e8111511749dc7a3ebedca9b04190e37a17afeb73f55d4425dbf0b90fad9\'\\nif (!(Test-Path -LiteralPath $archive) -or (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) {\\n    Invoke-WebRequest \'https://github.com/facebook/zstd/releases/download/v1.5.7/zstd-v1.5.7-win64.zip\' -OutFile $archive\\n}\\nif ((Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) { throw \'Pinned zstd archive checksum mismatch\' }\\nExpand-Archive -LiteralPath $archive -DestinationPath $archiveRoot -Force\\n$compressor = Join-Path $archiveRoot \'zstd-v1.5.7-win64/zstd.exe\'\\nif (!(Test-Path -LiteralPath $compressor -PathType Leaf)) { throw \'Pinned zstd executable missing\' }\\n$compressorRoot = Split-Path -Parent $compressor\\n$env:PATH = (Split-Path -Parent $compiler) + [IO.Path]::PathSeparator + $compressorRoot + [IO.Path]::PathSeparator + $env:PATH\\n# === Verification === #\\n& $compressor --version\\nif ($LASTEXITCODE -ne 0) { throw \'Pinned zstd cannot execute\' }\\n$selectedLink = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'link.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }) | Select-Object -First 1\\nif ($selectedLink -ne (Join-Path (Split-Path -Parent $compiler) \'link.exe\')) { throw \'MSVC linker precedence is required\' }\\n# === Entry Point === #\\n$paths = @($env:PATH -split [IO.Path]::PathSeparator | ForEach-Object { $_.Trim(\'\\"\') } | Where-Object { [IO.Path]::GetPathRoot($_).Length -ge 3 })\\n$candidates = @($paths | ForEach-Object { Join-Path $_ \'tar.exe\' })\\nforeach ($git in @($paths | ForEach-Object { Join-Path $_ \'git.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })) {\\n  $root = Split-Path -Parent (Split-Path -Parent $git)\\n  $candidates += Join-Path $root \'usr/bin/tar.exe\'\\n}\\n$tar = $null\\nforeach ($candidate in @($candidates | Select-Object -Unique)) {\\n  if ([IO.Path]::GetPathRoot($candidate).Length -lt 3 -or !(Test-Path -LiteralPath $candidate -PathType Leaf)) { continue }\\n  try {\\n    $version = & $candidate --version 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$version\\" -notmatch \'^tar \\\\(GNU tar\\\\)\') { continue }\\n    $capabilities = & $candidate --help 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$capabilities\\" -notmatch \'--force-local\' -or \\"$capabilities\\" -notmatch \'--use-compress-program\') { continue }\\n    $tar = $candidate\\n    break\\n  } catch { continue }\\n}\\nif (!$tar) { throw \'GNU tar with force-local and external-compressor support is required from the standard Git image baseline\' }\\n$env:PATH = (@((Split-Path -Parent $compiler), (Split-Path -Parent $tar), $compressorRoot) + @($env:PATH -split [IO.Path]::PathSeparator) | Select-Object -Unique) -join [IO.Path]::PathSeparator\\nif ($env:GITHUB_ENV) {\\n  foreach ($name in @(\'PATH\', \'INCLUDE\', \'LIB\', \'LIBPATH\')) {\\n    $value = [Environment]::GetEnvironmentVariable($name)\\n    if ($null -ne $value) { \\"$name=$value\\" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append }\\n  }\\n}","tools":"go,node,python,rust,uv,github:bnjbvr/cargo-machete,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,npm:@fission-ai/openspec,taplo","skip":null,"image":"managed:ubuntu-24.04","minutes":120},{"key":"build_product_windows","id":"build","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":[],"inputs":[],"outputs":{"product":["target/build/{profile}/{target}/bin","target/build/{profile}/{target}/lib","target/build/{profile}/{target}/product","target/generated/python/pyproject.toml","target/generated/python/src"]},"commands":[{"run":"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.build --profile ${{ env.PROFILE }} --output product --task build","cached":false,"shell":null}],"bootstrap":[],"setup":"$ErrorActionPreference = \'Stop\'\\n# === Setup === #\\n$vswhere = Join-Path ${env:ProgramFiles(x86)} \'Microsoft Visual Studio/Installer/vswhere.exe\'\\nif (!(Test-Path -LiteralPath $vswhere -PathType Leaf)) { throw \'The standard Windows image requires installed Visual Studio Build Tools\' }\\n$installation = & $vswhere -latest -products \'*\' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath\\nif ($LASTEXITCODE -ne 0 -or !$installation) { throw \'The standard Windows image requires installed Visual Studio C++ tools\' }\\nImport-Module (Join-Path $installation \'Common7/Tools/Microsoft.VisualStudio.DevShell.dll\')\\nEnter-VsDevShell -VsInstallPath $installation -SkipAutomaticLocation -DevCmdArguments \'-arch=x64 -host_arch=x64\' | Out-Null\\n$compiler = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'cl.exe\' } | Where-Object { (Test-Path -LiteralPath $_ -PathType Leaf) -and $_.StartsWith($installation, [StringComparison]::OrdinalIgnoreCase) }) | Select-Object -First 1\\nif (!$compiler -or !(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $compiler) \'link.exe\') -PathType Leaf)) { throw \'MSVC compiler and linker are required from the selected installation\' }\\n$archiveRoot = Join-Path $PWD \'.cache/bootstrap/zstd-1.5.7\'\\n$archive = Join-Path $archiveRoot \'zstd.zip\'\\nNew-Item -ItemType Directory -Force $archiveRoot | Out-Null\\n$checksum = \'acb4e8111511749dc7a3ebedca9b04190e37a17afeb73f55d4425dbf0b90fad9\'\\nif (!(Test-Path -LiteralPath $archive) -or (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) {\\n    Invoke-WebRequest \'https://github.com/facebook/zstd/releases/download/v1.5.7/zstd-v1.5.7-win64.zip\' -OutFile $archive\\n}\\nif ((Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) { throw \'Pinned zstd archive checksum mismatch\' }\\nExpand-Archive -LiteralPath $archive -DestinationPath $archiveRoot -Force\\n$compressor = Join-Path $archiveRoot \'zstd-v1.5.7-win64/zstd.exe\'\\nif (!(Test-Path -LiteralPath $compressor -PathType Leaf)) { throw \'Pinned zstd executable missing\' }\\n$compressorRoot = Split-Path -Parent $compressor\\n$env:PATH = (Split-Path -Parent $compiler) + [IO.Path]::PathSeparator + $compressorRoot + [IO.Path]::PathSeparator + $env:PATH\\n# === Verification === #\\n& $compressor --version\\nif ($LASTEXITCODE -ne 0) { throw \'Pinned zstd cannot execute\' }\\n$selectedLink = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'link.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }) | Select-Object -First 1\\nif ($selectedLink -ne (Join-Path (Split-Path -Parent $compiler) \'link.exe\')) { throw \'MSVC linker precedence is required\' }\\n# === Entry Point === #\\n$paths = @($env:PATH -split [IO.Path]::PathSeparator | ForEach-Object { $_.Trim(\'\\"\') } | Where-Object { [IO.Path]::GetPathRoot($_).Length -ge 3 })\\n$candidates = @($paths | ForEach-Object { Join-Path $_ \'tar.exe\' })\\nforeach ($git in @($paths | ForEach-Object { Join-Path $_ \'git.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })) {\\n  $root = Split-Path -Parent (Split-Path -Parent $git)\\n  $candidates += Join-Path $root \'usr/bin/tar.exe\'\\n}\\n$tar = $null\\nforeach ($candidate in @($candidates | Select-Object -Unique)) {\\n  if ([IO.Path]::GetPathRoot($candidate).Length -lt 3 -or !(Test-Path -LiteralPath $candidate -PathType Leaf)) { continue }\\n  try {\\n    $version = & $candidate --version 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$version\\" -notmatch \'^tar \\\\(GNU tar\\\\)\') { continue }\\n    $capabilities = & $candidate --help 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$capabilities\\" -notmatch \'--force-local\' -or \\"$capabilities\\" -notmatch \'--use-compress-program\') { continue }\\n    $tar = $candidate\\n    break\\n  } catch { continue }\\n}\\nif (!$tar) { throw \'GNU tar with force-local and external-compressor support is required from the standard Git image baseline\' }\\n$env:PATH = (@((Split-Path -Parent $compiler), (Split-Path -Parent $tar), $compressorRoot) + @($env:PATH -split [IO.Path]::PathSeparator) | Select-Object -Unique) -join [IO.Path]::PathSeparator\\nif ($env:GITHUB_ENV) {\\n  foreach ($name in @(\'PATH\', \'INCLUDE\', \'LIB\', \'LIBPATH\')) {\\n    $value = [Environment]::GetEnvironmentVariable($name)\\n    if ($null -ne $value) { \\"$name=$value\\" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append }\\n  }\\n}","tools":"aqua:koalaman/shellcheck,aqua:nextest-rs/nextest/cargo-nextest,github:bnjbvr/cargo-machete,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,npm:@fission-ai/openspec,python,rust,sccache,taplo,uv","skip":null,"image":"managed:ubuntu-24.04","minutes":120},{"key":"build_tests_windows","id":"build:rust:tests","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":[],"inputs":[],"outputs":{"fixtures":["target/build/{profile}/{target}/fixtures"],"tests":["target/build/{profile}/{target}/tests/archive.tar.zst","target/build/{profile}/{target}/tests/compiler.json"]},"commands":[{"run":"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.build --profile ${{ env.PROFILE }} --output tests --task build:rust:tests","cached":false,"shell":null}],"bootstrap":[],"setup":"$ErrorActionPreference = \'Stop\'\\n# === Setup === #\\n$vswhere = Join-Path ${env:ProgramFiles(x86)} \'Microsoft Visual Studio/Installer/vswhere.exe\'\\nif (!(Test-Path -LiteralPath $vswhere -PathType Leaf)) { throw \'The standard Windows image requires installed Visual Studio Build Tools\' }\\n$installation = & $vswhere -latest -products \'*\' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath\\nif ($LASTEXITCODE -ne 0 -or !$installation) { throw \'The standard Windows image requires installed Visual Studio C++ tools\' }\\nImport-Module (Join-Path $installation \'Common7/Tools/Microsoft.VisualStudio.DevShell.dll\')\\nEnter-VsDevShell -VsInstallPath $installation -SkipAutomaticLocation -DevCmdArguments \'-arch=x64 -host_arch=x64\' | Out-Null\\n$compiler = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'cl.exe\' } | Where-Object { (Test-Path -LiteralPath $_ -PathType Leaf) -and $_.StartsWith($installation, [StringComparison]::OrdinalIgnoreCase) }) | Select-Object -First 1\\nif (!$compiler -or !(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $compiler) \'link.exe\') -PathType Leaf)) { throw \'MSVC compiler and linker are required from the selected installation\' }\\n$archiveRoot = Join-Path $PWD \'.cache/bootstrap/zstd-1.5.7\'\\n$archive = Join-Path $archiveRoot \'zstd.zip\'\\nNew-Item -ItemType Directory -Force $archiveRoot | Out-Null\\n$checksum = \'acb4e8111511749dc7a3ebedca9b04190e37a17afeb73f55d4425dbf0b90fad9\'\\nif (!(Test-Path -LiteralPath $archive) -or (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) {\\n    Invoke-WebRequest \'https://github.com/facebook/zstd/releases/download/v1.5.7/zstd-v1.5.7-win64.zip\' -OutFile $archive\\n}\\nif ((Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) { throw \'Pinned zstd archive checksum mismatch\' }\\nExpand-Archive -LiteralPath $archive -DestinationPath $archiveRoot -Force\\n$compressor = Join-Path $archiveRoot \'zstd-v1.5.7-win64/zstd.exe\'\\nif (!(Test-Path -LiteralPath $compressor -PathType Leaf)) { throw \'Pinned zstd executable missing\' }\\n$compressorRoot = Split-Path -Parent $compressor\\n$env:PATH = (Split-Path -Parent $compiler) + [IO.Path]::PathSeparator + $compressorRoot + [IO.Path]::PathSeparator + $env:PATH\\n# === Verification === #\\n& $compressor --version\\nif ($LASTEXITCODE -ne 0) { throw \'Pinned zstd cannot execute\' }\\n$selectedLink = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'link.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }) | Select-Object -First 1\\nif ($selectedLink -ne (Join-Path (Split-Path -Parent $compiler) \'link.exe\')) { throw \'MSVC linker precedence is required\' }\\n# === Entry Point === #\\n$paths = @($env:PATH -split [IO.Path]::PathSeparator | ForEach-Object { $_.Trim(\'\\"\') } | Where-Object { [IO.Path]::GetPathRoot($_).Length -ge 3 })\\n$candidates = @($paths | ForEach-Object { Join-Path $_ \'tar.exe\' })\\nforeach ($git in @($paths | ForEach-Object { Join-Path $_ \'git.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })) {\\n  $root = Split-Path -Parent (Split-Path -Parent $git)\\n  $candidates += Join-Path $root \'usr/bin/tar.exe\'\\n}\\n$tar = $null\\nforeach ($candidate in @($candidates | Select-Object -Unique)) {\\n  if ([IO.Path]::GetPathRoot($candidate).Length -lt 3 -or !(Test-Path -LiteralPath $candidate -PathType Leaf)) { continue }\\n  try {\\n    $version = & $candidate --version 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$version\\" -notmatch \'^tar \\\\(GNU tar\\\\)\') { continue }\\n    $capabilities = & $candidate --help 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$capabilities\\" -notmatch \'--force-local\' -or \\"$capabilities\\" -notmatch \'--use-compress-program\') { continue }\\n    $tar = $candidate\\n    break\\n  } catch { continue }\\n}\\nif (!$tar) { throw \'GNU tar with force-local and external-compressor support is required from the standard Git image baseline\' }\\n$env:PATH = (@((Split-Path -Parent $compiler), (Split-Path -Parent $tar), $compressorRoot) + @($env:PATH -split [IO.Path]::PathSeparator) | Select-Object -Unique) -join [IO.Path]::PathSeparator\\nif ($env:GITHUB_ENV) {\\n  foreach ($name in @(\'PATH\', \'INCLUDE\', \'LIB\', \'LIBPATH\')) {\\n    $value = [Environment]::GetEnvironmentVariable($name)\\n    if ($null -ne $value) { \\"$name=$value\\" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append }\\n  }\\n}","tools":"aqua:koalaman/shellcheck,aqua:nextest-rs/nextest/cargo-nextest,github:bnjbvr/cargo-machete,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,npm:@fission-ai/openspec,python,rust,sccache,taplo,uv","skip":null,"image":"managed:ubuntu-24.04","minutes":120},{"key":"check_generated_python","id":"check:python:generated","platform":"linux","independent":true,"pipelines":["pr","dev","release","nightly"],"needs":["build_product_linux"],"inputs":["build#product"],"outputs":{},"commands":[{"run":"mise exec -- uv run --locked --project tools/python python -m harmos_tools.test.archive --profile ${{ env.PROFILE }} --install-wheels","cached":false,"shell":null},{"run":"mise run check:python:generated","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build","image":"managed:ubuntu-24.04","minutes":120},{"key":"prepare_test_fixtures_windows","id":"prepare:tests:fixtures","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":["build_product_windows","build_tests_windows"],"inputs":["build#product","build:rust:tests#fixtures"],"outputs":{"fixtures":["target/build/{profile}/{target}/fixtures"]},"commands":[{"run":"mise run prepare:tests:fixtures --profile ${{ env.PROFILE }}","cached":true,"shell":null}],"bootstrap":[],"setup":"$ErrorActionPreference = \'Stop\'\\n# === Setup === #\\n$vswhere = Join-Path ${env:ProgramFiles(x86)} \'Microsoft Visual Studio/Installer/vswhere.exe\'\\nif (!(Test-Path -LiteralPath $vswhere -PathType Leaf)) { throw \'The standard Windows image requires installed Visual Studio Build Tools\' }\\n$installation = & $vswhere -latest -products \'*\' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath\\nif ($LASTEXITCODE -ne 0 -or !$installation) { throw \'The standard Windows image requires installed Visual Studio C++ tools\' }\\nImport-Module (Join-Path $installation \'Common7/Tools/Microsoft.VisualStudio.DevShell.dll\')\\nEnter-VsDevShell -VsInstallPath $installation -SkipAutomaticLocation -DevCmdArguments \'-arch=x64 -host_arch=x64\' | Out-Null\\n$compiler = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'cl.exe\' } | Where-Object { (Test-Path -LiteralPath $_ -PathType Leaf) -and $_.StartsWith($installation, [StringComparison]::OrdinalIgnoreCase) }) | Select-Object -First 1\\nif (!$compiler -or !(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $compiler) \'link.exe\') -PathType Leaf)) { throw \'MSVC compiler and linker are required from the selected installation\' }\\n$archiveRoot = Join-Path $PWD \'.cache/bootstrap/zstd-1.5.7\'\\n$archive = Join-Path $archiveRoot \'zstd.zip\'\\nNew-Item -ItemType Directory -Force $archiveRoot | Out-Null\\n$checksum = \'acb4e8111511749dc7a3ebedca9b04190e37a17afeb73f55d4425dbf0b90fad9\'\\nif (!(Test-Path -LiteralPath $archive) -or (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) {\\n    Invoke-WebRequest \'https://github.com/facebook/zstd/releases/download/v1.5.7/zstd-v1.5.7-win64.zip\' -OutFile $archive\\n}\\nif ((Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) { throw \'Pinned zstd archive checksum mismatch\' }\\nExpand-Archive -LiteralPath $archive -DestinationPath $archiveRoot -Force\\n$compressor = Join-Path $archiveRoot \'zstd-v1.5.7-win64/zstd.exe\'\\nif (!(Test-Path -LiteralPath $compressor -PathType Leaf)) { throw \'Pinned zstd executable missing\' }\\n$compressorRoot = Split-Path -Parent $compressor\\n$env:PATH = (Split-Path -Parent $compiler) + [IO.Path]::PathSeparator + $compressorRoot + [IO.Path]::PathSeparator + $env:PATH\\n# === Verification === #\\n& $compressor --version\\nif ($LASTEXITCODE -ne 0) { throw \'Pinned zstd cannot execute\' }\\n$selectedLink = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'link.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }) | Select-Object -First 1\\nif ($selectedLink -ne (Join-Path (Split-Path -Parent $compiler) \'link.exe\')) { throw \'MSVC linker precedence is required\' }\\n# === Entry Point === #\\n$paths = @($env:PATH -split [IO.Path]::PathSeparator | ForEach-Object { $_.Trim(\'\\"\') } | Where-Object { [IO.Path]::GetPathRoot($_).Length -ge 3 })\\n$candidates = @($paths | ForEach-Object { Join-Path $_ \'tar.exe\' })\\nforeach ($git in @($paths | ForEach-Object { Join-Path $_ \'git.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })) {\\n  $root = Split-Path -Parent (Split-Path -Parent $git)\\n  $candidates += Join-Path $root \'usr/bin/tar.exe\'\\n}\\n$tar = $null\\nforeach ($candidate in @($candidates | Select-Object -Unique)) {\\n  if ([IO.Path]::GetPathRoot($candidate).Length -lt 3 -or !(Test-Path -LiteralPath $candidate -PathType Leaf)) { continue }\\n  try {\\n    $version = & $candidate --version 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$version\\" -notmatch \'^tar \\\\(GNU tar\\\\)\') { continue }\\n    $capabilities = & $candidate --help 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$capabilities\\" -notmatch \'--force-local\' -or \\"$capabilities\\" -notmatch \'--use-compress-program\') { continue }\\n    $tar = $candidate\\n    break\\n  } catch { continue }\\n}\\nif (!$tar) { throw \'GNU tar with force-local and external-compressor support is required from the standard Git image baseline\' }\\n$env:PATH = (@((Split-Path -Parent $compiler), (Split-Path -Parent $tar), $compressorRoot) + @($env:PATH -split [IO.Path]::PathSeparator) | Select-Object -Unique) -join [IO.Path]::PathSeparator\\nif ($env:GITHUB_ENV) {\\n  foreach ($name in @(\'PATH\', \'INCLUDE\', \'LIB\', \'LIBPATH\')) {\\n    $value = [Environment]::GetEnvironmentVariable($name)\\n    if ($null -ne $value) { \\"$name=$value\\" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append }\\n  }\\n}","tools":"aqua:koalaman/shellcheck,aqua:nextest-rs/nextest/cargo-nextest,git-cliff,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,python,rust,sccache,uv","skip":"build,build:rust:tests","image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_unit_windows","id":"test:rust:unit","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":["build_tests_windows"],"inputs":["build:rust:tests#tests"],"outputs":{},"commands":[{"run":"mise run test:rust:unit --profile ${{ env.PROFILE }}","cached":false,"shell":null}],"bootstrap":[],"setup":"$ErrorActionPreference = \'Stop\'\\n# === Setup === #\\n$vswhere = Join-Path ${env:ProgramFiles(x86)} \'Microsoft Visual Studio/Installer/vswhere.exe\'\\nif (!(Test-Path -LiteralPath $vswhere -PathType Leaf)) { throw \'The standard Windows image requires installed Visual Studio Build Tools\' }\\n$installation = & $vswhere -latest -products \'*\' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath\\nif ($LASTEXITCODE -ne 0 -or !$installation) { throw \'The standard Windows image requires installed Visual Studio C++ tools\' }\\nImport-Module (Join-Path $installation \'Common7/Tools/Microsoft.VisualStudio.DevShell.dll\')\\nEnter-VsDevShell -VsInstallPath $installation -SkipAutomaticLocation -DevCmdArguments \'-arch=x64 -host_arch=x64\' | Out-Null\\n$compiler = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'cl.exe\' } | Where-Object { (Test-Path -LiteralPath $_ -PathType Leaf) -and $_.StartsWith($installation, [StringComparison]::OrdinalIgnoreCase) }) | Select-Object -First 1\\nif (!$compiler -or !(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $compiler) \'link.exe\') -PathType Leaf)) { throw \'MSVC compiler and linker are required from the selected installation\' }\\n$archiveRoot = Join-Path $PWD \'.cache/bootstrap/zstd-1.5.7\'\\n$archive = Join-Path $archiveRoot \'zstd.zip\'\\nNew-Item -ItemType Directory -Force $archiveRoot | Out-Null\\n$checksum = \'acb4e8111511749dc7a3ebedca9b04190e37a17afeb73f55d4425dbf0b90fad9\'\\nif (!(Test-Path -LiteralPath $archive) -or (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) {\\n    Invoke-WebRequest \'https://github.com/facebook/zstd/releases/download/v1.5.7/zstd-v1.5.7-win64.zip\' -OutFile $archive\\n}\\nif ((Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) { throw \'Pinned zstd archive checksum mismatch\' }\\nExpand-Archive -LiteralPath $archive -DestinationPath $archiveRoot -Force\\n$compressor = Join-Path $archiveRoot \'zstd-v1.5.7-win64/zstd.exe\'\\nif (!(Test-Path -LiteralPath $compressor -PathType Leaf)) { throw \'Pinned zstd executable missing\' }\\n$compressorRoot = Split-Path -Parent $compressor\\n$env:PATH = (Split-Path -Parent $compiler) + [IO.Path]::PathSeparator + $compressorRoot + [IO.Path]::PathSeparator + $env:PATH\\n# === Verification === #\\n& $compressor --version\\nif ($LASTEXITCODE -ne 0) { throw \'Pinned zstd cannot execute\' }\\n$selectedLink = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'link.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }) | Select-Object -First 1\\nif ($selectedLink -ne (Join-Path (Split-Path -Parent $compiler) \'link.exe\')) { throw \'MSVC linker precedence is required\' }\\n# === Entry Point === #\\n$paths = @($env:PATH -split [IO.Path]::PathSeparator | ForEach-Object { $_.Trim(\'\\"\') } | Where-Object { [IO.Path]::GetPathRoot($_).Length -ge 3 })\\n$candidates = @($paths | ForEach-Object { Join-Path $_ \'tar.exe\' })\\nforeach ($git in @($paths | ForEach-Object { Join-Path $_ \'git.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })) {\\n  $root = Split-Path -Parent (Split-Path -Parent $git)\\n  $candidates += Join-Path $root \'usr/bin/tar.exe\'\\n}\\n$tar = $null\\nforeach ($candidate in @($candidates | Select-Object -Unique)) {\\n  if ([IO.Path]::GetPathRoot($candidate).Length -lt 3 -or !(Test-Path -LiteralPath $candidate -PathType Leaf)) { continue }\\n  try {\\n    $version = & $candidate --version 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$version\\" -notmatch \'^tar \\\\(GNU tar\\\\)\') { continue }\\n    $capabilities = & $candidate --help 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$capabilities\\" -notmatch \'--force-local\' -or \\"$capabilities\\" -notmatch \'--use-compress-program\') { continue }\\n    $tar = $candidate\\n    break\\n  } catch { continue }\\n}\\nif (!$tar) { throw \'GNU tar with force-local and external-compressor support is required from the standard Git image baseline\' }\\n$env:PATH = (@((Split-Path -Parent $compiler), (Split-Path -Parent $tar), $compressorRoot) + @($env:PATH -split [IO.Path]::PathSeparator) | Select-Object -Unique) -join [IO.Path]::PathSeparator\\nif ($env:GITHUB_ENV) {\\n  foreach ($name in @(\'PATH\', \'INCLUDE\', \'LIB\', \'LIBPATH\')) {\\n    $value = [Environment]::GetEnvironmentVariable($name)\\n    if ($null -ne $value) { \\"$name=$value\\" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append }\\n  }\\n}","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build:rust:tests","image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_integration_windows","id":"test:rust:integration","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":["build_tests_windows","prepare_test_fixtures_windows"],"inputs":["build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:integration --profile ${{ env.PROFILE }}","cached":false,"shell":null}],"bootstrap":[],"setup":"$ErrorActionPreference = \'Stop\'\\n# === Setup === #\\n$vswhere = Join-Path ${env:ProgramFiles(x86)} \'Microsoft Visual Studio/Installer/vswhere.exe\'\\nif (!(Test-Path -LiteralPath $vswhere -PathType Leaf)) { throw \'The standard Windows image requires installed Visual Studio Build Tools\' }\\n$installation = & $vswhere -latest -products \'*\' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath\\nif ($LASTEXITCODE -ne 0 -or !$installation) { throw \'The standard Windows image requires installed Visual Studio C++ tools\' }\\nImport-Module (Join-Path $installation \'Common7/Tools/Microsoft.VisualStudio.DevShell.dll\')\\nEnter-VsDevShell -VsInstallPath $installation -SkipAutomaticLocation -DevCmdArguments \'-arch=x64 -host_arch=x64\' | Out-Null\\n$compiler = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'cl.exe\' } | Where-Object { (Test-Path -LiteralPath $_ -PathType Leaf) -and $_.StartsWith($installation, [StringComparison]::OrdinalIgnoreCase) }) | Select-Object -First 1\\nif (!$compiler -or !(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $compiler) \'link.exe\') -PathType Leaf)) { throw \'MSVC compiler and linker are required from the selected installation\' }\\n$archiveRoot = Join-Path $PWD \'.cache/bootstrap/zstd-1.5.7\'\\n$archive = Join-Path $archiveRoot \'zstd.zip\'\\nNew-Item -ItemType Directory -Force $archiveRoot | Out-Null\\n$checksum = \'acb4e8111511749dc7a3ebedca9b04190e37a17afeb73f55d4425dbf0b90fad9\'\\nif (!(Test-Path -LiteralPath $archive) -or (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) {\\n    Invoke-WebRequest \'https://github.com/facebook/zstd/releases/download/v1.5.7/zstd-v1.5.7-win64.zip\' -OutFile $archive\\n}\\nif ((Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) { throw \'Pinned zstd archive checksum mismatch\' }\\nExpand-Archive -LiteralPath $archive -DestinationPath $archiveRoot -Force\\n$compressor = Join-Path $archiveRoot \'zstd-v1.5.7-win64/zstd.exe\'\\nif (!(Test-Path -LiteralPath $compressor -PathType Leaf)) { throw \'Pinned zstd executable missing\' }\\n$compressorRoot = Split-Path -Parent $compressor\\n$env:PATH = (Split-Path -Parent $compiler) + [IO.Path]::PathSeparator + $compressorRoot + [IO.Path]::PathSeparator + $env:PATH\\n# === Verification === #\\n& $compressor --version\\nif ($LASTEXITCODE -ne 0) { throw \'Pinned zstd cannot execute\' }\\n$selectedLink = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'link.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }) | Select-Object -First 1\\nif ($selectedLink -ne (Join-Path (Split-Path -Parent $compiler) \'link.exe\')) { throw \'MSVC linker precedence is required\' }\\n# === Entry Point === #\\n$paths = @($env:PATH -split [IO.Path]::PathSeparator | ForEach-Object { $_.Trim(\'\\"\') } | Where-Object { [IO.Path]::GetPathRoot($_).Length -ge 3 })\\n$candidates = @($paths | ForEach-Object { Join-Path $_ \'tar.exe\' })\\nforeach ($git in @($paths | ForEach-Object { Join-Path $_ \'git.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })) {\\n  $root = Split-Path -Parent (Split-Path -Parent $git)\\n  $candidates += Join-Path $root \'usr/bin/tar.exe\'\\n}\\n$tar = $null\\nforeach ($candidate in @($candidates | Select-Object -Unique)) {\\n  if ([IO.Path]::GetPathRoot($candidate).Length -lt 3 -or !(Test-Path -LiteralPath $candidate -PathType Leaf)) { continue }\\n  try {\\n    $version = & $candidate --version 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$version\\" -notmatch \'^tar \\\\(GNU tar\\\\)\') { continue }\\n    $capabilities = & $candidate --help 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$capabilities\\" -notmatch \'--force-local\' -or \\"$capabilities\\" -notmatch \'--use-compress-program\') { continue }\\n    $tar = $candidate\\n    break\\n  } catch { continue }\\n}\\nif (!$tar) { throw \'GNU tar with force-local and external-compressor support is required from the standard Git image baseline\' }\\n$env:PATH = (@((Split-Path -Parent $compiler), (Split-Path -Parent $tar), $compressorRoot) + @($env:PATH -split [IO.Path]::PathSeparator) | Select-Object -Unique) -join [IO.Path]::PathSeparator\\nif ($env:GITHUB_ENV) {\\n  foreach ($name in @(\'PATH\', \'INCLUDE\', \'LIB\', \'LIBPATH\')) {\\n    $value = [Environment]::GetEnvironmentVariable($name)\\n    if ($null -ne $value) { \\"$name=$value\\" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append }\\n  }\\n}","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build:rust:tests,prepare:tests:fixtures","image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_acceptance_windows","id":"test:rust:acceptance","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":["build_product_windows","build_tests_windows","prepare_test_fixtures_windows"],"inputs":["build#product","build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:acceptance --profile ${{ env.PROFILE }}","cached":false,"shell":null}],"bootstrap":[],"setup":"$ErrorActionPreference = \'Stop\'\\n# === Setup === #\\n$vswhere = Join-Path ${env:ProgramFiles(x86)} \'Microsoft Visual Studio/Installer/vswhere.exe\'\\nif (!(Test-Path -LiteralPath $vswhere -PathType Leaf)) { throw \'The standard Windows image requires installed Visual Studio Build Tools\' }\\n$installation = & $vswhere -latest -products \'*\' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath\\nif ($LASTEXITCODE -ne 0 -or !$installation) { throw \'The standard Windows image requires installed Visual Studio C++ tools\' }\\nImport-Module (Join-Path $installation \'Common7/Tools/Microsoft.VisualStudio.DevShell.dll\')\\nEnter-VsDevShell -VsInstallPath $installation -SkipAutomaticLocation -DevCmdArguments \'-arch=x64 -host_arch=x64\' | Out-Null\\n$compiler = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'cl.exe\' } | Where-Object { (Test-Path -LiteralPath $_ -PathType Leaf) -and $_.StartsWith($installation, [StringComparison]::OrdinalIgnoreCase) }) | Select-Object -First 1\\nif (!$compiler -or !(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $compiler) \'link.exe\') -PathType Leaf)) { throw \'MSVC compiler and linker are required from the selected installation\' }\\n$archiveRoot = Join-Path $PWD \'.cache/bootstrap/zstd-1.5.7\'\\n$archive = Join-Path $archiveRoot \'zstd.zip\'\\nNew-Item -ItemType Directory -Force $archiveRoot | Out-Null\\n$checksum = \'acb4e8111511749dc7a3ebedca9b04190e37a17afeb73f55d4425dbf0b90fad9\'\\nif (!(Test-Path -LiteralPath $archive) -or (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) {\\n    Invoke-WebRequest \'https://github.com/facebook/zstd/releases/download/v1.5.7/zstd-v1.5.7-win64.zip\' -OutFile $archive\\n}\\nif ((Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) { throw \'Pinned zstd archive checksum mismatch\' }\\nExpand-Archive -LiteralPath $archive -DestinationPath $archiveRoot -Force\\n$compressor = Join-Path $archiveRoot \'zstd-v1.5.7-win64/zstd.exe\'\\nif (!(Test-Path -LiteralPath $compressor -PathType Leaf)) { throw \'Pinned zstd executable missing\' }\\n$compressorRoot = Split-Path -Parent $compressor\\n$env:PATH = (Split-Path -Parent $compiler) + [IO.Path]::PathSeparator + $compressorRoot + [IO.Path]::PathSeparator + $env:PATH\\n# === Verification === #\\n& $compressor --version\\nif ($LASTEXITCODE -ne 0) { throw \'Pinned zstd cannot execute\' }\\n$selectedLink = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'link.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }) | Select-Object -First 1\\nif ($selectedLink -ne (Join-Path (Split-Path -Parent $compiler) \'link.exe\')) { throw \'MSVC linker precedence is required\' }\\n# === Entry Point === #\\n$paths = @($env:PATH -split [IO.Path]::PathSeparator | ForEach-Object { $_.Trim(\'\\"\') } | Where-Object { [IO.Path]::GetPathRoot($_).Length -ge 3 })\\n$candidates = @($paths | ForEach-Object { Join-Path $_ \'tar.exe\' })\\nforeach ($git in @($paths | ForEach-Object { Join-Path $_ \'git.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })) {\\n  $root = Split-Path -Parent (Split-Path -Parent $git)\\n  $candidates += Join-Path $root \'usr/bin/tar.exe\'\\n}\\n$tar = $null\\nforeach ($candidate in @($candidates | Select-Object -Unique)) {\\n  if ([IO.Path]::GetPathRoot($candidate).Length -lt 3 -or !(Test-Path -LiteralPath $candidate -PathType Leaf)) { continue }\\n  try {\\n    $version = & $candidate --version 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$version\\" -notmatch \'^tar \\\\(GNU tar\\\\)\') { continue }\\n    $capabilities = & $candidate --help 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$capabilities\\" -notmatch \'--force-local\' -or \\"$capabilities\\" -notmatch \'--use-compress-program\') { continue }\\n    $tar = $candidate\\n    break\\n  } catch { continue }\\n}\\nif (!$tar) { throw \'GNU tar with force-local and external-compressor support is required from the standard Git image baseline\' }\\n$env:PATH = (@((Split-Path -Parent $compiler), (Split-Path -Parent $tar), $compressorRoot) + @($env:PATH -split [IO.Path]::PathSeparator) | Select-Object -Unique) -join [IO.Path]::PathSeparator\\nif ($env:GITHUB_ENV) {\\n  foreach ($name in @(\'PATH\', \'INCLUDE\', \'LIB\', \'LIBPATH\')) {\\n    $value = [Environment]::GetEnvironmentVariable($name)\\n    if ($null -ne $value) { \\"$name=$value\\" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append }\\n  }\\n}","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build,build:rust:tests,prepare:tests:fixtures","image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_system_windows","id":"test:rust:system","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":["build_product_windows","build_tests_windows","prepare_test_fixtures_windows"],"inputs":["build#product","build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:system --profile ${{ env.PROFILE }}","cached":false,"shell":null}],"bootstrap":[],"setup":"$ErrorActionPreference = \'Stop\'\\n# === Setup === #\\n$vswhere = Join-Path ${env:ProgramFiles(x86)} \'Microsoft Visual Studio/Installer/vswhere.exe\'\\nif (!(Test-Path -LiteralPath $vswhere -PathType Leaf)) { throw \'The standard Windows image requires installed Visual Studio Build Tools\' }\\n$installation = & $vswhere -latest -products \'*\' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath\\nif ($LASTEXITCODE -ne 0 -or !$installation) { throw \'The standard Windows image requires installed Visual Studio C++ tools\' }\\nImport-Module (Join-Path $installation \'Common7/Tools/Microsoft.VisualStudio.DevShell.dll\')\\nEnter-VsDevShell -VsInstallPath $installation -SkipAutomaticLocation -DevCmdArguments \'-arch=x64 -host_arch=x64\' | Out-Null\\n$compiler = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'cl.exe\' } | Where-Object { (Test-Path -LiteralPath $_ -PathType Leaf) -and $_.StartsWith($installation, [StringComparison]::OrdinalIgnoreCase) }) | Select-Object -First 1\\nif (!$compiler -or !(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $compiler) \'link.exe\') -PathType Leaf)) { throw \'MSVC compiler and linker are required from the selected installation\' }\\n$archiveRoot = Join-Path $PWD \'.cache/bootstrap/zstd-1.5.7\'\\n$archive = Join-Path $archiveRoot \'zstd.zip\'\\nNew-Item -ItemType Directory -Force $archiveRoot | Out-Null\\n$checksum = \'acb4e8111511749dc7a3ebedca9b04190e37a17afeb73f55d4425dbf0b90fad9\'\\nif (!(Test-Path -LiteralPath $archive) -or (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) {\\n    Invoke-WebRequest \'https://github.com/facebook/zstd/releases/download/v1.5.7/zstd-v1.5.7-win64.zip\' -OutFile $archive\\n}\\nif ((Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) { throw \'Pinned zstd archive checksum mismatch\' }\\nExpand-Archive -LiteralPath $archive -DestinationPath $archiveRoot -Force\\n$compressor = Join-Path $archiveRoot \'zstd-v1.5.7-win64/zstd.exe\'\\nif (!(Test-Path -LiteralPath $compressor -PathType Leaf)) { throw \'Pinned zstd executable missing\' }\\n$compressorRoot = Split-Path -Parent $compressor\\n$env:PATH = (Split-Path -Parent $compiler) + [IO.Path]::PathSeparator + $compressorRoot + [IO.Path]::PathSeparator + $env:PATH\\n# === Verification === #\\n& $compressor --version\\nif ($LASTEXITCODE -ne 0) { throw \'Pinned zstd cannot execute\' }\\n$selectedLink = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'link.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }) | Select-Object -First 1\\nif ($selectedLink -ne (Join-Path (Split-Path -Parent $compiler) \'link.exe\')) { throw \'MSVC linker precedence is required\' }\\n# === Entry Point === #\\n$paths = @($env:PATH -split [IO.Path]::PathSeparator | ForEach-Object { $_.Trim(\'\\"\') } | Where-Object { [IO.Path]::GetPathRoot($_).Length -ge 3 })\\n$candidates = @($paths | ForEach-Object { Join-Path $_ \'tar.exe\' })\\nforeach ($git in @($paths | ForEach-Object { Join-Path $_ \'git.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })) {\\n  $root = Split-Path -Parent (Split-Path -Parent $git)\\n  $candidates += Join-Path $root \'usr/bin/tar.exe\'\\n}\\n$tar = $null\\nforeach ($candidate in @($candidates | Select-Object -Unique)) {\\n  if ([IO.Path]::GetPathRoot($candidate).Length -lt 3 -or !(Test-Path -LiteralPath $candidate -PathType Leaf)) { continue }\\n  try {\\n    $version = & $candidate --version 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$version\\" -notmatch \'^tar \\\\(GNU tar\\\\)\') { continue }\\n    $capabilities = & $candidate --help 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$capabilities\\" -notmatch \'--force-local\' -or \\"$capabilities\\" -notmatch \'--use-compress-program\') { continue }\\n    $tar = $candidate\\n    break\\n  } catch { continue }\\n}\\nif (!$tar) { throw \'GNU tar with force-local and external-compressor support is required from the standard Git image baseline\' }\\n$env:PATH = (@((Split-Path -Parent $compiler), (Split-Path -Parent $tar), $compressorRoot) + @($env:PATH -split [IO.Path]::PathSeparator) | Select-Object -Unique) -join [IO.Path]::PathSeparator\\nif ($env:GITHUB_ENV) {\\n  foreach ($name in @(\'PATH\', \'INCLUDE\', \'LIB\', \'LIBPATH\')) {\\n    $value = [Environment]::GetEnvironmentVariable($name)\\n    if ($null -ne $value) { \\"$name=$value\\" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append }\\n  }\\n}","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build,build:rust:tests,prepare:tests:fixtures","image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_starter_windows","id":"test:rust:starter","platform":"windows","independent":false,"pipelines":["nightly"],"needs":["build_product_windows","build_tests_windows","prepare_test_fixtures_windows"],"inputs":["build#product","build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:starter --profile ${{ env.PROFILE }}","cached":false,"shell":null}],"bootstrap":[],"setup":"$ErrorActionPreference = \'Stop\'\\n# === Setup === #\\n$vswhere = Join-Path ${env:ProgramFiles(x86)} \'Microsoft Visual Studio/Installer/vswhere.exe\'\\nif (!(Test-Path -LiteralPath $vswhere -PathType Leaf)) { throw \'The standard Windows image requires installed Visual Studio Build Tools\' }\\n$installation = & $vswhere -latest -products \'*\' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath\\nif ($LASTEXITCODE -ne 0 -or !$installation) { throw \'The standard Windows image requires installed Visual Studio C++ tools\' }\\nImport-Module (Join-Path $installation \'Common7/Tools/Microsoft.VisualStudio.DevShell.dll\')\\nEnter-VsDevShell -VsInstallPath $installation -SkipAutomaticLocation -DevCmdArguments \'-arch=x64 -host_arch=x64\' | Out-Null\\n$compiler = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'cl.exe\' } | Where-Object { (Test-Path -LiteralPath $_ -PathType Leaf) -and $_.StartsWith($installation, [StringComparison]::OrdinalIgnoreCase) }) | Select-Object -First 1\\nif (!$compiler -or !(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $compiler) \'link.exe\') -PathType Leaf)) { throw \'MSVC compiler and linker are required from the selected installation\' }\\n$archiveRoot = Join-Path $PWD \'.cache/bootstrap/zstd-1.5.7\'\\n$archive = Join-Path $archiveRoot \'zstd.zip\'\\nNew-Item -ItemType Directory -Force $archiveRoot | Out-Null\\n$checksum = \'acb4e8111511749dc7a3ebedca9b04190e37a17afeb73f55d4425dbf0b90fad9\'\\nif (!(Test-Path -LiteralPath $archive) -or (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) {\\n    Invoke-WebRequest \'https://github.com/facebook/zstd/releases/download/v1.5.7/zstd-v1.5.7-win64.zip\' -OutFile $archive\\n}\\nif ((Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) { throw \'Pinned zstd archive checksum mismatch\' }\\nExpand-Archive -LiteralPath $archive -DestinationPath $archiveRoot -Force\\n$compressor = Join-Path $archiveRoot \'zstd-v1.5.7-win64/zstd.exe\'\\nif (!(Test-Path -LiteralPath $compressor -PathType Leaf)) { throw \'Pinned zstd executable missing\' }\\n$compressorRoot = Split-Path -Parent $compressor\\n$env:PATH = (Split-Path -Parent $compiler) + [IO.Path]::PathSeparator + $compressorRoot + [IO.Path]::PathSeparator + $env:PATH\\n# === Verification === #\\n& $compressor --version\\nif ($LASTEXITCODE -ne 0) { throw \'Pinned zstd cannot execute\' }\\n$selectedLink = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'link.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }) | Select-Object -First 1\\nif ($selectedLink -ne (Join-Path (Split-Path -Parent $compiler) \'link.exe\')) { throw \'MSVC linker precedence is required\' }\\n# === Entry Point === #\\n$paths = @($env:PATH -split [IO.Path]::PathSeparator | ForEach-Object { $_.Trim(\'\\"\') } | Where-Object { [IO.Path]::GetPathRoot($_).Length -ge 3 })\\n$candidates = @($paths | ForEach-Object { Join-Path $_ \'tar.exe\' })\\nforeach ($git in @($paths | ForEach-Object { Join-Path $_ \'git.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })) {\\n  $root = Split-Path -Parent (Split-Path -Parent $git)\\n  $candidates += Join-Path $root \'usr/bin/tar.exe\'\\n}\\n$tar = $null\\nforeach ($candidate in @($candidates | Select-Object -Unique)) {\\n  if ([IO.Path]::GetPathRoot($candidate).Length -lt 3 -or !(Test-Path -LiteralPath $candidate -PathType Leaf)) { continue }\\n  try {\\n    $version = & $candidate --version 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$version\\" -notmatch \'^tar \\\\(GNU tar\\\\)\') { continue }\\n    $capabilities = & $candidate --help 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$capabilities\\" -notmatch \'--force-local\' -or \\"$capabilities\\" -notmatch \'--use-compress-program\') { continue }\\n    $tar = $candidate\\n    break\\n  } catch { continue }\\n}\\nif (!$tar) { throw \'GNU tar with force-local and external-compressor support is required from the standard Git image baseline\' }\\n$env:PATH = (@((Split-Path -Parent $compiler), (Split-Path -Parent $tar), $compressorRoot) + @($env:PATH -split [IO.Path]::PathSeparator) | Select-Object -Unique) -join [IO.Path]::PathSeparator\\nif ($env:GITHUB_ENV) {\\n  foreach ($name in @(\'PATH\', \'INCLUDE\', \'LIB\', \'LIBPATH\')) {\\n    $value = [Environment]::GetEnvironmentVariable($name)\\n    if ($null -ne $value) { \\"$name=$value\\" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append }\\n  }\\n}","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build,build:rust:tests,prepare:tests:fixtures","image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_coverage_rust","id":"test:rust:coverage","platform":"linux","independent":true,"pipelines":["nightly"],"needs":["build_product_linux","build_tests_linux","prepare_test_fixtures_linux"],"inputs":["build#product","build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:coverage --profile ${{ env.PROFILE }}","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:taiki-e/cargo-llvm-cov,github:bnjbvr/cargo-machete,sccache,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,npm:@fission-ai/openspec,taplo","skip":"build,build:rust:tests,prepare:tests:fixtures","image":"managed:ubuntu-24.04","minutes":120},{"key":"build_bench","id":"build:rust:bench","platform":"linux","independent":true,"pipelines":["nightly"],"needs":[],"inputs":[],"outputs":{"bench":["target/build/bench/{target}/bench","target/build/bench/{target}/bin","target/build/bench/{target}/fixtures"]},"commands":[{"run":"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.build --profile bench --output bench --task build:rust:bench","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"aqua:koalaman/shellcheck,aqua:nextest-rs/nextest/cargo-nextest,github:bnjbvr/cargo-machete,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,npm:@fission-ai/openspec,python,rust,sccache,taplo,uv","skip":null,"image":"managed:ubuntu-24.04","minutes":120},{"key":"bench_rust_integration","id":"bench:rust:integration","platform":"linux","independent":true,"pipelines":["nightly"],"needs":["build_bench","check_generated_python","check_python","check_rust_linux","check_rust_windows","source","validate_coverage_rust","validate_test_rust_acceptance_linux","validate_test_rust_acceptance_windows","validate_test_rust_integration_linux","validate_test_rust_integration_windows","validate_test_rust_starter_linux","validate_test_rust_starter_windows","validate_test_rust_system_linux","validate_test_rust_system_windows","validate_test_rust_unit_linux","validate_test_rust_unit_windows"],"inputs":["build:rust:bench#bench"],"outputs":{},"commands":[{"run":"mise run bench:rust:integration --refresh-baseline","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build:rust:bench","image":"managed:ubuntu-24.04","minutes":120},{"key":"bench_rust_system","id":"bench:rust:system","platform":"linux","independent":true,"pipelines":["nightly"],"needs":["build_bench","check_generated_python","check_python","check_rust_linux","check_rust_windows","source","validate_coverage_rust","validate_test_rust_acceptance_linux","validate_test_rust_acceptance_windows","validate_test_rust_integration_linux","validate_test_rust_integration_windows","validate_test_rust_starter_linux","validate_test_rust_starter_windows","validate_test_rust_system_linux","validate_test_rust_system_windows","validate_test_rust_unit_linux","validate_test_rust_unit_windows"],"inputs":["build:rust:bench#bench"],"outputs":{},"commands":[{"run":"mise run bench:rust:system --refresh-baseline","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build:rust:bench","image":"managed:ubuntu-24.04","minutes":120},{"key":"bench_rust_unit","id":"bench:rust:unit","platform":"linux","independent":true,"pipelines":["nightly"],"needs":["build_bench","check_generated_python","check_python","check_rust_linux","check_rust_windows","source","validate_coverage_rust","validate_test_rust_acceptance_linux","validate_test_rust_acceptance_windows","validate_test_rust_integration_linux","validate_test_rust_integration_windows","validate_test_rust_starter_linux","validate_test_rust_starter_windows","validate_test_rust_system_linux","validate_test_rust_system_windows","validate_test_rust_unit_linux","validate_test_rust_unit_windows"],"inputs":["build:rust:bench#bench"],"outputs":{},"commands":[{"run":"mise run bench:rust:unit --refresh-baseline","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build:rust:bench","image":"managed:ubuntu-24.04","minutes":120},{"key":"check_rust_linux","id":"check:rust","platform":"linux","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":[],"inputs":[],"outputs":{},"commands":[{"run":"mise run check:rust","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"go,node,python,rust,uv,github:bnjbvr/cargo-machete,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,npm:@fission-ai/openspec,taplo","skip":null,"image":"managed:ubuntu-24.04","minutes":120},{"key":"build_product_linux","id":"build","platform":"linux","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":[],"inputs":[],"outputs":{"product":["target/build/{profile}/{target}/bin","target/build/{profile}/{target}/lib","target/build/{profile}/{target}/product","target/generated/python/pyproject.toml","target/generated/python/src"]},"commands":[{"run":"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.build --profile ${{ env.PROFILE }} --output product --task build","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"aqua:koalaman/shellcheck,aqua:nextest-rs/nextest/cargo-nextest,github:bnjbvr/cargo-machete,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,npm:@fission-ai/openspec,python,rust,sccache,taplo,uv","skip":null,"image":"managed:ubuntu-24.04","minutes":120},{"key":"build_tests_linux","id":"build:rust:tests","platform":"linux","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":[],"inputs":[],"outputs":{"fixtures":["target/build/{profile}/{target}/fixtures"],"tests":["target/build/{profile}/{target}/tests/archive.tar.zst","target/build/{profile}/{target}/tests/compiler.json"]},"commands":[{"run":"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.build --profile ${{ env.PROFILE }} --output tests --task build:rust:tests","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"aqua:koalaman/shellcheck,aqua:nextest-rs/nextest/cargo-nextest,github:bnjbvr/cargo-machete,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,npm:@fission-ai/openspec,python,rust,sccache,taplo,uv","skip":null,"image":"managed:ubuntu-24.04","minutes":120},{"key":"prepare_test_fixtures_linux","id":"prepare:tests:fixtures","platform":"linux","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":["build_product_linux","build_tests_linux"],"inputs":["build#product","build:rust:tests#fixtures"],"outputs":{"fixtures":["target/build/{profile}/{target}/fixtures"]},"commands":[{"run":"mise run prepare:tests:fixtures --profile ${{ env.PROFILE }}","cached":true,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"aqua:koalaman/shellcheck,aqua:nextest-rs/nextest/cargo-nextest,git-cliff,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,python,rust,sccache,uv","skip":"build,build:rust:tests","image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_unit_linux","id":"test:rust:unit","platform":"linux","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":["build_tests_linux"],"inputs":["build:rust:tests#tests"],"outputs":{},"commands":[{"run":"mise run test:rust:unit --profile ${{ env.PROFILE }}","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build:rust:tests","image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_integration_linux","id":"test:rust:integration","platform":"linux","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":["build_tests_linux","prepare_test_fixtures_linux"],"inputs":["build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:integration --profile ${{ env.PROFILE }}","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build:rust:tests,prepare:tests:fixtures","image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_acceptance_linux","id":"test:rust:acceptance","platform":"linux","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":["build_product_linux","build_tests_linux","prepare_test_fixtures_linux"],"inputs":["build#product","build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:acceptance --profile ${{ env.PROFILE }}","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build,build:rust:tests,prepare:tests:fixtures","image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_system_linux","id":"test:rust:system","platform":"linux","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":["build_product_linux","build_tests_linux","prepare_test_fixtures_linux"],"inputs":["build#product","build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:system --profile ${{ env.PROFILE }}","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build,build:rust:tests,prepare:tests:fixtures","image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_starter_linux","id":"test:rust:starter","platform":"linux","independent":false,"pipelines":["nightly"],"needs":["build_product_linux","build_tests_linux","prepare_test_fixtures_linux"],"inputs":["build#product","build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:starter --profile ${{ env.PROFILE }}","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build,build:rust:tests,prepare:tests:fixtures","image":"managed:ubuntu-24.04","minutes":120},{"key":"package_rust_linux","id":"package:rust","platform":"linux","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":["build_product_linux","check_generated_python","check_python","check_rust_linux","source"],"inputs":["build#product"],"outputs":{"package":["target/package/{profile}/{target}"]},"commands":[{"run":"mise run package:rust --profile ${{ env.PROFILE }}","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build","image":"managed:ubuntu-24.04","minutes":120},{"key":"package_rust_windows","id":"package:rust","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"needs":["build_product_windows","check_generated_python","check_python","check_rust_windows","source"],"inputs":["build#product"],"outputs":{"package":["target/package/{profile}/{target}"]},"commands":[{"run":"mise run package:rust --profile ${{ env.PROFILE }}","cached":false,"shell":null}],"bootstrap":[],"setup":"$ErrorActionPreference = \'Stop\'\\n# === Setup === #\\n$vswhere = Join-Path ${env:ProgramFiles(x86)} \'Microsoft Visual Studio/Installer/vswhere.exe\'\\nif (!(Test-Path -LiteralPath $vswhere -PathType Leaf)) { throw \'The standard Windows image requires installed Visual Studio Build Tools\' }\\n$installation = & $vswhere -latest -products \'*\' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath\\nif ($LASTEXITCODE -ne 0 -or !$installation) { throw \'The standard Windows image requires installed Visual Studio C++ tools\' }\\nImport-Module (Join-Path $installation \'Common7/Tools/Microsoft.VisualStudio.DevShell.dll\')\\nEnter-VsDevShell -VsInstallPath $installation -SkipAutomaticLocation -DevCmdArguments \'-arch=x64 -host_arch=x64\' | Out-Null\\n$compiler = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'cl.exe\' } | Where-Object { (Test-Path -LiteralPath $_ -PathType Leaf) -and $_.StartsWith($installation, [StringComparison]::OrdinalIgnoreCase) }) | Select-Object -First 1\\nif (!$compiler -or !(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $compiler) \'link.exe\') -PathType Leaf)) { throw \'MSVC compiler and linker are required from the selected installation\' }\\n$archiveRoot = Join-Path $PWD \'.cache/bootstrap/zstd-1.5.7\'\\n$archive = Join-Path $archiveRoot \'zstd.zip\'\\nNew-Item -ItemType Directory -Force $archiveRoot | Out-Null\\n$checksum = \'acb4e8111511749dc7a3ebedca9b04190e37a17afeb73f55d4425dbf0b90fad9\'\\nif (!(Test-Path -LiteralPath $archive) -or (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) {\\n    Invoke-WebRequest \'https://github.com/facebook/zstd/releases/download/v1.5.7/zstd-v1.5.7-win64.zip\' -OutFile $archive\\n}\\nif ((Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) { throw \'Pinned zstd archive checksum mismatch\' }\\nExpand-Archive -LiteralPath $archive -DestinationPath $archiveRoot -Force\\n$compressor = Join-Path $archiveRoot \'zstd-v1.5.7-win64/zstd.exe\'\\nif (!(Test-Path -LiteralPath $compressor -PathType Leaf)) { throw \'Pinned zstd executable missing\' }\\n$compressorRoot = Split-Path -Parent $compressor\\n$env:PATH = (Split-Path -Parent $compiler) + [IO.Path]::PathSeparator + $compressorRoot + [IO.Path]::PathSeparator + $env:PATH\\n# === Verification === #\\n& $compressor --version\\nif ($LASTEXITCODE -ne 0) { throw \'Pinned zstd cannot execute\' }\\n$selectedLink = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'link.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }) | Select-Object -First 1\\nif ($selectedLink -ne (Join-Path (Split-Path -Parent $compiler) \'link.exe\')) { throw \'MSVC linker precedence is required\' }\\n# === Entry Point === #\\n$paths = @($env:PATH -split [IO.Path]::PathSeparator | ForEach-Object { $_.Trim(\'\\"\') } | Where-Object { [IO.Path]::GetPathRoot($_).Length -ge 3 })\\n$candidates = @($paths | ForEach-Object { Join-Path $_ \'tar.exe\' })\\nforeach ($git in @($paths | ForEach-Object { Join-Path $_ \'git.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })) {\\n  $root = Split-Path -Parent (Split-Path -Parent $git)\\n  $candidates += Join-Path $root \'usr/bin/tar.exe\'\\n}\\n$tar = $null\\nforeach ($candidate in @($candidates | Select-Object -Unique)) {\\n  if ([IO.Path]::GetPathRoot($candidate).Length -lt 3 -or !(Test-Path -LiteralPath $candidate -PathType Leaf)) { continue }\\n  try {\\n    $version = & $candidate --version 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$version\\" -notmatch \'^tar \\\\(GNU tar\\\\)\') { continue }\\n    $capabilities = & $candidate --help 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$capabilities\\" -notmatch \'--force-local\' -or \\"$capabilities\\" -notmatch \'--use-compress-program\') { continue }\\n    $tar = $candidate\\n    break\\n  } catch { continue }\\n}\\nif (!$tar) { throw \'GNU tar with force-local and external-compressor support is required from the standard Git image baseline\' }\\n$env:PATH = (@((Split-Path -Parent $compiler), (Split-Path -Parent $tar), $compressorRoot) + @($env:PATH -split [IO.Path]::PathSeparator) | Select-Object -Unique) -join [IO.Path]::PathSeparator\\nif ($env:GITHUB_ENV) {\\n  foreach ($name in @(\'PATH\', \'INCLUDE\', \'LIB\', \'LIBPATH\')) {\\n    $value = [Environment]::GetEnvironmentVariable($name)\\n    if ($null -ne $value) { \\"$name=$value\\" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append }\\n  }\\n}","tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build","image":"managed:ubuntu-24.04","minutes":120},{"key":"verification_dry","id":"release:publish:dry","platform":"linux","independent":true,"pipeline_needs":{"pr":["source","check_python","check_rust_windows","build_product_windows","build_tests_windows","check_generated_python","prepare_test_fixtures_windows","validate_test_rust_unit_windows","validate_test_rust_integration_windows","validate_test_rust_acceptance_windows","validate_test_rust_system_windows","check_rust_linux","build_product_linux","build_tests_linux","prepare_test_fixtures_linux","validate_test_rust_unit_linux","validate_test_rust_integration_linux","validate_test_rust_acceptance_linux","validate_test_rust_system_linux","package_rust_linux","package_rust_windows"],"dev":["source","check_python","check_rust_windows","build_product_windows","build_tests_windows","check_generated_python","prepare_test_fixtures_windows","validate_test_rust_unit_windows","validate_test_rust_integration_windows","validate_test_rust_acceptance_windows","validate_test_rust_system_windows","check_rust_linux","build_product_linux","build_tests_linux","prepare_test_fixtures_linux","validate_test_rust_unit_linux","validate_test_rust_integration_linux","validate_test_rust_acceptance_linux","validate_test_rust_system_linux","package_rust_linux","package_rust_windows"],"release":["source","check_python","check_rust_windows","build_product_windows","build_tests_windows","check_generated_python","prepare_test_fixtures_windows","validate_test_rust_unit_windows","validate_test_rust_integration_windows","validate_test_rust_acceptance_windows","validate_test_rust_system_windows","check_rust_linux","build_product_linux","build_tests_linux","prepare_test_fixtures_linux","validate_test_rust_unit_linux","validate_test_rust_integration_linux","validate_test_rust_acceptance_linux","validate_test_rust_system_linux","package_rust_linux","package_rust_windows"],"nightly":["source","check_python","check_rust_windows","build_product_windows","build_tests_windows","check_generated_python","prepare_test_fixtures_windows","validate_test_rust_unit_windows","validate_test_rust_integration_windows","validate_test_rust_acceptance_windows","validate_test_rust_system_windows","validate_test_rust_starter_windows","check_rust_linux","build_product_linux","build_tests_linux","prepare_test_fixtures_linux","validate_test_rust_unit_linux","validate_test_rust_integration_linux","validate_test_rust_acceptance_linux","validate_test_rust_system_linux","validate_test_rust_starter_linux","package_rust_linux","package_rust_windows"]},"pipelines":["pr","dev","release","nightly"],"needs":["source","check_python","check_rust_windows","build_product_windows","build_tests_windows","check_generated_python","prepare_test_fixtures_windows","validate_test_rust_unit_windows","validate_test_rust_integration_windows","validate_test_rust_acceptance_windows","validate_test_rust_system_windows","validate_test_rust_starter_windows","check_rust_linux","build_product_linux","build_tests_linux","prepare_test_fixtures_linux","validate_test_rust_unit_linux","validate_test_rust_integration_linux","validate_test_rust_acceptance_linux","validate_test_rust_system_linux","validate_test_rust_starter_linux","package_rust_linux","package_rust_windows"],"inputs":["package:rust#package"],"outputs":{},"commands":[{"run":"mise run release:publish:dry","cached":false,"shell":null}],"bootstrap":[],"setup":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #","tools":"go,node,python,rust,uv,gh,git-cliff,glab","skip":null,"image":"managed:ubuntu-24.04","minutes":120}],"mise_image":"ghcr.io/jdx/mise:2026.10.8-debian@sha256:8cc0b83dd86465df9ae4a813cce9ce24cb0b4f749617c419d3e4beb838b365b1","impact":{"rules":[{"only":["^README\\\\.md$","^CONTRIBUTING\\\\.md$","^LICENSE$","^LICENSE\\\\..*$","^docs(?:/|/.*/).*\\\\.md$"],"exclude":["^docs/examples/.*$","^docs/tutorials/.*$","^docs/src/.*$"],"jobs":["check:repository"]}],"protected":["^\\\\.github/workflows/.*$","^\\\\.github/actions/.*$","^\\\\.gitlab/.*$","^\\\\.mise/.*$","^\\\\.stkr/.*$","^\\\\.agents/.*$","^src/plugin/.*$","^src/templates/.*$","^src/guidance/.*$","^(?:/?|.*/)stkr\\\\.toml$","^(?:/?|.*/)mise.*\\\\.toml$","^(?:/?|.*/)mise\\\\.lock$","^(?:/?|.*/)\\\\.mise/.*$","^(?:/?|.*/)mise\\\\-tasks/.*$","^(?:/?|.*/)\\\\.cargo/.*$","^(?:/?|.*/)rust\\\\-toolchain.*$","^(?:/?|.*/)\\\\.gitlab\\\\-ci\\\\.yml$","^(?:/?|.*/)\\\\.gitlab\\\\-ci\\\\.yaml$","^(?:/?|.*/)Cargo\\\\.toml$","^(?:/?|.*/)Cargo\\\\.lock$","^(?:/?|.*/)build\\\\.rs$","^(?:/?|.*/)package\\\\.json$","^(?:/?|.*/)package\\\\-lock\\\\.json$","^(?:/?|.*/)npm\\\\-shrinkwrap\\\\.json$","^(?:/?|.*/)pnpm\\\\-workspace\\\\.yaml$","^(?:/?|.*/)pyproject\\\\.toml$","^(?:/?|.*/).*\\\\.lock$","^(?:/?|.*/)pnpm\\\\-lock\\\\.yaml$","^(?:/?|.*/)Dockerfile.*$","^(?:/?|.*/)Containerfile.*$","^(?:/?|.*/)Makefile$","^(?:/?|.*/)CMakeLists\\\\.txt$","^(?:/?|.*/)nextest\\\\.toml$","^(?:/?|.*/)deny\\\\.toml$","^(?:/?|.*/)ruff\\\\.toml$","^(?:/?|.*/)biome\\\\.json.*$","^(?:/?|.*/)tsconfig.*\\\\.json$","^(?:/?|.*/)AGENTS\\\\.md$","^(?:/?|.*/)CLAUDE\\\\.md$"],"dependencies":{"bench:rust:integration":["build:rust:bench","check:python","check:python:generated","check:repository","check:rust","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"bench:rust:system":["build:rust:bench","check:python","check:python:generated","check:repository","check:rust","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"bench:rust:unit":["build:rust:bench","check:python","check:python:generated","check:repository","check:rust","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"build":[],"build:rust:bench":[],"build:rust:tests":[],"check:python":[],"check:python:generated":["build"],"check:repository":[],"check:rust":[],"package:rust":["build","check:python","check:python:generated","check:repository","check:rust"],"prepare:tests:fixtures":["build","build:rust:tests"],"release:prepare":["bench:rust:integration","bench:rust:system","bench:rust:unit","build","build:rust:bench","build:rust:tests","check:python","check:python:generated","check:repository","check:rust","package:rust","prepare:tests:fixtures","release:publish:dry","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"release:publish":["bench:rust:integration","bench:rust:system","bench:rust:unit","build","build:rust:bench","build:rust:tests","check:python","check:python:generated","check:repository","check:rust","package:rust","prepare:tests:fixtures","release:publish:dry","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"release:publish:dry":["bench:rust:integration","bench:rust:system","bench:rust:unit","build","build:rust:bench","build:rust:tests","check:python","check:python:generated","check:repository","check:rust","package:rust","prepare:tests:fixtures","release:publish:dry","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"release:rehearse":["bench:rust:integration","bench:rust:system","bench:rust:unit","build","build:rust:bench","build:rust:tests","check:python","check:python:generated","check:repository","check:rust","package:rust","prepare:tests:fixtures","release:publish:dry","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"release:tag":["bench:rust:integration","bench:rust:system","bench:rust:unit","build","build:rust:bench","build:rust:tests","check:python","check:python:generated","check:repository","check:rust","package:rust","prepare:tests:fixtures","release:publish:dry","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"release:verify":["bench:rust:integration","bench:rust:system","bench:rust:unit","build","build:rust:bench","build:rust:tests","check:python","check:python:generated","check:repository","check:rust","package:rust","prepare:tests:fixtures","release:publish:dry","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"test:rust:acceptance":["build","build:rust:tests","prepare:tests:fixtures"],"test:rust:coverage":["build","build:rust:tests","prepare:tests:fixtures"],"test:rust:integration":["build:rust:tests","prepare:tests:fixtures"],"test:rust:starter":["build","build:rust:tests","prepare:tests:fixtures"],"test:rust:system":["build","build:rust:tests","prepare:tests:fixtures"],"test:rust:unit":["build:rust:tests"]}},"setup":{"windows":"$ErrorActionPreference = \'Stop\'\\n# === Setup === #\\n$vswhere = Join-Path ${env:ProgramFiles(x86)} \'Microsoft Visual Studio/Installer/vswhere.exe\'\\nif (!(Test-Path -LiteralPath $vswhere -PathType Leaf)) { throw \'The standard Windows image requires installed Visual Studio Build Tools\' }\\n$installation = & $vswhere -latest -products \'*\' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath\\nif ($LASTEXITCODE -ne 0 -or !$installation) { throw \'The standard Windows image requires installed Visual Studio C++ tools\' }\\nImport-Module (Join-Path $installation \'Common7/Tools/Microsoft.VisualStudio.DevShell.dll\')\\nEnter-VsDevShell -VsInstallPath $installation -SkipAutomaticLocation -DevCmdArguments \'-arch=x64 -host_arch=x64\' | Out-Null\\n$compiler = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'cl.exe\' } | Where-Object { (Test-Path -LiteralPath $_ -PathType Leaf) -and $_.StartsWith($installation, [StringComparison]::OrdinalIgnoreCase) }) | Select-Object -First 1\\nif (!$compiler -or !(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $compiler) \'link.exe\') -PathType Leaf)) { throw \'MSVC compiler and linker are required from the selected installation\' }\\n$archiveRoot = Join-Path $PWD \'.cache/bootstrap/zstd-1.5.7\'\\n$archive = Join-Path $archiveRoot \'zstd.zip\'\\nNew-Item -ItemType Directory -Force $archiveRoot | Out-Null\\n$checksum = \'acb4e8111511749dc7a3ebedca9b04190e37a17afeb73f55d4425dbf0b90fad9\'\\nif (!(Test-Path -LiteralPath $archive) -or (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) {\\n    Invoke-WebRequest \'https://github.com/facebook/zstd/releases/download/v1.5.7/zstd-v1.5.7-win64.zip\' -OutFile $archive\\n}\\nif ((Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant() -ne $checksum) { throw \'Pinned zstd archive checksum mismatch\' }\\nExpand-Archive -LiteralPath $archive -DestinationPath $archiveRoot -Force\\n$compressor = Join-Path $archiveRoot \'zstd-v1.5.7-win64/zstd.exe\'\\nif (!(Test-Path -LiteralPath $compressor -PathType Leaf)) { throw \'Pinned zstd executable missing\' }\\n$compressorRoot = Split-Path -Parent $compressor\\n$env:PATH = (Split-Path -Parent $compiler) + [IO.Path]::PathSeparator + $compressorRoot + [IO.Path]::PathSeparator + $env:PATH\\n# === Verification === #\\n& $compressor --version\\nif ($LASTEXITCODE -ne 0) { throw \'Pinned zstd cannot execute\' }\\n$selectedLink = @($env:PATH -split [IO.Path]::PathSeparator | Where-Object { [IO.Path]::IsPathFullyQualified($_) } | ForEach-Object { Join-Path $_ \'link.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }) | Select-Object -First 1\\nif ($selectedLink -ne (Join-Path (Split-Path -Parent $compiler) \'link.exe\')) { throw \'MSVC linker precedence is required\' }\\n# === Entry Point === #\\n$paths = @($env:PATH -split [IO.Path]::PathSeparator | ForEach-Object { $_.Trim(\'\\"\') } | Where-Object { [IO.Path]::GetPathRoot($_).Length -ge 3 })\\n$candidates = @($paths | ForEach-Object { Join-Path $_ \'tar.exe\' })\\nforeach ($git in @($paths | ForEach-Object { Join-Path $_ \'git.exe\' } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })) {\\n  $root = Split-Path -Parent (Split-Path -Parent $git)\\n  $candidates += Join-Path $root \'usr/bin/tar.exe\'\\n}\\n$tar = $null\\nforeach ($candidate in @($candidates | Select-Object -Unique)) {\\n  if ([IO.Path]::GetPathRoot($candidate).Length -lt 3 -or !(Test-Path -LiteralPath $candidate -PathType Leaf)) { continue }\\n  try {\\n    $version = & $candidate --version 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$version\\" -notmatch \'^tar \\\\(GNU tar\\\\)\') { continue }\\n    $capabilities = & $candidate --help 2>$null\\n    if ($LASTEXITCODE -ne 0 -or \\"$capabilities\\" -notmatch \'--force-local\' -or \\"$capabilities\\" -notmatch \'--use-compress-program\') { continue }\\n    $tar = $candidate\\n    break\\n  } catch { continue }\\n}\\nif (!$tar) { throw \'GNU tar with force-local and external-compressor support is required from the standard Git image baseline\' }\\n$env:PATH = (@((Split-Path -Parent $compiler), (Split-Path -Parent $tar), $compressorRoot) + @($env:PATH -split [IO.Path]::PathSeparator) | Select-Object -Unique) -join [IO.Path]::PathSeparator\\nif ($env:GITHUB_ENV) {\\n  foreach ($name in @(\'PATH\', \'INCLUDE\', \'LIB\', \'LIBPATH\')) {\\n    $value = [Environment]::GetEnvironmentVariable($name)\\n    if ($null -ne $value) { \\"$name=$value\\" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append }\\n  }\\n}","linux":"set -eu\\n# === Setup === #\\nnative_tools=\'tar zstd unzip zip\'\\ncase \\",${MISE_ENABLE_TOOLS:-rust},\\" in\\n    *,rust,*|*,cargo:*)\\n        native_tools=\\"$native_tools cc c++ make pkg-config\\"\\n        compiler=1\\n        ;;\\n    *) compiler=0 ;;\\nesac\\nmissing=\'\'\\nfor tool in $native_tools; do\\n    if ! command -v \\"$tool\\" >/dev/null 2>&1; then missing=\\"$missing $tool\\"; fi\\ndone\\ncase \\"$(tar --version 2>/dev/null || true)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) missing=\\"$missing GNU-tar\\" ;;\\nesac\\nif [ \\"$compiler\\" -eq 1 ] && ! pkg-config --exists openssl 2>/dev/null; then\\n    missing=\\"$missing openssl-development\\"\\nfi\\ninstall_prerequisites() {\\n    set -- tar zstd unzip zip\\n    if command -v apt-get >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-essential pkg-config libssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then\\n            apt-get update\\n            apt-get install -y --no-install-recommends \\"$@\\"\\n        else\\n            sudo -n apt-get update\\n            sudo -n apt-get install -y --no-install-recommends \\"$@\\"\\n        fi\\n    elif command -v apk >/dev/null 2>&1; then\\n        if [ \\"$compiler\\" -eq 1 ]; then set -- \\"$@\\" build-base pkgconf openssl-dev; fi\\n        if [ \\"$(id -u)\\" -eq 0 ]; then apk add --no-cache \\"$@\\"\\n        else sudo -n apk add --no-cache \\"$@\\"; fi\\n    else\\n        printf \'Missing shared CI prerequisites:%s; provide them through the declared job bootstrap or pinned image.\\\\n\' \\"$missing\\" >&2\\n        exit 1\\n    fi\\n}\\nif [ -n \\"$missing\\" ]; then install_prerequisites; fi\\n# === Verification === #\\ncase \\"$(tar --version)\\" in\\n    \'tar (GNU tar)\'*) ;;\\n    *) printf \'GNU tar is required.\\\\n\' >&2; exit 1 ;;\\nesac\\nzstd --version\\n# === Entry Point === #"}}'
)
GIB = 1024**3
PRINT = threading.Lock()
NATIVE_LIMIT: NativeLimit | None = None
BUDGET = None


# === Types === #

# === Classes === #


# === Public Functions === #


def cache_root(root: Path) -> Path:
    identity = hashlib.sha256(str(root).encode()).hexdigest()[:16]
    if sys.platform == "win32":
        return Path.home() / ".cache/stkr/ci" / identity
    return (
        Path(os.environ.get("LOCALAPPDATA", os.environ.get("XDG_CACHE_HOME", str(Path.home() / ".cache"))))
        / "stkr/ci-simulation"
        / identity
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Simulate The Declared CI Graph")
    parser.add_argument("--pipeline", choices=("pr", "dev", "release", "nightly"), default="pr")
    parser.add_argument("--platform", action="append", choices=("windows", "linux"))
    parser.add_argument("--executor", action="append", help="platform=native|docker")
    parser.add_argument("--cpus", type=int)
    parser.add_argument("--memory-gib", type=float)
    parser.add_argument("--export", help=argparse.SUPPRESS)
    options = parser.parse_args()
    if options.export:
        export(Path.cwd(), json.loads(options.export), Path("target/ci/simulation-output.tar"))
        return 0
    global NATIVE_LIMIT, BUDGET
    started = time.monotonic()
    results = {}
    output = None
    peak_memory = None
    try:
        root = next(path for path in (Path.cwd(), *Path.cwd().parents) if (path / "mise.toml").is_file())
        platforms = options.platform or PLAN["platforms"]
        executors = {platform: "native" if platform == "windows" else "docker" for platform in platforms}
        for selection in options.executor or []:
            platform, executor = selection.split("=", 1)
            if platform not in platforms or executor not in ("native", "docker"):
                raise SimulationError("executor must select a declared platform=native|docker")
            executors[platform] = executor
        jobs = [
            job
            for job in PLAN["jobs"]
            if options.pipeline in job["pipelines"] and (job["platform"] in platforms or job["platform"] == "any" or job.get("independent"))
        ]
        branch = subprocess.run(["git", "branch", "--show-current"], cwd=root, capture_output=True, text=True, check=False).stdout.strip()
        release_review = options.pipeline == "pr" and branch.startswith("release/v")
        if options.pipeline in ("pr", "dev") and not release_review:
            selected = impact.selection(root, PLAN["impact"])
            jobs = [job for job in jobs if job["id"] in selected]
        if set(platforms) != set(PLAN["platforms"]):
            jobs = [job for job in jobs if job["id"] != "release:publish:dry"]
            emit("Scope", "Partial platform run: aggregate Release / Dry is excluded; this run does not satisfy the review gate")
        keys = {job["key"] for job in jobs}
        for job in jobs:
            job["needs"] = job.get("pipeline_needs", {}).get(options.pipeline, job["needs"])
            unavailable = set(job["needs"]) - keys
            if unavailable:
                raise SimulationError(f"{job['key']}: unavailable selected prerequisites: {sorted(unavailable)}")
            platform = job["platform"] if job["platform"] in platforms else platforms[0]
            job["execution_platform"] = platform
            job["setup"] = PLAN["setup"][platform]
            job["executor"] = executors[platform]
        if not jobs:
            raise SimulationError("no jobs match the selected pipeline and platforms")
        needed = {job["execution_platform"] for job in jobs}
        cpu, memory, limits = preflight(
            {platform: executor for platform, executor in executors.items() if platform in needed}, options.cpus, options.memory_gib
        )
        docker_jobs = [job for job in jobs if job["executor"] == "docker"]
        if any(job["image"] == "managed:ubuntu-24.04" for job in docker_jobs):
            image = managed_image(PLAN["mise_image"], PLAN["setup"]["linux"])
            for job in docker_jobs:
                if job["image"] == "managed:ubuntu-24.04":
                    job["image"] = image
        for image in {job["image"] for job in docker_jobs}:
            probe = subprocess.run(
                ["docker", "run", "--rm", "--entrypoint", "bash", image, "-c", "command -v python3 && command -v mise && command -v git"],
                capture_output=True,
                text=True,
                check=False,
            )
            if probe.returncode:
                raise SimulationError(
                    f"Linux image {image} must provide bash, Python3, mise and git before authored bootstrap; no fallback was applied"
                )
        BUDGET = Budget(cpu, memory, limits, report=lambda message: emit("Resources", message))
        emit("Resources", f"CPU={cpu}; available RAM={memory / GIB:.2f} GiB; executor ceilings={limits}")
        if os.name == "nt" and any(job["executor"] == "native" for job in jobs):
            NATIVE_LIMIT = NativeLimit(cpu, memory)
            emit(
                "Resources",
                "Native commands share one aggregate Windows process-tree CPU/RAM limit; Docker jobs share scheduler reservations and the engine ceiling",
            )
        BUDGET.native = NATIVE_LIMIT
        BUDGET.update([], jobs)
        token = uuid.uuid4().hex[:12]
        output = root / "target/ci/simulation" / token
        output.mkdir(parents=True)
        cache = cache_root(root)
        identity = cache.name
        cache.mkdir(parents=True, exist_ok=True)
        profile = "release" if options.pipeline in ("release", "nightly") or release_review else "dev"
        with tempfile.TemporaryDirectory(prefix="stkr-ci-simulation-") as temporary:
            candidate = Path(temporary) / "candidate"
            stage(root, candidate)

            def execute(job, assigned_cpu, assigned_memory):
                return run_job(job, candidate, output, cache, identity, token, profile, assigned_cpu, assigned_memory)

            results = schedule(jobs, cpu, memory, limits, execute)
        return 0 if all(result["state"] == "passed" for result in results.values()) else 1
    except (OSError, ValueError, StopIteration, SimulationError, subprocess.SubprocessError) as error:
        emit("Preflight", str(error))
        return 1
    except KeyboardInterrupt:
        if BUDGET:
            BUDGET.cancel()
        return 130
    finally:
        if NATIVE_LIMIT:
            peak_memory = NATIVE_LIMIT.peak()
            NATIVE_LIMIT.close()
            NATIVE_LIMIT = None
        BUDGET = None
        summary = summarize(results, time.monotonic() - started)
        if peak_memory is not None:
            summary["native_peak_memory_bytes"] = peak_memory
            emit("Memory", f"Native Peak={peak_memory / GIB:.2f} GiB")
        statistics = summary["cache"] if isinstance(summary["cache"], dict) else {}
        reporting.render(
            "CI Simulation",
            summary["wall_seconds"],
            statistics,
            phases={
                "Critical Path": summary["critical_path_seconds"],
                "Resource Wait": sum(result["wait_seconds"] for result in results.values()),
            },
            caches={name: {"Restore": "Unavailable"} for name in ("Cargo", "UV", "PNPM", "Mise")},
        )
        for key, result in results.items():
            reporting.render(
                f"{key} / {result['state']}",
                result["seconds"],
                result.get("cache"),
                phases={"Resource Wait": result["wait_seconds"], **result.get("phases", {})},
            )
            if result.get("peak_memory_bytes") is not None:
                emit(key, f"Memory / Peak={result['peak_memory_bytes'] / GIB:.2f} GiB")
        if output:
            (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")


def schedule(jobs: list[dict], cpu: int, memory: int, limits: dict, execute) -> dict:
    pending = {job["key"]: job for job in jobs}
    results = {}
    active = {}
    ready_at = {}
    queued = time.monotonic()
    with concurrent.futures.ThreadPoolExecutor(max_workers=cpu) as pool:
        while pending or active:
            for key, job in list(pending.items()):
                if any(results.get(need, {}).get("state") in ("failed", "blocked") for need in job["needs"]):
                    results[key] = {
                        "state": "blocked",
                        "seconds": 0,
                        "wait_seconds": time.monotonic() - queued,
                        "needs": job["needs"],
                        "cache": None,
                    }
                    emit(key, "blocked by failed prerequisite")
                    del pending[key]
            ready = [(key, job) for key, job in pending.items() if all(need in results for need in job["needs"])]
            ready.sort(key=lambda item: not item[1].get("id", "").startswith("build"))
            for key, _ in ready:
                ready_at.setdefault(key, time.monotonic())
            platforms = {job.get("execution_platform", job["executor"]) for job in jobs}
            slots = min(cpu, 2 * len(platforms))
            for key, job in ready:
                if len(active) >= slots:
                    break
                platform = job.get("execution_platform", job["executor"])
                if sum(item[4].get("execution_platform", item[3]) == platform for item in active.values()) >= 2:
                    continue
                assigned_cpu = max(1, cpu // slots)
                assigned_memory = memory // slots
                if job["executor"] == "docker":
                    assigned_cpu = min(assigned_cpu, limits["docker"][0])
                    assigned_memory = min(assigned_memory, limits["docker"][1] // 2)
                job["wait_seconds"] = time.monotonic() - ready_at[key]
                job["started"] = time.monotonic()
                emit(f"{platform}/{key}", f"running; shared CPU budget={cpu}; reserved RAM={assigned_memory / GIB:.2f} GiB")
                future = pool.submit(execute, job, assigned_cpu, assigned_memory)
                active[future] = (key, assigned_cpu, assigned_memory, job["executor"], job)
                del pending[key]
            if BUDGET:
                try:
                    BUDGET.update([item[4] for item in active.values()], list(pending.values()))
                except (OSError, SimulationError, subprocess.SubprocessError) as error:
                    BUDGET.cancel()
                    results["resource_budget"] = {
                        "state": "failed",
                        "seconds": time.monotonic() - queued,
                        "wait_seconds": 0,
                        "needs": [],
                        "cache": None,
                        "error": str(error),
                    }
                    for key, job in pending.items():
                        results[key] = {"state": "blocked", "seconds": 0, "wait_seconds": 0, "needs": job["needs"], "cache": None}
                    pending.clear()
            if not active:
                if pending:
                    raise SimulationError("selected graph has an unavailable dependency or cycle")
                break
            try:
                done, _ = concurrent.futures.wait(active, return_when=concurrent.futures.FIRST_COMPLETED)
            except KeyboardInterrupt:
                if BUDGET:
                    BUDGET.cancel()
                raise
            for future in done:
                key, _, _, _, job = active.pop(future)
                try:
                    result = future.result()
                except (OSError, ValueError, RuntimeError, KeyError, subprocess.SubprocessError) as error:
                    result = {"state": "failed", "seconds": time.monotonic() - job["started"], "cache": None, "error": str(error)}
                result.update(wait_seconds=job["wait_seconds"], needs=job["needs"])
                results[key] = result
                emit(key, str(result["state"]))
    return results


def summarize(results: dict, wall: float) -> dict:
    paths = {}
    remaining = dict(results)
    while remaining:
        ready = [(key, result) for key, result in remaining.items() if all(need in paths for need in result["needs"])]
        if not ready:
            break
        for key, result in ready:
            paths[key] = result["seconds"] + max((paths[need] for need in result["needs"]), default=0)
            del remaining[key]
    caches = [result["cache"] for result in results.values() if result.get("cache") is not None]
    cache = (
        {name: sum(item[name] for item in caches if item.get(name) is not None) for name in {key for item in caches for key in item}}
        if caches
        else "unavailable"
    )
    return {"wall_seconds": wall, "critical_path_seconds": max(paths.values(), default=0), "cache": cache, "jobs": results}


# === Private Functions === #


def emit(job: str, message: str) -> None:
    secrets = [
        value
        for key, value in os.environ.items()
        if any(word in key.upper() for word in ("TOKEN", "PASSWORD", "SECRET", "CREDENTIAL")) and len(value) >= 6
    ]
    for secret in secrets:
        message = message.replace(secret, "[redacted]")
    with PRINT:
        print(f"[{job}] {message}", flush=True)


def available_memory() -> int:
    if os.name == "nt":

        class Memory(ctypes.Structure):
            _fields_ = [
                ("length", ctypes.c_ulong),
                ("load", ctypes.c_ulong),
                *[
                    (name, ctypes.c_ulonglong)
                    for name in ("total", "available", "page_total", "page_available", "virtual_total", "virtual_available", "extended")
                ],
            ]

        memory = Memory()
        memory.length = ctypes.sizeof(memory)
        if not ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(memory)):
            raise SimulationError("cannot determine available host RAM")
        return memory.available
    return os.sysconf("SC_AVPHYS_PAGES") * os.sysconf("SC_PAGE_SIZE")


def preflight(executors: dict, requested_cpu: int | None, requested_memory: float | None) -> tuple[int, int, dict]:
    cpu, memory = os.process_cpu_count() or 1, available_memory()
    if requested_cpu is not None:
        if requested_cpu < 1 or requested_cpu > cpu:
            raise SimulationError("CPU limit must be positive and no larger than available host CPUs")
        cpu = requested_cpu
    if requested_memory is not None:
        if requested_memory <= 0 or requested_memory * GIB > memory:
            raise SimulationError("RAM limit must be positive and no larger than available host RAM")
        memory = int(requested_memory * GIB)
    limits = {}
    for platform, executor in executors.items():
        if executor == "native":
            if platform != "windows" or os.name != "nt" or not shutil.which("mise") or not shutil.which("pwsh"):
                raise SimulationError(
                    f"required native {platform} executor unavailable; install mise and its native shell or select an available executor"
                )
        elif platform != "linux":
            raise SimulationError("Docker supports Linux only; a native Windows host is required for Windows")
        elif "docker" not in limits:
            command = subprocess.run(["docker", "info", "--format", "{" + "{json .}" + "}"], capture_output=True, text=True, check=True)
            info = json.loads(command.stdout)
            if info.get("OSType") != "linux":
                raise SimulationError("required Docker Linux engine unavailable; start Docker Desktop with its WSL Linux backend")
            limits["docker"] = (int(info["NCPU"]), int(info["MemTotal"]))
    return cpu, memory, limits


def stage(root: Path, destination: Path) -> None:
    git = subprocess.run(["git", "-C", str(root), "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=False)
    if git.returncode == 0:
        subprocess.run(["git", "clone", "--quiet", "--no-checkout", "--no-hardlinks", "--local", str(root), str(destination)], check=True)
        for key, default in (("core.autocrlf", "false"), ("core.eol", "native")):
            setting = subprocess.run(["git", "-C", str(root), "config", "--get", key], capture_output=True, text=True, check=False)
            if setting.returncode not in (0, 1):
                raise SimulationError(f"could not read candidate Git setting: {key}")
            value = setting.stdout.strip() if setting.returncode == 0 else default
            subprocess.run(["git", "-C", str(destination), "config", "--local", key, value], check=True)
        index = subprocess.run(["git", "-C", str(root), "ls-files", "--stage", "-z"], capture_output=True, check=True).stdout
        subprocess.run(["git", "-C", str(destination), "read-tree", "--empty"], check=True)
        subprocess.run(["git", "-C", str(destination), "update-index", "-z", "--index-info"], input=index, check=True)
        listed = subprocess.run(
            ["git", "-C", str(root), "ls-files", "-z", "--cached", "--others", "--exclude-standard"], capture_output=True, check=True
        )
        files = [Path(os.fsdecode(name)) for name in listed.stdout.split(b"\0") if name]
    else:
        destination.mkdir()
        files = [
            path.relative_to(root)
            for path in root.rglob("*")
            if path.is_file() and not set(path.relative_to(root).parts) & {"target", ".cache", ".git", ".venv", "node_modules", ".env"}
        ]
    for relative in files:
        source = root / relative
        if not source.is_file() and not source.is_symlink():
            continue
        if not source.resolve().is_relative_to(root.resolve()):
            raise SimulationError(f"candidate file escapes repository: {relative}")
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        if source.is_symlink():
            target.symlink_to(os.readlink(source))
        else:
            shutil.copy2(source, target)


def export(root: Path, outputs: dict, archive: Path) -> None:
    archive.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive, "w") as transport:
        for output, patterns in outputs.items():
            for pattern in patterns:
                selected = glob.glob(pattern.rstrip("/"), root_dir=root, recursive=True)
                if not selected:
                    raise SimulationError(f"required output missing: {pattern}")
                for name in selected:
                    path = root / name
                    for file in [path] if path.is_file() else [path, *path.rglob("*")]:
                        if not file.resolve().is_relative_to(root.resolve()) or file.is_symlink():
                            raise SimulationError("artifact escapes job workspace")
                        if file.is_file() or file.is_dir():
                            transport.add(file, arcname=f"{output}/{file.relative_to(root).as_posix()}", recursive=False)


def transport(archive: Path, destination: Path) -> None:
    modes = {}
    directories = []
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(archive, destination.with_name(destination.name + ".tar"))
    with tarfile.open(archive) as source:
        for member in source:
            path = destination / member.name
            if (
                not (member.isfile() or member.isdir())
                or not relative_path(member.name)
                or not path.resolve().is_relative_to(destination.resolve())
            ):
                raise SimulationError("unsafe local artifact")
            modes[member.name] = {"type": "file" if member.isfile() else "directory", "mode": member.mode & 0o777}
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
                directories.append((path, member.mode & 0o777))
                continue
            path.parent.mkdir(parents=True, exist_ok=True)
            data = source.extractfile(member)
            if data is None:
                raise SimulationError("artifact member has no file content")
            with data, path.open("wb") as target:
                shutil.copyfileobj(data, target)
            path.chmod(member.mode & 0o777)
    for path, mode in sorted(directories, key=lambda item: len(item[0].parts), reverse=True):
        path.chmod(mode)
    destination.with_name(destination.name + ".modes.json").write_text(json.dumps({"version": 1, "files": modes}), encoding="utf-8")


def relative_path(name: str) -> bool:
    return (
        bool(name)
        and not name.startswith("/")
        and not any(part in ("", ".", "..") for part in name.split("/"))
        and "\\" not in name
        and ":" not in name
    )


def artifact_modes(path: Path) -> dict:
    document = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(document, dict) or document.get("version") != 1 or not isinstance(document.get("files"), dict):
        raise SimulationError("invalid artifact mode metadata")
    result = {}
    for name, item in document["files"].items():
        if (
            not relative_path(name)
            or not isinstance(item, dict)
            or item.get("type") not in ("file", "directory")
            or type(item.get("mode")) is not int
            or not 0 <= item["mode"] <= 0o777
        ):
            raise SimulationError("unsafe artifact mode metadata")
        root = path.with_name(path.name.removesuffix(".modes.json"))
        entry = root / name
        if (
            entry.is_symlink()
            or not entry.resolve().is_relative_to(root.resolve())
            or not (entry.is_file() if item["type"] == "file" else entry.is_dir())
        ):
            raise SimulationError("artifact mode metadata does not match the retained artifact")
        result[name] = item["mode"]
    return result


def source_archive(workspace: Path, archive: Path, modes: dict) -> dict:
    indexed = subprocess.run(["git", "ls-files", "--stage", "-z"], cwd=workspace, capture_output=True, check=False)
    tracked = {}
    if indexed.returncode == 0:
        for entry in indexed.stdout.split(b"\0"):
            if entry:
                metadata, name = entry.split(b"\t", 1)
                tracked[os.fsdecode(name)] = int(metadata.split()[0], 8)
    records = {}
    with tarfile.open(archive, "w") as target:
        for path in workspace.rglob("*"):
            relative = path.relative_to(workspace).as_posix()
            member = target.gettarinfo(str(path), arcname=relative)
            if member.issym() or tracked.get(relative) == 0o120000:
                link = os.readlink(path) if path.is_symlink() else path.read_text(encoding="utf-8")
                if not (path.parent / link).resolve().is_relative_to(workspace.resolve()):
                    raise SimulationError("candidate symlink escapes repository")
                member.type, member.linkname, member.size = tarfile.SYMTYPE, link, 0
            member.mode = (
                modes.get(relative, tracked.get(relative, (0o755 if path.is_dir() else 0o644) if os.name == "nt" else member.mode)) & 0o777
            )
            if member.isfile():
                with path.open("rb") as source:
                    target.addfile(member, source)
            elif member.isdir() or member.issym():
                target.addfile(member)
            else:
                raise SimulationError("unsupported candidate file type")
            records[relative] = {
                "type": "file" if member.isfile() else "directory" if member.isdir() else "symlink",
                "mode": member.mode,
                "origin": "artifact"
                if relative in modes
                else "git"
                if relative in tracked
                else "windows-default"
                if os.name == "nt"
                else "filesystem",
            }
    return records


def run_job(job: dict, candidate: Path, output: Path, cache: Path, identity: str, token: str, profile: str, cpu: int, memory: int) -> dict:
    started = time.monotonic()
    if BUDGET and BUDGET.cancelled.is_set():
        raise SimulationError("simulation cancelled before job start")
    key = job["key"]
    result: dict = {"state": "failed", "seconds": 0, "cache": None}
    with tempfile.TemporaryDirectory(prefix="stkr-ci-job-") as temporary:
        workspace = Path(temporary) / "workspace"
        shutil.copytree(candidate, workspace, symlinks=True)
        transferred_modes = {}
        transfer_started = time.monotonic()
        for reference in job["inputs"]:
            producer, name = reference.split("#", 1)
            candidates = [source for source in PLAN["jobs"] if source["id"] == producer and source["key"] in job["needs"]]
            if not candidates:
                raise SimulationError(f"missing artifact producer {reference}")
            for source in candidates:
                artifact = output / "artifacts" / source["key"] / name
                if not artifact.is_dir():
                    raise SimulationError(f"missing input {reference}")
                shutil.copytree(artifact, workspace, dirs_exist_ok=True)
                metadata = artifact.parent.with_name(artifact.parent.name + ".modes.json")
                for relative, mode in artifact_modes(metadata).items():
                    if relative.startswith(name + "/"):
                        transferred_modes[relative.removeprefix(name + "/")] = mode
        transfer_seconds = time.monotonic() - transfer_started
        native = job.get("execution_platform", job["platform"] if job["platform"] != "any" else "linux")
        label = f"{native}/{key}"
        target = f"{native}-x86_64" if job["platform"] != "any" else "any"
        graph_profile = "bench" if job["id"] == "build:rust:bench" or job["id"].startswith("bench:") else profile

        def replace(text):
            return (
                text.replace("$" + "{" + "{ env.PROFILE }" + "}", graph_profile)
                .replace("$env:PROFILE", graph_profile)
                .replace("$PROFILE", graph_profile)
            )

        outputs = {
            name: [path.replace("{profile}", graph_profile).replace("{target}", target) for path in patterns]
            for name, patterns in job["outputs"].items()
        }
        environment = {
            name: value
            for name, value in os.environ.items()
            if not any(word in name.upper() for word in ("TOKEN", "PASSWORD", "SECRET", "CREDENTIAL"))
            and name not in ("UV_PROJECT_ENVIRONMENT", "RUSTC_WRAPPER", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_TARGET_DIR")
            and not name.startswith(("STKR_SCCACHE_", "SCCACHE_", "GITHUB_", "CI_", "CARGO_PROFILE_"))
        }
        environment.update(
            PROFILE=graph_profile,
            RELEASE_PROFILE=graph_profile,
            CARGO_BUILD_JOBS=str(cpu),
            CMAKE_BUILD_PARALLEL_LEVEL=str(cpu),
            CARGO_INCREMENTAL="0",
            MISE_YES="1",
            MISE_LOCKED="1",
            MISE_EXPERIMENTAL="1",
            GIT_TERMINAL_PROMPT="0",
            STKR_CI_SIMULATE="1",
            STKR_SCCACHE_BACKEND="disk",
            STKR_SCCACHE_MODE="READ_WRITE",
        )
        if job.get("tools"):
            environment["MISE_ENABLE_TOOLS"] = job["tools"]
        if job.get("skip"):
            environment["MISE_TASK_SKIP"] = job["skip"]
        commands = [job["setup"], *job["bootstrap"], "mise trust --yes", "mise install --yes"]
        for step in job["commands"]:
            command = replace(step["run"])
            if step.get("cached"):
                shell = step.get("shell") or ("pwsh" if native == "windows" else "bash")
                command = f"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.build --internal-script {command.encode().hex()} --shell {shell}"
            commands.append(command)
        with (output / f"{key}.log").open("w", encoding="utf-8") as log:
            if job["executor"] == "docker":
                status = docker(
                    job, workspace, output, identity, token, environment, commands, outputs, cpu, memory, log, result, transferred_modes
                )
            else:
                local = cache / native
                local.mkdir(parents=True, exist_ok=True)
                environment.update(
                    MISE_DATA_DIR=str(local / "mise"),
                    MISE_CACHE_DIR=str(local / "mise-cache"),
                    MISE_DOWNLOADS_DIR=str(local / "mise-downloads"),
                    CARGO_HOME=str(local / "cargo"),
                    UV_CACHE_DIR=str(local / "uv"),
                    pnpm_config_store_dir=str(local / "pnpm"),
                    STKR_SCCACHE_DIR=str(local / "sccache"),
                )
                evidence(output, job, commands, environment, cpu, memory)
                windows = os.name == "nt"
                script = workspace.with_suffix(".ps1" if windows else ".sh")
                for name, suffix in [("GITHUB_ENV", ".env"), ("GITHUB_PATH", ".path")]:
                    scratch = workspace.with_suffix(suffix)
                    scratch.write_text("", encoding="utf-8")
                    environment[name] = str(scratch)
                lines = ["$ErrorActionPreference='Stop'" if windows else "set -eo pipefail"]
                for index, command in enumerate(commands):
                    label = f"Command {index + 1}/{len(commands)}: {command.splitlines()[0]}".replace("'", "''" if windows else "'\"'\"'")
                    if windows:
                        lines.extend(
                            [
                                f"Write-Output '{label}'",
                                "$global:LASTEXITCODE=0",
                                command,
                                "if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }",
                            ]
                        )
                    else:
                        lines.extend([f"printf '%s\\n' '{label}'", command])
                    lines.extend(step_environment.commands(windows, sys.executable, str(Path(__file__).with_name("environment.py"))))
                script.write_text("\n".join(lines) + "\n", encoding="utf-8")
                shell = ["pwsh", "-NoProfile", "-NonInteractive", "-File", str(script)] if windows else ["bash", str(script)]
                try:
                    status = stream(shell, workspace, environment, label, log, job["minutes"] * 60 - (time.monotonic() - started), result)
                    if status == 0:
                        transfer_started = time.monotonic()
                        archive = workspace / "target/ci/simulation-output.tar"
                        export(workspace, outputs, archive)
                        transport(archive, output / "artifacts" / key)
                        transfer_seconds += time.monotonic() - transfer_started
                finally:
                    preserve_reports(workspace, output, key, log)
        result.setdefault("phases", {})["Transfer"] = transfer_seconds
        result.update(state="passed" if status == 0 else "failed", seconds=time.monotonic() - started)
    return result


def docker(
    job: dict,
    workspace: Path,
    output: Path,
    identity: str,
    token: str,
    environment: dict,
    commands: list[str],
    outputs: dict,
    cpu: int,
    memory: int,
    log,
    result: dict,
    modes: dict | None = None,
) -> int:
    name = f"stkr-ci-{token}-{job['key'].replace('_', '-')}"
    volume = name + "-work"
    image_identity = hashlib.sha256(job["image"].encode()).hexdigest()[:12]
    cache = f"stkr-ci-{identity}-linux-{image_identity}-cache"
    script = workspace.with_suffix(".sh")
    lines = [
        "set -eo pipefail",
        ': > "$GITHUB_ENV"; : > "$GITHUB_PATH"',
        "trap 'if [ -r /sys/fs/cgroup/memory.peak ]; then printf \"STKR_PEAK_MEMORY \"; cat /sys/fs/cgroup/memory.peak; fi' EXIT",
    ]
    for index, command in enumerate(commands):
        lines.extend([f"printf '%s\n' 'Command {index + 1}/{len(commands)}'", command])
        lines.extend(step_environment.commands(False, "python3", "/ci-environment.py"))

    export_argument = json.dumps(outputs).replace("'", "'\"'\"'")
    lines.append(
        f"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.simulate --export '{export_argument}'",
    )
    script.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    child = {
        key: value
        for key, value in environment.items()
        if key
        in (
            "PROFILE",
            "RELEASE_PROFILE",
            "CARGO_BUILD_JOBS",
            "CMAKE_BUILD_PARALLEL_LEVEL",
            "CARGO_INCREMENTAL",
            "MISE_YES",
            "MISE_LOCKED",
            "MISE_EXPERIMENTAL",
            "MISE_ENABLE_TOOLS",
            "MISE_TASK_SKIP",
            "STKR_CI_SIMULATE",
            "STKR_SCCACHE_BACKEND",
            "STKR_SCCACHE_MODE",
        )
    }
    child.update(
        MISE_DATA_DIR="/cache/mise",
        MISE_CACHE_DIR="/cache/mise-cache",
        MISE_DOWNLOADS_DIR="/cache/downloads",
        CARGO_HOME="/cache/cargo",
        UV_CACHE_DIR="/cache/uv",
        pnpm_config_store_dir="/cache/pnpm",
        STKR_SCCACHE_DIR="/cache/sccache",
        GITHUB_ENV="/ci-environment",
        GITHUB_PATH="/ci-path",
    )
    evidence(output, job, commands, child, cpu, memory)
    create = [
        "docker",
        "create",
        "--name",
        name,
        "--cpus",
        str(cpu),
        "--memory",
        str(memory),
        "--mount",
        f"type=volume,source={volume},target=/workspace",
        "--mount",
        f"type=volume,source={cache},target=/cache",
        "--workdir",
        "/workspace",
    ]
    for key, value in child.items():
        create += ["--env", f"{key}={value}"]
    create += [job["image"], "bash", "/ci-simulation.sh"]
    created = False
    try:
        subprocess.run(create, check=True, capture_output=True, text=True)
        created = True
        if BUDGET:
            BUDGET.register(job["key"], name)
        archive = workspace.with_suffix(".tar")
        records = source_archive(workspace, archive, modes or {})
        (output / f"{job['key']}.source-modes.json").write_text(json.dumps(records), encoding="utf-8")
        with archive.open("rb") as source:
            subprocess.run(["docker", "cp", "-", name + ":/workspace"], stdin=source, check=True, capture_output=True)
        subprocess.run(["docker", "cp", str(script), name + ":/ci-simulation.sh"], check=True, capture_output=True)
        subprocess.run(
            ["docker", "cp", str(Path(__file__).with_name("environment.py")), name + ":/ci-environment.py"], check=True, capture_output=True
        )
        status = stream(["docker", "start", "--attach", name], workspace, None, f"linux/{job['key']}", log, job["minutes"] * 60, result)
        if status == 0:
            archive = workspace / "simulation-output.tar"
            subprocess.run(
                ["docker", "cp", name + ":/workspace/target/ci/simulation-output.tar", str(archive)], check=True, capture_output=True
            )
            transport(archive, output / "artifacts" / job["key"])
        return status
    finally:
        if created:
            preserve_reports(workspace, output, job["key"], log, name)
            if BUDGET:
                BUDGET.unregister(job["key"])
            subprocess.run(["docker", "rm", "--force", name], check=False, capture_output=True)
            subprocess.run(["docker", "volume", "rm", volume], check=False, capture_output=True)


def preserve_reports(workspace: Path, output: Path, key: str, log, container: str | None = None) -> None:
    archive = workspace.with_suffix(".reports.tar")
    try:
        if container:
            with archive.open("wb") as target:
                subprocess.run(
                    ["docker", "cp", container + ":/workspace/target/tests", "-"],
                    stdout=target,
                    stderr=subprocess.PIPE,
                    check=True,
                    timeout=30,
                )
        else:
            reports = workspace / "target/tests"
            if not reports.exists():
                return
            with tarfile.open(archive, "w") as target:
                for path in [reports, *reports.rglob("*")]:
                    if path.is_symlink() or not path.resolve().is_relative_to(reports.resolve()):
                        raise SimulationError("diagnostic escapes test reports")
                    if path.is_file() or path.is_dir():
                        target.add(path, arcname=path.relative_to(reports.parent).as_posix(), recursive=False)
        transport(archive, output / "diagnostics" / key)
    except (OSError, tarfile.TarError, subprocess.SubprocessError, SimulationError) as error:
        try:
            log.write(f"Diagnostic collection unavailable: {error}\n")
            log.flush()
        except OSError:
            pass


def evidence(output: Path, job: dict, commands: list[str], environment: dict, cpu: int, memory: int) -> None:
    selected = {
        name: value
        for name, value in environment.items()
        if name
        in (
            "PROFILE",
            "RELEASE_PROFILE",
            "CARGO_BUILD_JOBS",
            "CMAKE_BUILD_PARALLEL_LEVEL",
            "CARGO_INCREMENTAL",
            "CARGO_HOME",
            "UV_CACHE_DIR",
            "pnpm_config_store_dir",
        )
        or name.startswith(("MISE_", "STKR_SCCACHE_", "STKR_CI_SIMULATE"))
    }
    recorded = {"job": job, "commands": commands, "environment": selected, "cpu": cpu, "memory_bytes": memory}
    (output / f"{job['key']}.execution.json").write_text(json.dumps(recorded, indent=2) + "\n", encoding="utf-8")


def stream(command: list[str], cwd: Path, environment: dict | None, key: str, log, timeout: float, result: dict) -> int:
    if timeout <= 0:
        raise SimulationError("job deadline exceeded")
    flags = getattr(subprocess, "CREATE_NO_WINDOW", 0)
    if NATIVE_LIMIT and command[0] != "docker":
        flags |= 4
    process = subprocess.Popen(
        command,
        cwd=cwd,
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        errors="replace",
        creationflags=flags,
        start_new_session=os.name != "nt",
    )
    owned = None
    if NATIVE_LIMIT and command[0] != "docker":
        try:
            NATIVE_LIMIT.attach(process, resume=False)
            owned = NativeLimit(None, None)
            owned.attach(process)
        except BaseException:
            if owned:
                owned.close()
            process.kill()
            process.wait()
            raise

    def drain():
        assert process.stdout is not None
        for line in process.stdout:
            for secret_name, secret in os.environ.items():
                if any(word in secret_name.upper() for word in ("TOKEN", "PASSWORD", "SECRET", "CREDENTIAL")) and len(secret) >= 6:
                    line = line.replace(secret, "[redacted]")
            log.write(line)
            log.flush()
            emit(key, line.rstrip())
            if line.startswith("STKR_PEAK_MEMORY "):
                result["peak_memory_bytes"] = int(line.removeprefix("STKR_PEAK_MEMORY ").strip())
            if line.startswith("STKR_BUILD_STATISTICS "):
                information = json.loads(line.removeprefix("STKR_BUILD_STATISTICS "))
                metrics = {name: value for name, value in information["statistics"].items() if type(value) is int}
                result["phases"] = information["phases"]
                if metrics:
                    result["cache"] = (
                        {name: result.get("cache", {}).get(name, 0) + value for name, value in metrics.items()}
                        if result.get("cache")
                        else metrics
                    )

    reader = threading.Thread(target=drain, daemon=True)
    reader.start()
    try:
        return process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        if os.name == "nt":
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], capture_output=True, check=False)
        else:
            import signal

            os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SimulationError("job deadline exceeded") from None
    finally:
        if owned:
            owned.close()
        reader.join(timeout=5)
        if reader.is_alive():
            raise SimulationError("owned command retained its output stream after termination")


# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
