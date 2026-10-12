# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

# Install The Latest Stable Mise For Windows Service Accounts.

# === Imports === #

# === Constants === #

# === Functions === #

# === Entry Point === #
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$miseIndex = (Invoke-WebRequest 'https://mise.jdx.dev/releases.tsv' -UseBasicParsing).Content
$miseNow = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
$miseVersion = $null
foreach ($miseLine in ($miseIndex.Trim() -split '\r?\n')) {
    if ($miseLine -notmatch '^(v[0-9]+\.[0-9]+\.[0-9]+)\s+([0-9]+)$') { throw 'Invalid Mise Release Index' }
    $miseCandidate = [version]$Matches[1].Substring(1)
    $misePublished = [long]$Matches[2]
    if ($misePublished -le $miseNow -and ($null -eq $miseVersion -or $miseCandidate -gt $miseVersion)) {
        $miseVersion = $miseCandidate
    }
}
if ($null -eq $miseVersion) { throw 'Missing Stable Mise Release' }
$miseRelease = "v$miseVersion"
$miseArch = if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq 'Arm64') { 'arm64' } else { 'x64' }
$miseAsset = "mise-$miseRelease-windows-$miseArch.zip"
$miseBase = "https://github.com/jdx/mise/releases/download/$miseRelease"
New-Item -ItemType Directory -Force '.tmp/mise' | Out-Null
Invoke-WebRequest "$miseBase/$miseAsset" -UseBasicParsing -OutFile '.tmp/mise/mise.zip'
Invoke-WebRequest "$miseBase/SHASUMS256.txt" -UseBasicParsing -OutFile '.tmp/mise/SHASUMS256.txt'
$miseEntry = Get-Content '.tmp/mise/SHASUMS256.txt' | Where-Object { $_.EndsWith(" ./$miseAsset") }
if (-not $miseEntry) { throw 'Missing Mise Checksum' }
$miseExpected = ($miseEntry -split '\s+')[0]
$miseActual = (Get-FileHash '.tmp/mise/mise.zip' -Algorithm SHA256).Hash.ToLowerInvariant()
if ($miseActual -ne $miseExpected) { throw 'Mise Checksum Mismatch' }
Expand-Archive '.tmp/mise/mise.zip' -DestinationPath '.tmp/mise' -Force
$env:PATH = "$PWD/.tmp/mise/mise/bin;$env:PATH"
& mise --version
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
