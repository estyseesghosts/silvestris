# Renders every golden appearance and scale, then optionally accepts the
# result. Requires the `capture` feature, which enables GPUI's test-support
# readback. Output defaults to target/goldens and is not committed.
#
#   pwsh tools/metrology/render.ps1
#   pwsh tools/metrology/render.ps1 -Out target\goldens -Appearance aqua-light
#   pwsh tools/metrology/render.ps1 -Accept -Reason "raise the checkbox tick"
param(
    [string]$Out = "target/goldens",
    [string[]]$Appearance = @("aqua-light", "aqua-dark", "geist-light", "geist-dark"),
    [string[]]$Scale = @("1.0", "1.5", "2.0"),
    [string]$PrependPath = "",
    [switch]$Accept,
    [string]$Reason = "",
    [string]$Python = "python"
)

$ErrorActionPreference = "Stop"
if ([string]::IsNullOrWhiteSpace($PrependPath)) {
    $PrependPath = $env:SILVESTRIS_WINDRES_SHIM
}
if ($PrependPath) {
    $env:PATH = "$PrependPath;$env:PATH"
}
$percent = @{ "1.0" = "100"; "1.5" = "150"; "2.0" = "200" }

foreach ($theme in $Appearance) {
    foreach ($factor in $Scale) {
        $target = Join-Path $Out "$theme\$($percent[$factor])"
        Write-Host "== $theme $($percent[$factor])%"
        & cargo run -q -p silvestris-demo --features capture --bin capture -- $theme $factor $target
        if ($LASTEXITCODE -ne 0) {
            throw "capture failed for $theme $factor"
        }
    }
}

if ($Accept) {
    if ([string]::IsNullOrWhiteSpace($Reason)) {
        throw "-Accept requires -Reason"
    }
    & $Python tools/metrology/gate.py --candidate $Out --accept --reason $Reason
    if ($LASTEXITCODE -ne 0) {
        throw "gate accept failed"
    }
} else {
    Write-Host "Renders in $Out. Compare with:"
    Write-Host "  $Python tools/metrology/gate.py --candidate $Out"
}
