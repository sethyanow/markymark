# select-binary.ps1 — Execute the bundled markymark binary (Windows).
#
# In the CI pre-packaged model, each per-platform plugin archive
# contains a single bin/markymark.exe binary already built for the
# target platform. This script simply finds and executes it.
#
# If the binary is missing (e.g. dev checkout without a build),
# the error message includes the platform-specific archive name
# so the user can download the correct one from GitHub Releases.

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$BinDir = Join-Path $ScriptDir "..\bin"

function Main {
    # Prefer system-installed markymark if already on PATH
    $systemBinary = Get-Command markymark -ErrorAction SilentlyContinue
    if ($systemBinary) {
        & $systemBinary.Source @args
        exit $LASTEXITCODE
    }

    $binary = Join-Path $BinDir "markymark.exe"

    if (-not (Test-Path $binary)) {
        $target = "x86_64-pc-windows-msvc"
        Write-Error "error: binary not found: $binary"
        Write-Error "hint: download markymark-plugin-$target.zip from GitHub Releases"
        Write-Error "      https://github.com/sethyanow/markymark/releases"
        exit 1
    }

    # Execute the binary with all passed arguments
    & $binary @args
    exit $LASTEXITCODE
}

Main @args
