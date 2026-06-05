$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path))))
$version = if ($args.Length -gt 0) { $args[0] } else { '' }
$channel = if ($args.Length -gt 1) { $args[1] } else { 'stable' }

if ([string]::IsNullOrWhiteSpace($version)) {
    throw 'missing release version'
}
if ([string]::IsNullOrWhiteSpace($env:SIDECAR_RELEASES_PUBLIC_URL)) {
    throw 'SIDECAR_RELEASES_PUBLIC_URL is required'
}

$tmpdir = Join-Path ([System.IO.Path]::GetTempPath()) ("sidecar-smoke-" + [System.Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmpdir | Out-Null

try {
    $env:HOME = Join-Path $tmpdir 'home'
    $env:SIDECAR_INSTALL_ROOT = Join-Path $tmpdir 'install'
    $env:SIDECAR_LOCAL_BIN_DIR = Join-Path $tmpdir 'bin'
    New-Item -ItemType Directory -Force -Path $env:HOME, $env:SIDECAR_INSTALL_ROOT, $env:SIDECAR_LOCAL_BIN_DIR | Out-Null

    & "$root/manage.ps1" install --channel $channel --version $version
    & (Join-Path $env:SIDECAR_LOCAL_BIN_DIR 'sidecar.cmd') --version
    & (Join-Path $env:SIDECAR_LOCAL_BIN_DIR 'sidecar.cmd') doctor --config (Join-Path $root 'examples/minimal.toml')

    & "$root/manage.ps1" update --channel $channel --version $version
    & (Join-Path $env:SIDECAR_LOCAL_BIN_DIR 'sidecar.cmd') --version
    & (Join-Path $env:SIDECAR_LOCAL_BIN_DIR 'sidecar.cmd') doctor --config (Join-Path $root 'examples/minimal.toml')

    & "$root/manage.ps1" uninstall --version $version
    if (Test-Path (Join-Path $env:SIDECAR_LOCAL_BIN_DIR 'sidecar.cmd')) {
        throw "uninstall left $(Join-Path $env:SIDECAR_LOCAL_BIN_DIR 'sidecar.cmd')"
    }
    if (Test-Path (Join-Path $env:SIDECAR_INSTALL_ROOT $version)) {
        throw "version uninstall left $(Join-Path $env:SIDECAR_INSTALL_ROOT $version)"
    }

    if ($env:SMOKE_LATEST -eq '1') {
        Remove-Item -LiteralPath (Join-Path $env:SIDECAR_LOCAL_BIN_DIR 'sidecar.cmd') -Force -ErrorAction SilentlyContinue
        & "$root/manage.ps1" install --channel $channel --install-root (Join-Path $env:SIDECAR_INSTALL_ROOT 'latest-smoke')
        & (Join-Path $env:SIDECAR_LOCAL_BIN_DIR 'sidecar.cmd') --version
        & (Join-Path $env:SIDECAR_LOCAL_BIN_DIR 'sidecar.cmd') doctor --config (Join-Path $root 'examples/minimal.toml')
        & "$root/manage.ps1" uninstall --install-root (Join-Path $env:SIDECAR_INSTALL_ROOT 'latest-smoke')
        if (Test-Path (Join-Path $env:SIDECAR_LOCAL_BIN_DIR 'sidecar.cmd')) {
            throw "latest uninstall left $(Join-Path $env:SIDECAR_LOCAL_BIN_DIR 'sidecar.cmd')"
        }
        if (Test-Path (Join-Path $env:SIDECAR_INSTALL_ROOT 'latest-smoke')) {
            throw "full uninstall left $(Join-Path $env:SIDECAR_INSTALL_ROOT 'latest-smoke')"
        }
    }
}
finally {
    Remove-Item -LiteralPath $tmpdir -Recurse -Force -ErrorAction SilentlyContinue
}
