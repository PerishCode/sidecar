$ErrorActionPreference = 'Stop'

$command = if ($args.Length -gt 0) { $args[0] } else { 'install' }
$rest = if ($args.Length -gt 1) { $args[1..($args.Length - 1)] } else { @() }

$channel = if ($env:SIDECAR_CHANNEL) { $env:SIDECAR_CHANNEL } else { 'stable' }
$version = if ($env:SIDECAR_VERSION) { $env:SIDECAR_VERSION } else { '' }
$publicUrl = if ($env:SIDECAR_RELEASES_PUBLIC_URL) { $env:SIDECAR_RELEASES_PUBLIC_URL } else { '' }
$installRoot = if ($env:SIDECAR_INSTALL_ROOT) { $env:SIDECAR_INSTALL_ROOT } else { Join-Path $HOME '.local/share/sidecar' }
$localBinDir = if ($env:SIDECAR_LOCAL_BIN_DIR) { $env:SIDECAR_LOCAL_BIN_DIR } else { Join-Path $HOME '.local/bin' }

for ($i = 0; $i -lt $rest.Length; $i++) {
    switch -Regex ($rest[$i]) {
        '^--channel$' { $i++; $channel = $rest[$i]; continue }
        '^--channel=(.+)$' { $channel = $Matches[1]; continue }
        '^--version$' { $i++; $version = $rest[$i]; continue }
        '^--version=(.+)$' { $version = $Matches[1]; continue }
        '^--public-url$' { $i++; $publicUrl = $rest[$i]; continue }
        '^--public-url=(.+)$' { $publicUrl = $Matches[1]; continue }
        '^--install-root$' { $i++; $installRoot = $rest[$i]; continue }
        '^--install-root=(.+)$' { $installRoot = $Matches[1]; continue }
        '^--bin-dir$' { $i++; $localBinDir = $rest[$i]; continue }
        '^--bin-dir=(.+)$' { $localBinDir = $Matches[1]; continue }
        '^-h$|^--help$|^help$' {
            @'
sidecar manager

Usage:
  manage.ps1 install [--channel stable|beta] [--version vX.Y.Z] [--public-url <url>]
  manage.ps1 update  [--channel stable|beta] [--version vX.Y.Z] [--public-url <url>]
  manage.ps1 uninstall [--version vX.Y.Z]
'@ | Write-Output
            exit 0
        }
        default { throw "unknown argument: $($rest[$i])" }
    }
}

function Need-PublicUrl {
    if ([string]::IsNullOrWhiteSpace($publicUrl)) {
        throw 'SIDECAR_RELEASES_PUBLIC_URL or --public-url is required'
    }
    return $publicUrl.TrimEnd('/')
}

function Latest-Version($metadataPath) {
    $metadata = Get-Content -Raw -Path $metadataPath | ConvertFrom-Json
    return $metadata.releaseVersion
}

function Install-Sidecar {
    $baseUrl = Need-PublicUrl
    $tmpdir = Join-Path ([System.IO.Path]::GetTempPath()) ("sidecar-install-" + [System.Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tmpdir | Out-Null
    try {
        if ([string]::IsNullOrWhiteSpace($version)) {
            $metadataPath = Join-Path $tmpdir 'metadata.json'
            Invoke-WebRequest -Uri "$baseUrl/$channel/latest/metadata.json" -OutFile $metadataPath
            $script:version = Latest-Version $metadataPath
            if ([string]::IsNullOrWhiteSpace($script:version)) {
                throw 'failed to resolve latest sidecar version'
            }
        }

        $archive = 'sidecar-x86_64-pc-windows-msvc.zip'
        $archivePath = Join-Path $tmpdir $archive
        Invoke-WebRequest -Uri "$baseUrl/$channel/versions/$version/$archive" -OutFile $archivePath

        $versionRoot = Join-Path $installRoot $version
        New-Item -ItemType Directory -Force -Path $versionRoot | Out-Null
        New-Item -ItemType Directory -Force -Path $localBinDir | Out-Null
        Expand-Archive -LiteralPath $archivePath -DestinationPath $versionRoot -Force

        $cmd = Join-Path $localBinDir 'sidecar.cmd'
        $exe = Join-Path $versionRoot 'sidecar.exe'
        "@echo off`r`n`"$exe`" %*`r`n" | Set-Content -Encoding ASCII -Path $cmd
        & $cmd --version
        Write-Output "installed sidecar to $cmd"
    }
    finally {
        Remove-Item -LiteralPath $tmpdir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Uninstall-Sidecar {
    $cmd = Join-Path $localBinDir 'sidecar.cmd'
    Remove-Item -LiteralPath $cmd -Force -ErrorAction SilentlyContinue
    Write-Output "removed $cmd"

    if ([string]::IsNullOrWhiteSpace($version)) {
        Remove-Item -LiteralPath $installRoot -Recurse -Force -ErrorAction SilentlyContinue
        Write-Output "removed $installRoot"
    }
    else {
        $versionRoot = Join-Path $installRoot $version
        Remove-Item -LiteralPath $versionRoot -Recurse -Force -ErrorAction SilentlyContinue
        Write-Output "removed $versionRoot"
    }
}

switch ($command) {
    'install' { Install-Sidecar }
    'update' { Install-Sidecar }
    'uninstall' { Uninstall-Sidecar }
    default { throw "unknown command: $command" }
}
