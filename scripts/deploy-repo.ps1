param(
    [Parameter(Mandatory = $true)]
    [string] $Version,
    [Parameter(Mandatory = $true)]
    [string] $RepoRoot,
[Alias('rb')]
[switch] $Rebuild
)

$ErrorActionPreference = 'Stop'

Write-Host "=== Syncing repository versions to $Version ==="
& "$RepoRoot\scripts\version-sync.ps1" -Version $Version -RepoRoot $RepoRoot

$targetTriple = 'x86_64-pc-windows-msvc'
$distDir = Join-Path $RepoRoot 'dist\npm'
$stageDir = Join-Path $distDir "codex-$Version"
$vendorSrc = Join-Path $distDir "vendor-src-$Version"
$packTgz = Join-Path $distDir "openai-codex-$Version.tgz"

if ($Rebuild) {
    Write-Host "=== Full rebuild requested (-rb): cleaning outputs ==="
    if (Test-Path $distDir) { Remove-Item $distDir -Recurse -Force }
    Push-Location "$RepoRoot\codex-rs"
    cargo clean
    Pop-Location
    Write-Host "=== Removing previously installed global codex (best-effort) ==="
    npm uninstall -g @openai/codex | Out-Null
}

if (-not (Test-Path $distDir)) { New-Item -ItemType Directory -Path $distDir | Out-Null }

Write-Host "=== Building codex CLI (release) ==="
Push-Location "$RepoRoot\codex-rs"
cargo build --release -p codex-cli
Pop-Location

Write-Host "=== Preparing vendor payload ==="
if (Test-Path $vendorSrc) { Remove-Item $vendorSrc -Recurse -Force }
$codexDir = Join-Path $vendorSrc "$targetTriple\codex"
$pathDir = Join-Path $vendorSrc "$targetTriple\path"
New-Item -ItemType Directory -Path $codexDir, $pathDir | Out-Null
Copy-Item "$RepoRoot\codex-rs\target\release\codex.exe" -Destination (Join-Path $codexDir 'codex.exe') -Force

Write-Host "=== Downloading ripgrep binary ==="
$rgUrl = 'https://github.com/BurntSushi/ripgrep/releases/download/14.1.1/ripgrep-14.1.1-x86_64-pc-windows-msvc.zip'
$rgArchive = Join-Path $env:TEMP "rg-$([guid]::NewGuid()).zip"
$rgExtract = Join-Path $env:TEMP "rg-extract-$([guid]::NewGuid())"
Invoke-WebRequest -Uri $rgUrl -OutFile $rgArchive
New-Item -ItemType Directory -Path $rgExtract | Out-Null
Expand-Archive -LiteralPath $rgArchive -DestinationPath $rgExtract -Force
$rgBin = Join-Path $rgExtract 'ripgrep-14.1.1-x86_64-pc-windows-msvc\rg.exe'
if (-not (Test-Path $rgBin)) { throw "ripgrep binary not found at $rgBin" }
Copy-Item $rgBin -Destination (Join-Path $pathDir 'rg.exe') -Force
Remove-Item $rgArchive -Force
Remove-Item $rgExtract -Recurse -Force

Write-Host "=== Staging npm package ==="
if (Test-Path $stageDir) { Remove-Item $stageDir -Recurse -Force }
if (Test-Path $packTgz) { Remove-Item $packTgz -Force }

$python = 'python'
& $python "$RepoRoot\codex-cli\scripts\build_npm_package.py" `
    --package codex `
    --release-version $Version `
    --staging-dir $stageDir `
    --vendor-src $vendorSrc

Write-Host "=== Packing npm tarball ==="
Push-Location $stageDir
$packOutput = npm pack --json --pack-destination $distDir
Pop-Location

if (-not (Test-Path $packTgz)) {
    throw "Unable to locate generated npm tarball at $packTgz."
}

Write-Host "=== Installing $packTgz globally ==="
npm install -g $packTgz

Write-Host "=== deploy finished ==="
