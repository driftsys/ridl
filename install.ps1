<#
.SYNOPSIS
    Installs a binary on Windows from the newest editor-v* GitHub Release (or
    $env:RIDL_VERSION): downloads <binary>-x86_64-pc-windows-msvc.tar.gz and
    its .sha256, verifies the checksum, stores pinnable ridl releases in the
    shared toolchain directory, and copies <binary>.exe to
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

# Accept decimal release components from 0 to 4294967295 (u32).
function ConvertTo-ReleaseVersion {
    param([string]$Value)
    $value = $Value -creplace '^editor-v', ''
    if ($value -cnotmatch '\A[0-9]+\.[0-9]+\.[0-9]+\z') { throw "invalid release version $Value" }
    $parts = foreach ($part in $value.Split('.')) {
        [uint32]$number = 0
        if (-not [uint32]::TryParse($part, [ref]$number)) { throw "invalid release version $Value" }
        $number.ToString([Globalization.CultureInfo]::InvariantCulture)
    }
    return [string]($parts -join '.')
}

function Get-ToolchainRoot {
    if ($env:RIDL_TOOLCHAIN_DIR) { return [IO.Path]::GetFullPath($env:RIDL_TOOLCHAIN_DIR) }
    if (-not $env:LOCALAPPDATA) { throw 'toolchain directory requires LOCALAPPDATA' }
    return [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'ridl'))
}

