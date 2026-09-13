<#
.SYNOPSIS
    Installs the ridl binary on Windows from the newest editor-v* GitHub
    Release (or $env:RIDL_VERSION): downloads ridl-x86_64-pc-windows-msvc.tar.gz
    and its .sha256, verifies the checksum, and places ridl.exe in
    $env:RIDL_INSTALL_DIR (default $HOME\.local\bin).

    $env:RIDL_INSTALL_BASE_URL overrides where release assets are fetched from
    (default https://github.com/$Repo/releases/download). It is a test hook
    for install.sh's own end-to-end check, kept identical here so the two
    scripts do not disagree about their own interface; it is not a setting an
    end user is expected to set.
.EXAMPLE
    irm https://raw.githubusercontent.com/driftsys/ridl/main/install.ps1 | iex
#>
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Repo = 'driftsys/ridl'
$TagPrefix = 'editor-v'
$Target = 'x86_64-pc-windows-msvc'
$Binary = 'ridl.exe'
$InstallDir = if ($env:RIDL_INSTALL_DIR) { $env:RIDL_INSTALL_DIR } else { Join-Path $HOME '.local\bin' }
$BaseUrl = if ($env:RIDL_INSTALL_BASE_URL) { $env:RIDL_INSTALL_BASE_URL } else { "https://github.com/$Repo/releases/download" }

function Get-Version {
    if ($env:RIDL_VERSION) { return $env:RIDL_VERSION }
    # Never /releases/latest: it is the newest release across every tag.
    $releases = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases?per_page=100" -UseBasicParsing
    $match = $releases | Where-Object { $_.tag_name -like "$TagPrefix*" } | Select-Object -First 1
    if (-not $match) { Write-Error "no $TagPrefix* release found" }
    return $match.tag_name
}

function Test-Checksum {
    param([string]$File, [string]$ChecksumFile)
    $expected = ((Get-Content -LiteralPath $ChecksumFile -Raw).Trim() -split '\s+', 2)[0]
    $actual = (Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash
    if ($expected.ToLower() -ne $actual.ToLower()) {
        Write-Error "checksum mismatch: expected $expected, got $actual"
    }
}

function Main {
    $version = Get-Version
    $tarball = "ridl-$Target.tar.gz"
    $url = "$BaseUrl/$version/$tarball"
    if ($env:RIDL_INSTALL_DRY_RUN) { Write-Output $url; return }

    Write-Host "Installing ridl $version ($Target) to $InstallDir"
    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("ridl-install-" + [System.Guid]::NewGuid())
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        Invoke-WebRequest -Uri $url -OutFile (Join-Path $tmp $tarball) -UseBasicParsing
        Invoke-WebRequest -Uri "$url.sha256" -OutFile (Join-Path $tmp "$tarball.sha256") -UseBasicParsing
        Test-Checksum -File (Join-Path $tmp $tarball) -ChecksumFile (Join-Path $tmp "$tarball.sha256")
        # Windows 10 1803+ ships bsdtar as tar.exe.
        tar -xzf (Join-Path $tmp $tarball) -C $tmp
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        Move-Item -LiteralPath (Join-Path $tmp $Binary) -Destination (Join-Path $InstallDir $Binary) -Force
        Write-Host "Installed $(Join-Path $InstallDir $Binary)"
        $onPath = ($env:Path -split ';') | Where-Object { $_.Trim().ToLower() -eq $InstallDir.ToLower() }
        if (-not $onPath) {
            # Through the registry, not setx: setx truncates a PATH over 1024 characters.
            Write-Host ""
            Write-Host "Add to your PATH (PowerShell):"
            Write-Host "  [Environment]::SetEnvironmentVariable('Path', `"$InstallDir;`" + [Environment]::GetEnvironmentVariable('Path', 'User'), 'User')"
        }
    } finally {
        Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Main
