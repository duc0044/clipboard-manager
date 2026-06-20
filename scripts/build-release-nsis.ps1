$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$envPath = Join-Path $root ".env"

if (-not (Test-Path -LiteralPath $envPath)) {
  throw ".env was not found. Add TAURI_SIGNING_PRIVATE_KEY before building a signed release."
}

foreach ($line in Get-Content -LiteralPath $envPath) {
  $trimmed = $line.Trim()
  if (-not $trimmed -or $trimmed.StartsWith("#")) {
    continue
  }

  $name, $value = $line -split "=", 2
  if ($name -and $null -ne $value) {
    Set-Item -Path "Env:$($name.Trim())" -Value $value.Trim()
  }
}

if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
  throw "TAURI_SIGNING_PRIVATE_KEY is missing from .env"
}

Push-Location $root
try {
  npx tauri build --bundles nsis
  npm run release:latest-json
}
finally {
  Pop-Location
}
