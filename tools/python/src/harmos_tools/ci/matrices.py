# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Declare CI Check and Test Matrices"""

# === Imports === #

# Standard Library Imports

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

CHECKS = [
    {
        "language": "repository",
        "label": "Repository",
        "runner": "ubuntu-24.04",
        "platform": "unix",
        "arguments": "--repository",
        "tools": "python,uv,taplo,go,go:github.com/rhysd/actionlint/cmd/actionlint,aqua:koalaman/shellcheck,aqua:bufbuild/buf",
        "cache": "",
    },
    {
        "language": "rust",
        "label": "Rust (unix)",
        "platform": "unix",
        "runner": "ubuntu-24.04",
        "arguments": "--language rust",
        "tools": "python,rust,uv,sccache,aqua:nextest-rs/nextest/cargo-nextest,github:bnjbvr/cargo-machete,taplo,go,go:github.com/rhysd/actionlint/cmd/actionlint,aqua:koalaman/shellcheck,aqua:bufbuild/buf",
        "cache": "disk",
    },
    {
        "language": "csharp",
        "label": "C# (unix)",
        "platform": "unix",
        "runner": "ubuntu-24.04",
        "arguments": "--language csharp",
        "tools": "python,uv,dotnet",
        "cache": "",
    },
    {
        "language": "rust",
        "label": "Rust (windows)",
        "platform": "windows",
        "runner": "windows-2022",
        "arguments": "--language rust",
        "tools": "python,rust,uv,sccache,aqua:nextest-rs/nextest/cargo-nextest,github:bnjbvr/cargo-machete,taplo,go,go:github.com/rhysd/actionlint/cmd/actionlint,aqua:koalaman/shellcheck,aqua:bufbuild/buf",
        "cache": "disk",
    },
    {
        "language": "csharp",
        "label": "C# (windows)",
        "platform": "windows",
        "runner": "windows-2022",
        "arguments": "--language csharp",
        "tools": "python,uv,dotnet",
        "cache": "",
    },
]

TESTS = [
    {
        "language": "python",
        "platform": "unix",
        "runner": "ubuntu-24.04",
        "minutes": 120,
        "layer": "integration",
        "label": "SDK / Python",
        "arguments": "--package harmos-sdk --target integration",
    },
    {
        "language": "python",
        "platform": "windows",
        "runner": "windows-2022",
        "minutes": 120,
        "layer": "integration",
        "label": "SDK / Python",
        "arguments": "--package harmos-sdk --target integration",
    },
    {
        "language": "rust",
        "platform": "unix",
        "runner": "ubuntu-24.04",
        "minutes": 120,
        "layer": "integration",
        "label": "Integration",
        "arguments": "--layer integration",
    },
    {
        "language": "rust",
        "platform": "unix",
        "runner": "ubuntu-24.04",
        "minutes": 120,
        "layer": "acceptance",
        "label": "Acceptance",
        "arguments": "--layer acceptance",
    },
    {
        "language": "rust",
        "platform": "unix",
        "runner": "ubuntu-24.04",
        "minutes": 120,
        "layer": "system",
        "label": "System",
        "arguments": "--layer system",
    },
    {
        "language": "rust",
        "platform": "windows",
        "runner": "windows-2022",
        "minutes": 120,
        "layer": "integration",
        "label": "Integration",
        "arguments": "--layer integration",
    },
    {
        "language": "rust",
        "platform": "windows",
        "runner": "windows-2022",
        "minutes": 120,
        "layer": "acceptance",
        "label": "Acceptance",
        "arguments": "--layer acceptance",
    },
    {
        "language": "rust",
        "platform": "windows",
        "runner": "windows-2022",
        "minutes": 120,
        "layer": "system",
        "label": "System",
        "arguments": "--layer system",
    },
]

# === Types === #

# === Classes === #

# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
