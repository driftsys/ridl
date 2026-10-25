<#
.SYNOPSIS
    Installs a binary on Windows from the newest editor-v* GitHub Release (or
    $env:RIDL_VERSION): downloads <binary>-x86_64-pc-windows-msvc.tar.gz and
    its .sha256, verifies the checksum, and places <binary>.exe in
    $env:RIDL_INSTALL_DIR (default %LOCALAPPDATA%\Programs\ridl, the folder the
    VS Code extension uses for its "Install ridl to PATH" command).

    $env:RIDL_INSTALL_BINARY selects which binary the release carries to
    install: ridl (default, the porcelain CLI and what the editor extension
    bundles) or ridlc (the plumbing compiler front end, stable flags for CI
    and build systems). Both are built for the same targets and published to
    the same release, one tarball per binary per target.

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
# `irm ... | iex` runs this script in the caller's scope, so the two
# preferences and the TLS setting changed below would stay changed in the
# caller's session. The values are saved here and restored after Main.
$SavedErrorActionPreference = $ErrorActionPreference
$SavedProgressPreference = $ProgressPreference
$SavedSecurityProtocol = [Net.ServicePointManager]::SecurityProtocol
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
# Windows PowerShell 5.1 can default to protocols that GitHub refuses. Add
# TLS 1.2 to the enabled set for the downloads; the set is restored after Main.
[Net.ServicePointManager]::SecurityProtocol = $SavedSecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Repo = 'driftsys/ridl'
$TagPrefix = 'editor-v'
$Target = 'x86_64-pc-windows-msvc'
$BinaryName = if ($env:RIDL_INSTALL_BINARY) { $env:RIDL_INSTALL_BINARY } else { 'ridl' }
if ($BinaryName -ne 'ridl' -and $BinaryName -ne 'ridlc') {
    Write-Error "RIDL_INSTALL_BINARY must be 'ridl' or 'ridlc', got '$BinaryName'"
}
$Binary = "$BinaryName.exe"
$LocalAppData = if ($env:LOCALAPPDATA) { $env:LOCALAPPDATA } else { Join-Path $HOME 'AppData\Local' }
$InstallDir = if ($env:RIDL_INSTALL_DIR) { $env:RIDL_INSTALL_DIR } else { Join-Path $LocalAppData 'Programs\ridl' }
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
    $tarball = "$BinaryName-$Target.tar.gz"
    $url = "$BaseUrl/$version/$tarball"
    if ($env:RIDL_INSTALL_DRY_RUN) { Write-Output $url; return }

    Write-Host "Installing $BinaryName $version ($Target) to $InstallDir"
    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("ridl-install-" + [System.Guid]::NewGuid())
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        Invoke-WebRequest -Uri $url -OutFile (Join-Path $tmp $tarball) -UseBasicParsing
        Invoke-WebRequest -Uri "$url.sha256" -OutFile (Join-Path $tmp "$tarball.sha256") -UseBasicParsing
        Test-Checksum -File (Join-Path $tmp $tarball) -ChecksumFile (Join-Path $tmp "$tarball.sha256")
        # Windows 10 1803+ ships bsdtar as tar.exe. $ErrorActionPreference does
        # not apply to a native command's own exit code, so it is checked here
        # directly — otherwise a failed or partial extraction would go
        # unnoticed and either error confusingly at Move-Item or, worse, move
        # a truncated binary onto the user's PATH.
        tar -xzf (Join-Path $tmp $tarball) -C $tmp
        if ($LASTEXITCODE -ne 0) {
            Write-Error "tar extraction failed with exit code $LASTEXITCODE"
        }
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

try {
    Main
} finally {
    [Net.ServicePointManager]::SecurityProtocol = $SavedSecurityProtocol
    $ProgressPreference = $SavedProgressPreference
    $ErrorActionPreference = $SavedErrorActionPreference
}
