param(
    [Parameter(Mandatory = $true)]
    [string] $Version,
    [Parameter(Mandatory = $true)]
    [string] $RepoRoot
)

$ErrorActionPreference = 'Stop'
$enc = [Text.UTF8Encoding]::new($false)

$jsons = @(
    'codex-cli/package.json',
    'sdk/typescript/package.json',
    'codex-rs/responses-api-proxy/npm/package.json',
    'shell-tool-mcp/package.json'
)

Write-Host "Version sync: $Version"
Write-Host "Repo root: $RepoRoot"

foreach ($rel in $jsons) {
    $p = Join-Path $RepoRoot $rel
    Write-Host "Updating $p"
    $obj = Get-Content $p -Raw | ConvertFrom-Json
    $obj.version = $Version
    $json = $obj | ConvertTo-Json -Depth 50
    [IO.File]::WriteAllBytes($p, $enc.GetBytes($json))
}

$tomlPath = Join-Path $RepoRoot 'codex-rs/Cargo.toml'
$lines = Get-Content $tomlPath
$start = $lines.IndexOf('[workspace.package]')
if ($start -lt 0) { throw 'workspace.package section not found' }
$found = $false
for ($i = $start + 1; $i -lt $lines.Count; $i++) {
    $trim = $lines[$i].TrimStart()
    if ($trim.StartsWith('[')) { break }
    if ($trim.StartsWith('version')) {
        $lines[$i] = 'version = "' + $Version + '"'
        $found = $true
        break
    }
}
if (-not $found) { throw 'workspace.package version not found' }
$tomlText = $lines -join [Environment]::NewLine
[IO.File]::WriteAllBytes($tomlPath, $enc.GetBytes($tomlText))
