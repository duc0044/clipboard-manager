$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$configPath = Join-Path $root "src-tauri\tauri.conf.json"
$config = Get-Content -Raw -LiteralPath $configPath | ConvertFrom-Json
$version = [string]$config.version
$repo = "ngthanhvu/clipboard-manager"

$bundleDir = Join-Path $root "src-tauri\target\release\bundle\nsis"
$installer = Get-ChildItem -LiteralPath $bundleDir -Filter "*_x64-setup.exe" |
  Sort-Object LastWriteTime -Descending |
  Select-Object -First 1

if (-not $installer) {
  throw "NSIS installer was not found in $bundleDir"
}

$signaturePath = "$($installer.FullName).sig"
if (-not (Test-Path -LiteralPath $signaturePath)) {
  throw "Updater signature was not found: $signaturePath"
}

$signature = (Get-Content -Raw -LiteralPath $signaturePath).Trim()
$latest = [ordered]@{
  version = $version
  notes = "Clipboard Manager $version"
  pub_date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
  platforms = [ordered]@{
    "windows-x86_64" = [ordered]@{
      signature = $signature
      url = "https://github.com/$repo/releases/download/v$version/$($installer.Name)"
    }
  }
}

$latestPath = Join-Path $bundleDir "latest.json"
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$json = $latest | ConvertTo-Json -Depth 6
[System.IO.File]::WriteAllText($latestPath, $json, $utf8NoBom)
Write-Host "Created $latestPath"
