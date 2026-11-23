# Builds locally present consumers against a candidate Silvestris checkout.
#
# The check is opt-in. A consumer checkout that is absent is skipped with a
# message; sandcat is always skipped while its adoption is deferred.
#
# Each consumer is checked in a temporary git worktree so the consumer's own
# working tree, lockfile and Cargo configuration are not modified:
#
#   lotus   replace vendor/silvestris with the candidate tree, then
#           cargo check -p lotus-desktop
#   chaus   add a [patch] entry for the Silvestris git source, then
#           cargo check -p chaus-app
#   johjoh  same patch entry, then cargo check -p johjoh-gui
#
# Usage:
#   pwsh tools/consumers/check.ps1 -Candidate .
#   pwsh tools/consumers/check.ps1 -Candidate . -Consumer chaus
#   pwsh tools/consumers/check.ps1 -Candidate . -Root C:\path\to\checkouts
#
# On this Windows GNU host, GPUI's build script needs a windres on PATH. Pass
# -PrependPath with a directory that provides it, or set
# SILVESTRIS_WINDRES_SHIM. The check is skipped if windres is not found, since
# a missing tool is not a consumer failure.
param(
    [string]$Candidate = (Get-Location).Path,
    [string]$Root = "",
    [string[]]$Consumer = @("lotus", "chaus", "johjoh"),
    [string]$PrependPath = "",
    [switch]$SkipSubmodules
)

$ErrorActionPreference = "Stop"
$Candidate = (Resolve-Path $Candidate).Path
$crate = Join-Path $Candidate "crates/silvestris"
if (-not (Test-Path (Join-Path $crate "Cargo.toml"))) {
    throw "No Silvestris crate at $crate"
}
if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Split-Path -Parent $Candidate
}
if ([string]::IsNullOrWhiteSpace($PrependPath)) {
    $PrependPath = $env:SILVESTRIS_WINDRES_SHIM
}
if ($PrependPath) {
    $env:PATH = "$PrependPath;$env:PATH"
}

$specs = @{
    lotus  = @{ Path = "lotus"; Package = "lotus-desktop"; Vendored = $true }
    chaus  = @{ Path = "chaus"; Package = "chaus-app"; Vendored = $false }
    johjoh = @{ Path = "nintendo-homebrew/johjoh"; Package = "johjoh-gui"; Vendored = $false }
}

$results = [System.Collections.Generic.List[string]]::new()
$failed = $false

function Copy-Candidate([string]$Destination) {
    $Destination = New-Item -ItemType Directory -Force -Path $Destination
    Get-ChildItem -Force $Candidate | Where-Object {
        $_.Name -notin @(".git", "target")
    } | ForEach-Object {
        Copy-Item -Recurse -Force $_.FullName (Join-Path $Destination.FullName $_.Name)
    }
}

foreach ($name in $Consumer) {
    if ($name -eq "sandcat") {
        $results.Add("sandcat: skipped (adoption deferred)")
        continue
    }
    if (-not $specs.ContainsKey($name)) {
        throw "Unknown consumer '$name'"
    }
    $spec = $specs[$name]
    $repo = Join-Path $Root $spec.Path
    if (-not (Test-Path (Join-Path $repo "Cargo.toml"))) {
        $results.Add("${name}: skipped (no checkout at $repo)")
        continue
    }
    $worktree = Join-Path ([System.IO.Path]::GetTempPath()) "silvestris-check-$name-$([guid]::NewGuid().ToString('N'))"
    Write-Host "== $name"
    try {
        git -C $repo worktree add --detach $worktree HEAD | Out-Host
        if (-not $SkipSubmodules -and (Test-Path (Join-Path $worktree ".gitmodules"))) {
            git -C $worktree submodule update --init --recursive | Out-Host
        }
        $config = Join-Path $worktree ".cargo/config.toml"
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $config) | Out-Null
        if ($spec.Vendored) {
            $vendor = Join-Path $worktree "vendor/silvestris"
            Remove-Item -Recurse -Force $vendor -ErrorAction SilentlyContinue
            Copy-Candidate $vendor
            $lock = @("--locked")
        } else {
            $patch = $crate -replace '\\', '/'
            @"
[patch."https://github.com/estyseesghosts/silvestris"]
silvestris = { path = "$patch" }
"@ | Set-Content -Encoding utf8 $config
            $lock = @()
        }
        $arguments = @("check", "--manifest-path", (Join-Path $worktree "Cargo.toml"), "-p", $spec.Package)
        if (-not $spec.Vendored) {
            # Cargo discovers config from the working directory, not from
            # --manifest-path, so pass the patch file explicitly.
            $arguments += @("--config", $config)
        }
        $arguments += $lock
        & cargo @arguments
        if ($LASTEXITCODE -ne 0) {
            throw "cargo check failed for $name"
        }
        $results.Add("${name}: ok")
    } catch {
        $failed = $true
        $results.Add("${name}: FAILED ($($_.Exception.Message))")
    } finally {
        git -C $repo worktree remove --force $worktree 2>$null | Out-Null
        Remove-Item -Recurse -Force $worktree -ErrorAction SilentlyContinue
    }
}

Write-Host ""
$results | ForEach-Object { Write-Host $_ }
if ($failed) {
    exit 1
}
