# select-binary.ps1 — Platform detection and binary selection for markymark (Windows).
#
# Detects the host architecture, then executes the correct pre-built markymark
# binary from the plugin's bin/ directory.
#
# Supported: Windows x86_64 → markymark-x86_64-pc-windows-msvc.exe

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$BinDir = Join-Path $ScriptDir "..\bin"

function Get-Target {
    $arch = $env:PROCESSOR_ARCHITECTURE
    
    switch ($arch) {
        "AMD64" { return "x86_64-pc-windows-msvc" }
        "x86"   { 
            Write-Error "error: 32-bit Windows is not supported"
            exit 1
        }
        "ARM64" {
            Write-Error "error: Windows ARM64 is not yet supported"
            exit 1
        }
        default {
            Write-Error "error: unsupported Windows architecture: $arch"
            exit 1
        }
    }
}

function Main {
    $target = Get-Target
    $binary = Join-Path $BinDir "markymark-$target.exe"
    
    if (-not (Test-Path $binary)) {
        Write-Error "error: binary not found: $binary"
        Write-Error "hint: run the release build or download from GitHub Releases"
        exit 1
    }
    
    # Execute the binary with all passed arguments
    & $binary @args
    exit $LASTEXITCODE
}

Main @args
