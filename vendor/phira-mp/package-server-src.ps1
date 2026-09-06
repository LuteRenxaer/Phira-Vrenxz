# ============================================================
# Package minimal phira-mp-server build sources into ONE folder:
#   server-src/
# Upload only this single folder to the server to compile.
# ============================================================
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$out  = Join-Path $root "server-src"

# clean old output
if (Test-Path $out) { Remove-Item $out -Recurse -Force }
New-Item -ItemType Directory -Path $out | Out-Null

# 1) workspace root files: Cargo.toml (drop phira-mp-client member) / Cargo.lock
$cargo = Get-Content (Join-Path $root "Cargo.toml") -Raw
$cargo = $cargo -replace '(?m)^\s*"phira-mp-client",\s*$', ''
Set-Content (Join-Path $out "Cargo.toml") $cargo -NoNewline -Encoding utf8
Copy-Item (Join-Path $root "Cargo.lock") (Join-Path $out "Cargo.lock")

# 2) the 3 crates the server actually depends on (phira-mp-client NOT needed)
foreach ($c in @("phira-mp-server", "phira-mp-common", "phira-mp-macros")) {
    $src = Join-Path $root $c
    Copy-Item $src (Join-Path $out $c) -Recurse
    $t = Join-Path $out "$c\target"
    if (Test-Path $t) { Remove-Item $t -Recurse -Force }
}

Write-Host ""
Write-Host ("Packaged: " + $out)
Write-Host "Upload the whole server-src folder to the server, then run:"
Write-Host ("  cd server-src")
Write-Host ("  cargo build --release" + " -p phira-mp-server")
Write-Host ""
Write-Host "If Rust is not installed, install it first: https://rustup.rs"
