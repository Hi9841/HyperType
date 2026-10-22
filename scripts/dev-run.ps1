<#
.SYNOPSIS
  Run a local build of the native app in place of the installed HyperType.

.DESCRIPTION
  Stops every running hypertype.exe, copies the build to
  %LOCALAPPDATA%\HyperType-dev, and starts it from there. The copy matters:
  an exe started from a folder with a Low integrity label runs at Low
  integrity and cannot write %APPDATA%\HyperType.

  With -Restore, it stops the dev build and starts the installed one again.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/dev-run.ps1
  powershell -ExecutionPolicy Bypass -File scripts/dev-run.ps1 -Release -Minimized
  powershell -ExecutionPolicy Bypass -File scripts/dev-run.ps1 -Restore
#>
param(
  [switch]$Release,
  [switch]$Minimized,
  [switch]$Restore
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$devDir = Join-Path $env:LOCALAPPDATA "HyperType-dev"
$installed = Join-Path $env:LOCALAPPDATA "HyperType\hypertype.exe"

function Stop-HyperType {
  Get-Process hypertype -ErrorAction SilentlyContinue | ForEach-Object {
    Write-Host "Stopping hypertype.exe (PID $($_.Id), $($_.Path))"
    Stop-Process -Id $_.Id -Force
    $_.WaitForExit(5000) | Out-Null
  }
}

if ($Restore) {
  Stop-HyperType
  if (Test-Path $installed) {
    Start-Process $installed -ArgumentList "--minimized"
    Write-Host "Started installed HyperType: $installed"
  } else {
    Write-Host "No installed HyperType at $installed"
  }
  exit 0
}

$profileName = if ($Release) { "release" } else { "debug" }
$cargoArgs = @("build", "-p", "hypertype", "--locked")
if ($Release) { $cargoArgs += "--release" }
& cargo @cargoArgs
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

Stop-HyperType
New-Item -ItemType Directory -Force $devDir | Out-Null
$exe = Join-Path $devDir "hypertype.exe"
Copy-Item (Join-Path $root "target\$profileName\hypertype.exe") $exe -Force

$runArgs = @()
if ($Minimized) { $runArgs += "--minimized" }
if ($runArgs.Count -gt 0) {
  $proc = Start-Process $exe -ArgumentList $runArgs -PassThru
} else {
  $proc = Start-Process $exe -PassThru
}
Write-Host "Started dev build (PID $($proc.Id)): $exe"
