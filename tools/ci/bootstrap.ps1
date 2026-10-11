# Prepare The Native Windows Toolchain
$ErrorActionPreference = 'Stop'
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
