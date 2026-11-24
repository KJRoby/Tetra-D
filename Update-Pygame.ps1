[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$venv = Join-Path $root '.venv'
$python = Join-Path $venv 'Scripts\python.exe'

if (-not (Test-Path -LiteralPath $python)) {
    Write-Host 'Creating project virtual environment...'
    $bootstrap = Get-Command py -ErrorAction SilentlyContinue
    if ($bootstrap) {
        & py -3 -m venv $venv
    } else {
        & python -m venv $venv
    }
}

Write-Host 'Installing frontend build dependencies...'
& $python -m pip install --disable-pip-version-check --upgrade pip maturin
& $python -m pip install --disable-pip-version-check -r (Join-Path $root 'frontends\pygame\requirements.txt')

Write-Host 'Building and installing Dynamo Core Python bindings...'
Push-Location $root
try {
    & $python -m maturin develop --manifest-path 'crates/tetrad-python/Cargo.toml'
} finally {
    Pop-Location
}

Write-Host 'Pygame frontend is up to date.'
