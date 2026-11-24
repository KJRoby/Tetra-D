[CmdletBinding()]
param(
    [switch]$SkipUpdate
)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$python = Join-Path $root '.venv\Scripts\python.exe'

if (-not $SkipUpdate) {
    & (Join-Path $root 'Update-Pygame.ps1')
}

if (-not (Test-Path -LiteralPath $python)) {
    throw 'The project virtual environment is missing. Run .\Update-Pygame.ps1 first.'
}

Push-Location $root
try {
    & $python 'frontends\pygame\main.py'
} finally {
    Pop-Location
}