function Test-CompleteRelease {
    param([string]$Directory)
    if (-not (Test-Path -LiteralPath $Directory)) { return $false }
    $dir = Get-Item -LiteralPath $Directory -Force
    $binaryPath = Join-Path $Directory 'ridl.exe'
    $markerPath = Join-Path $Directory '.complete'
    $valid = $dir.PSIsContainer -and -not ($dir.Attributes -band [IO.FileAttributes]::ReparsePoint)
    foreach ($path in @($binaryPath, $markerPath)) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { $valid = $false; continue }
        if ((Get-Item -LiteralPath $path -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { $valid = $false }
    }
    if ($valid -and (Get-Item -LiteralPath $markerPath -Force).Length -eq 0) { return $true }
    throw "damaged toolchain directory $Directory; preserve or move it before installing again"
}

function Get-Download {
    param([string]$Url, [string]$File, [long]$Limit)
    # Stream a bounded response directly to disk, including for chunked HTTP.
    $request = [Net.WebRequest]::Create($Url)
    $request.Timeout = 30000
    if ($request -is [Net.HttpWebRequest]) { $request.ReadWriteTimeout = 30000 }
    $response = $null; $inputStream = $null; $outputStream = $null
    $timer = [Diagnostics.Stopwatch]::StartNew()
    try {
        $response = $request.GetResponse()
        if ($response.ContentLength -gt $Limit) { throw "download exceeds $Limit bytes" }
        $inputStream = $response.GetResponseStream()
        $outputStream = [IO.File]::Open($File, [IO.FileMode]::CreateNew)
        $buffer = New-Object byte[] 81920
        [long]$total = 0
        while (($count = $inputStream.Read($buffer, 0, $buffer.Length)) -gt 0) {
            $total += $count
            if ($total -gt $Limit) { throw "download exceeds $Limit bytes" }
            if ($timer.Elapsed.TotalSeconds -gt 600) { throw 'download exceeded 600 seconds' }
            $outputStream.Write($buffer, 0, $count)
        }
    } finally {
        if ($outputStream) { $outputStream.Dispose() }
        if ($inputStream) { $inputStream.Dispose() }
        if ($response) { $response.Close() }
    }
}

function Test-Checksum {
    param([string]$File, [string]$ChecksumFile, [string]$Archive)
    $record = Get-Content -LiteralPath $ChecksumFile -Raw
    $pattern = '\A([a-fA-F0-9]{64})(?:  | \*)' + [regex]::Escape($Archive) + '(?:\r?\n)?\z'
    if ($record -cnotmatch $pattern) { throw "invalid checksum record for $Archive" }
    $expected = $Matches[1]
    $actual = (Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash
    if ($expected -ine $actual) { throw "checksum mismatch: expected $expected, got $actual" }
}

function Expand-Release {
    param([string]$Archive, [string]$Directory)
    $names = @(tar -tzf $Archive)
    if ($LASTEXITCODE -ne 0) { throw "tar extraction failed with exit code $LASTEXITCODE" }
    $types = @(tar -tvzf $Archive)
    if ($LASTEXITCODE -ne 0) { throw "tar extraction failed with exit code $LASTEXITCODE" }
    $required = @($Binary, 'LICENSE', 'THIRD-PARTY-NOTICES.txt')
    if ($names.Count -ne 3 -or @($names | Select-Object -Unique).Count -ne 3) { throw 'archive missing required files or contains duplicates' }
    foreach ($name in $names) {
        if ($required -cnotcontains $name) { throw "unsafe archive entry $name" }
    }
    foreach ($line in $types) {
        if (-not $line.StartsWith('-')) { throw 'unsafe archive entry type' }
    }
    tar -xzf $Archive -C $Directory
    if ($LASTEXITCODE -ne 0) { throw "tar extraction failed with exit code $LASTEXITCODE" }
    foreach ($name in $required) {
        $path = Join-Path $Directory $name
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "archive missing $name" }
        if ((Get-Item -LiteralPath $path -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "unsafe archive entry $name" }
    }
}

function Main {
    if ($BinaryName -ne 'ridl' -and $BinaryName -ne 'ridlc') {
        throw "RIDL_INSTALL_BINARY must be 'ridl' or 'ridlc', got '$BinaryName'"
    }
    $raw = Get-Version
    $version = $raw
    $normalized = $null
    try { $normalized = ConvertTo-ReleaseVersion $raw } catch {
        # Reselect only plain releases; historical tags retain flat installation.
    }
    $versioned = $false
    if ($normalized) {
        $version = "$TagPrefix$normalized"
        $parts = $normalized.Split('.')
        $versioned = $BinaryName -eq 'ridl' -and ([uint32]$parts[0] -gt 0 -or [uint32]$parts[1] -ge 23)
    }
    $tarball = "$BinaryName-$Target.tar.gz"
    $url = "$BaseUrl/$version/$tarball"
    if ($env:RIDL_INSTALL_DRY_RUN) { Write-Output $url; return }

    $tmp = $null; $stage = $null; $source = $null; $tmpBinary = $null
    try {
        if ($versioned) {
            $versions = Join-Path (Get-ToolchainRoot) 'versions'
            $directory = Join-Path $versions $normalized
            if (Test-CompleteRelease $directory) { $source = Join-Path $directory $Binary }
        }
        if (-not $source) {
            if ($versioned) {
                New-Item -ItemType Directory -Path $versions -Force | Out-Null
                # A previous directory for this PID is stale; other stages may be live.
                $candidate = Join-Path $versions ".tmp-script-$normalized-$PID"
                if (Test-Path -LiteralPath $candidate) {
                    Remove-Item -LiteralPath $candidate -Recurse -Force
                }
                New-Item -ItemType Directory -Path $candidate | Out-Null
                $stage = $candidate
            }
            $candidate = Join-Path ([IO.Path]::GetTempPath()) ("ridl-install-" + [guid]::NewGuid())
            New-Item -ItemType Directory -Path $candidate | Out-Null
            $tmp = $candidate
            $archive = Join-Path $tmp $tarball
            Get-Download $url $archive 268435456
            Get-Download "$url.sha256" "$archive.sha256" 4096
            Test-Checksum $archive "$archive.sha256" $tarball
            if ($versioned) {
                Expand-Release $archive $stage
                [IO.File]::WriteAllBytes((Join-Path $stage '.complete'), [byte[]]@())
                if (-not (Test-CompleteRelease $directory)) {
                    # Directory.Move refuses an existing destination atomically.
                    try {
                        [IO.Directory]::Move($stage, $directory)
                    } catch {
                        if (-not (Test-CompleteRelease $directory)) {
                            throw "toolchain directory could not be published: $directory; $($_.Exception.Message)"
                        }
                    }
                }
                if (Test-Path -LiteralPath $stage) {
                    Remove-Item -LiteralPath $stage -Recurse -Force
                }
                $stage = $null
                $source = Join-Path $directory $Binary
            } else {
                tar -xzf $archive -C $tmp
                if ($LASTEXITCODE -ne 0) { throw "tar extraction failed with exit code $LASTEXITCODE" }
                $source = Join-Path $tmp $Binary
            }
        }
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        $tmpBinary = Join-Path $InstallDir (".$BinaryName." + [guid]::NewGuid())
        Copy-Item -LiteralPath $source -Destination $tmpBinary
        Move-Item -LiteralPath $tmpBinary -Destination (Join-Path $InstallDir $Binary) -Force
        $tmpBinary = $null
        Write-Host "Installed $(Join-Path $InstallDir $Binary)"
        $onPath = ($env:Path -split ';') | Where-Object { $_.Trim().ToLower() -eq $InstallDir.ToLower() }
        if (-not $onPath) {
            Write-Host 'Add to your PATH (PowerShell):'
            Write-Host "  [Environment]::SetEnvironmentVariable('Path', `"$InstallDir;`" + [Environment]::GetEnvironmentVariable('Path', 'User'), 'User')"
        }
    } finally {
        foreach ($owned in @($tmp, $stage, $tmpBinary)) {
            if ($owned) { Remove-Item -LiteralPath $owned -Recurse -Force -ErrorAction SilentlyContinue }
        }
    }
}

try {
    Main
} finally {
    [Net.ServicePointManager]::SecurityProtocol = $SavedSecurityProtocol
    $ProgressPreference = $SavedProgressPreference
    $ErrorActionPreference = $SavedErrorActionPreference
}
