[CmdletBinding()]
param(
    [switch]$DryRun,
    [switch]$Execute,
    [switch]$AllowDirty,
    [ValidateRange(1, [int]::MaxValue)]
    [int]$FromTier = 1,
    [ValidateRange(0, [int]::MaxValue)]
    [int]$IndexWaitSeconds = 30
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ($DryRun -and $Execute) {
    throw "Use either -DryRun or -Execute, not both."
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$WorkspaceRoot = Split-Path -Parent $ScriptDir
$TiersFile = Join-Path $ScriptDir "publish-tiers.txt"
$Mode = if ($Execute) { "execute" } elseif ($DryRun) { "dry-run" } else { "plan" }

$Tiers = Get-Content $TiersFile |
    Where-Object { $_.Trim() -and -not $_.TrimStart().StartsWith("#") }

if ($FromTier -gt $Tiers.Count) {
    throw "-FromTier exceeds the number of publish tiers ($($Tiers.Count))."
}

if ($Mode -eq "dry-run") {
    Write-Host "Running cargo publish --dry-run in tier order." -ForegroundColor Yellow
    Write-Host "Higher tiers will fail until lower tiers for this version are already indexed." -ForegroundColor Yellow
    Write-Host "Publishes use --locked by default." -ForegroundColor Yellow
}

for ($TierIndex = $FromTier - 1; $TierIndex -lt $Tiers.Count; $TierIndex++) {
    $TierNumber = $TierIndex + 1
    $TierCrates = $Tiers[$TierIndex]
    Write-Host "Tier $TierNumber`: $TierCrates" -ForegroundColor Cyan

    foreach ($Crate in ($TierCrates -split '\s+')) {
        if (-not $Crate) {
            continue
        }

        $Args = @("publish", "--locked", "-p", $Crate)
        if ($Mode -eq "dry-run") {
            $Args += "--dry-run"
        }
        if ($AllowDirty) {
            $Args += "--allow-dirty"
        }

        Write-Host "+ cargo $($Args -join ' ')"
        if ($Mode -ne "plan") {
            Push-Location $WorkspaceRoot
            try {
                & cargo @Args
            } finally {
                Pop-Location
            }
        }
    }

    if ($Mode -eq "execute" -and $TierIndex -lt ($Tiers.Count - 1) -and $IndexWaitSeconds -gt 0) {
        Write-Host "Waiting $IndexWaitSeconds seconds for crates.io indexing..." -ForegroundColor Yellow
        Start-Sleep -Seconds $IndexWaitSeconds
    }
}
