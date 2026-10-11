#!/bin/sh
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

# Install The Latest Stable Mise Before Repository Tasks.

# === Imports === #

# === Constants === #

# === Functions === #

# === Entry Point === #
set -eu
mkdir -p "${TMPDIR:-/tmp}"
case "$(uname -s)" in
    Linux) mise_system=linux ;;
    Darwin) mise_system=macos ;;
    *) exit 1 ;;
esac
case "$(uname -m)" in
    x86_64|amd64) mise_arch=x64 ;;
    aarch64|arm64) mise_arch=arm64 ;;
    *) exit 1 ;;
esac
mise_index=$(curl -fsSL https://mise.jdx.dev/releases.tsv)
mise_release=$(printf '%s\n' "$mise_index" | awk -v now="$(date +%s)" '
    NF != 2 || $1 !~ /^v[0-9]+\.[0-9]+\.[0-9]+$/ || $2 !~ /^[0-9]+$/ { invalid = 1; next }
    $2 <= now {
        split(substr($1, 2), parts, ".")
        if (!selected || parts[1]+0 > year || (parts[1]+0 == year && parts[2]+0 > month) || (parts[1]+0 == year && parts[2]+0 == month && parts[3]+0 > patch)) {
            selected = $1; year = parts[1]+0; month = parts[2]+0; patch = parts[3]+0
        }
    }
    END { if (invalid || !selected) exit 1; print selected }
')
mise_asset="mise-${mise_release}-${mise_system}-${mise_arch}"
mise_base="https://github.com/jdx/mise/releases/download/${mise_release}"
mkdir -p .tmp/mise/bin
curl -fsSL "$mise_base/$mise_asset" -o .tmp/mise/bin/mise
curl -fsSL "$mise_base/SHASUMS256.txt" -o .tmp/mise/SHASUMS256.txt
mise_expected=$(awk -v asset="./$mise_asset" '$2 == asset { print $1 }' .tmp/mise/SHASUMS256.txt)
if command -v sha256sum >/dev/null 2>&1; then
    mise_actual=$(sha256sum .tmp/mise/bin/mise | awk '{print $1}')
else
    mise_actual=$(shasum -a 256 .tmp/mise/bin/mise | awk '{print $1}')
fi
test -n "$mise_expected" && test "$mise_expected" = "$mise_actual"
chmod +x .tmp/mise/bin/mise
.tmp/mise/bin/mise --version
