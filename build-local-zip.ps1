# build-local-zip.ps1
# Builds release CLAPs via cargo-aura, packages Plugin-WGPU-vX.Y.Z-{win|linux}.zip
#
# Default: full shipping set (aether, equilibrium, lucent, lucent-relay,
# meridian), both platforms.
#
# Usage:
#   .\build-local-zip.ps1                              # full shipping set, win+linux
#   .\build-local-zip.ps1 -Platform win                # Windows only
#   .\build-local-zip.ps1 -Platform linux              # Linux cross only
#   .\build-local-zip.ps1 -Plugins lucent,aether       # subset (comma or space separated)
#
# Linux cross (from Windows) requires:
#   winget install zig.zig --source winget
#   cargo install cargo-zigbuild
#   rustup target add x86_64-unknown-linux-gnu
#   cargo-aura (installed from AURA repo)
# Zig linker is wired via a caller-scoped env var in this script, pointing at
# .cargo/zigcc-*.bat. .cargo/config.toml stays clean so GitHub Actions'
# native Linux builds are unaffected.
#
# Output:
#   dist/<ClapName>-WGPU-vX.Y.Z-win.zip
#   dist/<ClapName>-WGPU-vX.Y.Z-linux.zip

param(
    [string[]]$Plugins = @(
        "aether",
        "equilibrium",
        "lucent",
        "lucent-relay",
        "meridian"
    ),
    # Catches space-separated names after -Plugins (e.g. `-Plugins lucent aether`).
    # Without this, PowerShell cascades the extras into -LinuxTarget.
    [Parameter(Position = 1, ValueFromRemainingArguments = $true)]
    [string[]]$ExtraPluginArgs,
    [ValidateSet("win", "linux", "both")]
    [string]$Platform = "both",
    [string]$LinuxTarget = "x86_64-unknown-linux-gnu"
)

$ErrorActionPreference = "Stop"
$distDir = "dist"

# CLAP product names (must match aura.toml name → display name)
$clapNames = @{
    "aether"       = "Aether"
    "equilibrium"  = "Equilibrium"
    "lucent"       = "Lucent"
    "lucent-relay" = "Lucent Relay"
    "meridian"     = "Meridian"
}

# Accept comma-separated single arg: .\build-local-zip.ps1 lucent,aether
if ($Plugins.Count -eq 1 -and $Plugins[0] -match ",") {
    $Plugins = $Plugins[0] -split "," | ForEach-Object { $_.Trim() } | Where-Object { $_ }
}

