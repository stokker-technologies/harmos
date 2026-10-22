# Prepare The Native Windows Toolchain
param([switch]$ExportGitHubEnvironment)

$ErrorActionPreference = 'Stop'
$before = @{}
Get-ChildItem Env: | ForEach-Object { $before[$_.Name] = $_.Value }
if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    $vswhere = "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe"
    if (-not (Test-Path $vswhere)) { throw 'Install Visual Studio Build Tools With The C++ Workload And Windows SDK' }
    $installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $installation) { throw 'The Native C++ Workload Is Missing' }
    $variables = & cmd.exe /s /c "call `"$installation/Common7/Tools/VsDevCmd.bat`" -arch=amd64 -host_arch=amd64 >nul && set"
    if ($LASTEXITCODE -ne 0) { throw 'Visual Studio Environment Initialization Failed' }
    foreach ($line in $variables) {
        if ($line -match '^([^=]+)=(.*)$') { [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], 'Process') }
    }
}
if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) { throw 'The Native C++ Compiler Is Unavailable' }
if (-not $env:WindowsSdkDir) { throw 'The Windows SDK Is Unavailable' }
Write-Host "Windows SDK: $env:WindowsSDKVersion"

if ($ExportGitHubEnvironment) {
    if (-not $env:GITHUB_ENV -or -not $env:GITHUB_PATH) { throw 'GitHub Environment Files Are Unavailable' }
    Get-ChildItem Env: | Where-Object { $_.Name -notmatch '^(GITHUB_|RUNNER_|PATH$)' -and $before[$_.Name] -ne $_.Value } |
        ForEach-Object { "$($_.Name)=$($_.Value)" } | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding utf8
    $env:PATH.Substring(0, $env:PATH.Length - $before['PATH'].Length).TrimEnd(';') -split ';' |
        Out-File -FilePath $env:GITHUB_PATH -Append -Encoding utf8
}
