$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$env:CARGO_HOME = Join-Path $root ".cargo-home"
$env:CARGO_TARGET_DIR = Join-Path $root "target"
$env:CARGO_INCREMENTAL = "1"

Write-Host "CARGO_HOME=$env:CARGO_HOME"
Write-Host "CARGO_TARGET_DIR=$env:CARGO_TARGET_DIR"
Write-Host "CARGO_INCREMENTAL=$env:CARGO_INCREMENTAL"