# Fold space-separated extras into $Plugins
if ($ExtraPluginArgs) {
    $Plugins = @($Plugins) + @($ExtraPluginArgs | ForEach-Object { ($_ -split ",") } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
}

# Cargo package names are lowercase and case-sensitive; accept `Lucent` etc.
$Plugins = @($Plugins | ForEach-Object { $_.ToLowerInvariant() })

$platforms = if ($Platform -eq "both") { @("win", "linux") } else { @($Platform) }

function Initialize-LinuxToolchain {
    param([string]$Target)
    if (-not (Get-Command zig -ErrorAction SilentlyContinue)) {
        Write-Warning "zig not on PATH. Install: winget install zig.zig --source winget"
    }
    if (-not (Get-Command cargo-zigbuild -ErrorAction SilentlyContinue)) {
        Write-Warning "cargo-zigbuild not on PATH. Install: cargo install cargo-zigbuild"
    }
    $rustupShow = & rustup target list --installed 2>$null
    if ($rustupShow -notcontains $Target) {
        Write-Host "Adding rustup target $Target ..." -ForegroundColor Yellow
        rustup target add $Target
        if ($LASTEXITCODE -ne 0) { throw "rustup target add failed" }
    }
}

function Get-PluginVersion {
    param([string]$CargoToml)
    $line = Select-String -Path $CargoToml -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
    if (-not $line) { throw "No version in $CargoToml" }
    return $line.Matches[0].Groups[1].Value
}

function Build-And-Package {
    param(
        [string[]]$PluginList,
        [string]$Plat,
        [string]$Target,
        [hashtable]$Names,
        [string]$OutDir
    )

    $suffix = if ($Plat -eq "linux") { "linux" } else { "win" }
    $bundlesDir = if ($Plat -eq "linux") {
        Join-Path "target\bundles" $Target
    } else {
        "target\bundles"
    }

    $list = $PluginList -join ", "
    Write-Host "=== Building: $list  [$Plat] ===" -ForegroundColor Cyan

    if ($Plat -eq "linux") {
        Initialize-LinuxToolchain -Target $Target
        # Skip fontconfig pkg-config sysroot (Windows host has none).
        # Runtime dlopen of libfontconfig on Linux DAW machines.
        $env:RUST_FONTCONFIG_DLOPEN = "on"
        # Wire the Zig linker here (caller scope) instead of .cargo/config.toml.
        # The .bat rehydrates PATH and invokes `cargo-zigbuild zig cc` so plain
        # `cargo aura build --target` links a glibc .so from a Windows host.
        $linkerVar = "CARGO_TARGET_" + (($Target -replace '-', '_').ToUpperInvariant()) + "_LINKER"
        $linkerBat = Join-Path $PSScriptRoot ".cargo\zigcc-$Target.bat"
        if (-not (Test-Path $linkerBat)) {
            throw "Missing Zig linker wrapper: $linkerBat"
        }
        Set-Item -Path "Env:$linkerVar" -Value $linkerBat
    }

    # cargo-aura builds one plugin at a time; copy the cdylib into target\bundles
    # and rename to <DisplayName>.clap for packaging.
    foreach ($p in $PluginList) {
        Write-Host "--- cargo aura build --clap -plug $p [$Plat] ---" -ForegroundColor DarkCyan
        $buildArgs = @("aura", "build", "--clap", "-plug", $p, "--release")
        if ($Plat -eq "linux") {
            $buildArgs += @("--target", $Target)
        }
        & cargo @buildArgs
        if ($LASTEXITCODE -ne 0) { throw "Build failed ($Plat / $p)" }

        $clapName = $Names[$p]
        $crateName = $p -replace '-', '_'
        $src = if ($Plat -eq "linux") {
            Join-Path "target" $Target | Join-Path -ChildPath "release" | Join-Path -ChildPath "lib$crateName.so"
        } else {
            Join-Path "target" "release" | Join-Path -ChildPath "$crateName.dll"
        }
        $dstDir = $bundlesDir
        New-Item -ItemType Directory -Force -Path $dstDir | Out-Null
        $dst = Join-Path $dstDir ($clapName + ".clap")
        if (-not (Test-Path $src)) {
            throw "Built artifact not found: $src"
        }
        Copy-Item -Path $src -Destination $dst -Force
        Write-Host "  -> $dst" -ForegroundColor DarkGray
    }

    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
    Write-Host "=== Packaging -> $OutDir (bundles: $bundlesDir) ===" -ForegroundColor Cyan

    foreach ($plugin in $PluginList) {
        $cargoToml = "plugins/$plugin/Cargo.toml"
        if (-not (Test-Path $cargoToml)) {
            Write-Warning "Skipping $plugin - no Cargo.toml"
            continue
        }
        if (-not $Names.ContainsKey($plugin)) {
            Write-Warning "Skipping $plugin - unknown clap name mapping"
            continue
        }

        $ver = Get-PluginVersion -CargoToml $cargoToml
        $clapName = $Names[$plugin]
        $clapPath = Join-Path $bundlesDir ($clapName + ".clap")
        $zipBase = ($clapName -replace '[()\s]+', '-').Trim('-')
        $zipName = "$zipBase-WGPU-v$ver-$suffix.zip"
        $zipPath = Join-Path $OutDir $zipName

        if (-not (Test-Path $clapPath)) {
            Write-Error "Bundle not found: $clapPath"
            continue
        }

        if (Test-Path $zipPath) { Remove-Item $zipPath -Force }
        Compress-Archive -Path $clapPath -DestinationPath $zipPath -Force
        Write-Host "  $zipName" -ForegroundColor Green
    }
}

foreach ($plat in $platforms) {
    Build-And-Package `
        -PluginList $Plugins `
        -Plat $plat `
        -Target $LinuxTarget `
        -Names $clapNames `
        -OutDir $distDir
}

$platLabel = $platforms -join "+"
Write-Host "=== Done: $distDir  [$platLabel] ===" -ForegroundColor Cyan
