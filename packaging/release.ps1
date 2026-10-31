# Builds, signs, and publishes one HyperType release. Run the stages in order:
#
#   powershell -ExecutionPolicy Bypass -File packaging\release.ps1 -Notes notes.md -Build
#   powershell -ExecutionPolicy Bypass -File packaging\release.ps1 -Notes notes.md -SignInCi
#   powershell -ExecutionPolicy Bypass -File packaging\release.ps1 -Notes notes.md -Publish
#
# -Build     cargo build + makensis -> dist\HyperType_<version>_x64-setup.exe
# -SignInCi  uploads the installer to a draft release (a draft has no tag and
#            users do not see it), runs the manual "Sign release" workflow,
#            which signs with the key password stored as a repository secret,
#            then verifies the .sig here and writes dist\latest.json.
# -SignLocal signs here instead; asks for the key password (hidden prompt).
# -Publish   uploads latest.json and publishes the draft as the latest release.
#            From then on, installed apps see the update.
#
# Nothing else runs in CI: the installer is always built on this machine.

param(
  [Parameter(Mandatory = $true)][string]$Notes,
  [switch]$Build,
  [switch]$SignInCi,
  [switch]$SignLocal,
  [switch]$Publish
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = (Select-String -Path 'crates\app\Cargo.toml' -Pattern '^version = "(.+)"' |
  Select-Object -First 1).Matches[0].Groups[1].Value
$notesPath = (Resolve-Path $Notes).Path
$tag = "v$version"
$name = "HyperType_${version}_x64-setup.exe"
$dist = Join-Path $root 'dist'
$installer = Join-Path $dist $name
$latest = Join-Path $dist 'latest.json'
$url = "https://github.com/Hi9841/HyperType/releases/download/$tag/$name"
$signer = Join-Path $root 'target\release\hypertype-sign.exe'
Write-Host "HyperType $version" -ForegroundColor Cyan

function Assert-Ok([string]$what) {
  if ($LASTEXITCODE -ne 0) { throw "$what failed (exit $LASTEXITCODE)" }
}

if ($Build) {
  cargo build --release --locked -p hypertype -p hypertype-sign
  Assert-Ok 'cargo build'
  $exe = Join-Path $root 'target\release\hypertype.exe'
  $sizeKiB = [math]::Ceiling((Get-Item $exe).Length / 1KB)
  $makensis = Join-Path $env:LOCALAPPDATA 'tauri\NSIS\makensis.exe'
  if (-not (Test-Path $makensis)) { $makensis = (Get-Command makensis).Source }
  New-Item -ItemType Directory -Force $dist | Out-Null
  & $makensis /V2 /INPUTCHARSET UTF8 "/DVERSION=$version" "/DMAINBINARYSRCPATH=$exe" `
    "/DOUTFILE=$installer" "/DESTIMATEDSIZE=$sizeKiB" 'packaging\nsis\installer.nsi'
  Assert-Ok 'makensis'
}

if ($SignInCi) {
  $existing = gh release view $tag --json isDraft -q .isDraft 2>$null
  if ($LASTEXITCODE -ne 0) {
    gh release create $tag $installer --draft --title "HyperType $tag" --notes-file $notesPath
    Assert-Ok 'gh release create --draft'
  } elseif ($existing -eq 'true') {
    gh release upload $tag $installer --clobber
    Assert-Ok 'gh release upload'
  } else {
    throw "$tag is already published"
  }
  gh workflow run sign.yml -f "tag=$tag"
  Assert-Ok 'gh workflow run'
  Start-Sleep -Seconds 5
  $runId = gh run list --workflow sign.yml --limit 1 --json databaseId -q '.[0].databaseId'
  gh run watch $runId --exit-status
  Assert-Ok 'Sign release workflow'
  Remove-Item "$installer.sig" -ErrorAction SilentlyContinue
  gh release download $tag -p "$name.sig" -D $dist
  Assert-Ok 'gh release download'
  & $signer $installer --version $version --notes $notesPath --url $url --sig "$installer.sig"
  Assert-Ok 'signature check'
}

if ($SignLocal) {
  $secure = Read-Host 'Updater key password (~/.tauri/hypertype.key)' -AsSecureString
  $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
  try {
    $env:HYPERTYPE_SIGNING_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr)
    & $signer $installer --version $version --notes $notesPath --url $url
  } finally {
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr)
    Remove-Item Env:HYPERTYPE_SIGNING_PASSWORD -ErrorAction SilentlyContinue
  }
  Assert-Ok 'signing'
}

if ($Publish) {
  if (-not (Test-Path $latest)) { throw 'dist\latest.json is missing; sign first' }
  $draft = gh release view $tag --json isDraft -q .isDraft 2>$null
  if ($LASTEXITCODE -ne 0) {
    gh release create $tag $installer "$installer.sig" $latest --title "HyperType $tag" --notes-file $notesPath --latest
    Assert-Ok 'gh release create'
  } else {
    if ($draft -ne 'true') { throw "$tag is already published" }
    gh release upload $tag "$installer.sig" $latest --clobber
    Assert-Ok 'gh release upload'
    gh release edit $tag --draft=false --latest
    Assert-Ok 'gh release edit'
  }
}
Write-Host 'Done.' -ForegroundColor Green
