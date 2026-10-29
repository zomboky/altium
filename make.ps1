<#
.SYNOPSIS
Altium build entry points on Windows, the counterpart of the Makefile.

.DESCRIPTION
`.\make.ps1 build` produces a single self-contained binary at
target\release\altium.exe with the web front embedded. Works from any
directory, with the Windows PowerShell 5.1 that ships with Windows.

.EXAMPLE
.\make.ps1 build

.EXAMPLE
.\make.ps1 test
#>
[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet('build', 'web', 'test', 'dev', 'clean')]
    [string] $Target = 'build'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Like CARGO and NPM in the Makefile, both can be overridden.
$Cargo = if ($env:CARGO) { $env:CARGO } else { 'cargo' }
$Npm = if ($env:NPM) { $env:NPM } else { 'npm' }

# Runs a native command and stops at its first failure: Windows PowerShell
# does not do it on its own.
function Invoke-Tool([string] $Command, [string[]] $Arguments) {
    if (-not (Get-Command $Command -ErrorAction SilentlyContinue)) {
        throw "$Command introuvable : installe-le puis ouvre un nouveau terminal (voir le README)."
    }
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "``$Command $($Arguments -join ' ')`` a échoué (code $LASTEXITCODE)."
    }
}

# `npm ci` only when web\node_modules is missing or older than the manifests.
function Install-WebDependencies {
    $modules = 'web\node_modules'
    if (Test-Path $modules) {
        $installed = (Get-Item $modules).LastWriteTime
        $changed = @('web\package.json', 'web\package-lock.json' | Where-Object { (Get-Item $_).LastWriteTime -gt $installed })
        if ($changed.Count -eq 0) {
            return
        }
    }
    Invoke-Tool $Npm @('--prefix', 'web', 'ci', '--no-fund', '--no-audit')
    (Get-Item $modules).LastWriteTime = Get-Date
}

function Build-Web {
    Install-WebDependencies
    Invoke-Tool $Npm @('--prefix', 'web', 'run', 'build')
}

Push-Location $PSScriptRoot
try {
    switch ($Target) {
        'build' {
            Build-Web
            Invoke-Tool $Cargo @('build', '--release', '-p', 'altium-cli')
            Write-Host "Binaire prêt : $(Join-Path $PSScriptRoot 'target\release\altium.exe')"
        }
        'web' {
            Build-Web
        }
        'test' {
            Install-WebDependencies
            Invoke-Tool $Cargo @('test', '--workspace')
            Invoke-Tool $Npm @('--prefix', 'web', 'run', 'check')
        }
        # Front with hot reload; API calls are proxied to `altium serve` on :7420.
        'dev' {
            Install-WebDependencies
            Invoke-Tool $Npm @('--prefix', 'web', 'run', 'dev')
        }
        'clean' {
            Invoke-Tool $Cargo @('clean')
            if (Test-Path 'web\dist') {
                Remove-Item 'web\dist' -Recurse -Force
            }
        }
    }
}
finally {
    Pop-Location
}
