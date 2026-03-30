[CmdletBinding()]
param(
    [switch]$DryRun,
    [switch]$Execute,
    [switch]$AllowDirty,
    [switch]$SkipPublished,
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
$CargoToml = Join-Path $WorkspaceRoot "Cargo.toml"

function Get-WorkspaceVersion {
    param([string]$ManifestPath)

    $InWorkspacePackage = $false
    foreach ($Line in Get-Content $ManifestPath) {
        if ($Line -match '^\s*\[(.+)\]\s*$') {
            $InWorkspacePackage = $Matches[1] -eq 'workspace.package'
            continue
        }

        if ($InWorkspacePackage -and $Line -match '^\s*version\s*=\s*"([^"]+)"') {
            return $Matches[1]
        }
    }

    throw "Could not determine [workspace.package].version from $ManifestPath."
}

function Test-CrateVersionPublished {
    param(
        [string]$Crate,
        [string]$Version
    )

    $Uri = "https://crates.io/api/v1/crates/$Crate"
    try {
        $Response = Invoke-RestMethod -Uri $Uri -Headers @{ 'User-Agent' = 'xchecker-release-script' } -TimeoutSec 30
    } catch {
        $StatusCode = $null
        if ($_.Exception.Response -and $_.Exception.Response.StatusCode) {
            $StatusCode = [int]$_.Exception.Response.StatusCode
        }

        if ($StatusCode -eq 404) {
            return $false
        }

        throw
    }

    return @($Response.versions | Where-Object { $_.num -eq $Version }).Count -gt 0
}

function Get-RetryAfterSeconds {
    param([string]$OutputText)

    if ($OutputText -match '(?i)try again after (?<RetryAfter>.+?)(?: and| or|\.|$)') {
        $RetryAfter = [DateTimeOffset]::Parse(
            $Matches['RetryAfter'],
            [System.Globalization.CultureInfo]::InvariantCulture
        )
        $Delay = [Math]::Ceiling(($RetryAfter - [DateTimeOffset]::UtcNow).TotalSeconds)
        return [Math]::Max(1, [int]$Delay)
    }

    return $null
}

function Invoke-CargoWithRetry {
    param(
        [string[]]$CargoArgs,
        [string]$Crate,
        [string]$Mode,
        [int]$IndexWaitSeconds,
        [bool]$SkipPublished
    )

    while ($true) {
        $Output = @(& cargo @CargoArgs 2>&1)
        $ExitCode = $LASTEXITCODE
        $Lines = @($Output | ForEach-Object { $_.ToString() })

        foreach ($Line in $Lines) {
            Write-Host $Line
        }

        if ($ExitCode -eq 0) {
            return
        }

        $OutputText = $Lines -join "`n"

        if ($SkipPublished -and $OutputText -match 'already uploaded') {
            Write-Host "Skipping $Crate because $Crate $WorkspaceVersion is already published." -ForegroundColor Yellow
            return
        }

        if ($Mode -eq "execute") {
            $RetryAfterSeconds = Get-RetryAfterSeconds -OutputText $OutputText
            if ($null -ne $RetryAfterSeconds) {
                Write-Host "Rate limited publishing $Crate; retrying in $RetryAfterSeconds seconds." -ForegroundColor Yellow
                Start-Sleep -Seconds $RetryAfterSeconds
                continue
            }

            if ($IndexWaitSeconds -gt 0 -and $OutputText -match 'no matching package named') {
                Write-Host "Waiting $IndexWaitSeconds seconds for crates.io indexing before retrying $Crate..." -ForegroundColor Yellow
                Start-Sleep -Seconds $IndexWaitSeconds
                continue
            }
        }

        throw "cargo $($CargoArgs -join ' ') failed with exit code $ExitCode."
    }
}

$WorkspaceVersion = Get-WorkspaceVersion -ManifestPath $CargoToml

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

if ($SkipPublished) {
    Write-Host "Skipping crates that already have version $WorkspaceVersion on crates.io." -ForegroundColor Yellow
}

for ($TierIndex = $FromTier - 1; $TierIndex -lt $Tiers.Count; $TierIndex++) {
    $TierNumber = $TierIndex + 1
    $TierCrates = $Tiers[$TierIndex]
    Write-Host "Tier $TierNumber`: $TierCrates" -ForegroundColor Cyan

    foreach ($Crate in ($TierCrates -split '\s+')) {
        if (-not $Crate) {
            continue
        }

        if ($SkipPublished -and (Test-CrateVersionPublished -Crate $Crate -Version $WorkspaceVersion)) {
            Write-Host "- $Crate $WorkspaceVersion already published; skipping." -ForegroundColor Yellow
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
                Invoke-CargoWithRetry -CargoArgs $Args -Crate $Crate -Mode $Mode -IndexWaitSeconds $IndexWaitSeconds -SkipPublished $SkipPublished
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
