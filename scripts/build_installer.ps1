<#
.SYNOPSIS
    Build the Rune launcher portable executable and, when available, its installer.
.DESCRIPTION
    Uses only Cargo and an already-installed Inno Setup compiler. No tools are
    installed. Fonts and application assets are embedded in the Rune executable.
#>
[CmdletBinding()]
param(
    [string]$Version = '',
    [switch]$SkipInstaller,
    [string]$IsccPath = ''
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$appExe = 'PrivateMewingLauncher'
$iconPath = Join-Path $root 'assets\icon.ico'
$dist = Join-Path $root 'dist'

if ($env:OS -ne 'Windows_NT') { throw 'Windows packaging must run on Windows.' }
if (-not (Test-Path -LiteralPath $iconPath -PathType Leaf)) { throw "Missing $iconPath" }

Push-Location $root
try {
    # Metadata locates this package without assuming a target directory, target
    # triple, or binary name. Build artifacts identify the actual compiled exe.
    $metadataLines = & cargo metadata --no-deps --format-version=1 --locked
    if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed with exit code $LASTEXITCODE" }
    $metadata = ($metadataLines -join "`n") | ConvertFrom-Json
    $manifest = Join-Path $root 'Cargo.toml'
    $package = @($metadata.packages | Where-Object {
        [IO.Path]::GetFullPath($_.manifest_path) -eq [IO.Path]::GetFullPath($manifest)
    }) | Select-Object -First 1
    if (-not $package) { throw 'Could not locate the launcher package in Cargo metadata.' }
    if (-not $Version) { $Version = $package.version }
    if ($Version -notmatch '^\d+\.\d+\.\d+(\.\d+)?$') {
        throw 'Installer version must have three or four numeric components.'
    }
    Write-Output "Building Private Mewing Launcher $Version"

    $buildLines = & cargo build --release --locked --message-format=json
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }
    $executables = @($buildLines | ForEach-Object {
        $artifact = $_ | ConvertFrom-Json
        if ($artifact.reason -eq 'compiler-artifact' -and
            $artifact.package_id -eq $package.id -and
            $artifact.target.kind -contains 'bin' -and $artifact.executable) {
            $artifact.executable
        }
    })
    if ($executables.Count -ne 1) { throw 'Expected exactly one launcher binary from Cargo.' }
    $releaseExe = $executables[0]
    if (-not (Test-Path -LiteralPath $releaseExe -PathType Leaf) -or
        [IO.Path]::GetExtension($releaseExe) -ne '.exe') {
        throw "Missing Windows release executable: $releaseExe"
    }
    New-Item -ItemType Directory -Force -Path $dist | Out-Null
    $portablePath = Join-Path $dist "$appExe.exe"
    Copy-Item -LiteralPath $releaseExe -Destination $portablePath -Force
    Write-Output ("PORTABLE: {0} ({1:N0} bytes)" -f $portablePath, (Get-Item -LiteralPath $portablePath).Length)

    if ($SkipInstaller) { return }
    $iscc = $IsccPath
    if ($iscc) {
        if (-not (Test-Path -LiteralPath $iscc -PathType Leaf)) { throw "Missing compiler: $iscc" }
    } else {
        $candidates = @(
            'C:\Program Files (x86)\Inno Setup 6\ISCC.exe',
            'C:\Program Files\Inno Setup 6\ISCC.exe'
        )
        if ($env:LOCALAPPDATA) {
            $candidates += Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'
        }
        $iscc = $candidates | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
        if (-not $iscc) {
            $command = Get-Command ISCC.exe -ErrorAction SilentlyContinue
            if ($command) { $iscc = $command.Source }
        }
    }
    if (-not $iscc) {
        Write-Warning 'Inno Setup compiler not found; portable executable is ready. Installer skipped (no tools installed).'
        return
    }
    Write-Output "Using Inno Setup compiler: $iscc"
    & $iscc "/DAppVersion=$Version" (Join-Path $root 'installer\mewing.iss')
    if ($LASTEXITCODE -ne 0) { throw "ISCC failed with exit code $LASTEXITCODE" }
    $setupPath = Join-Path $dist "$appExe-Setup-$Version.exe"
    if (-not (Test-Path -LiteralPath $setupPath -PathType Leaf)) { throw "Missing $setupPath" }
    Write-Output ("INSTALLER: {0} ({1:N0} bytes)" -f $setupPath, (Get-Item -LiteralPath $setupPath).Length)
} finally {
    Pop-Location
}
