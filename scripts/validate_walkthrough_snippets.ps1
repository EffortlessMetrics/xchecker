# validate_walkthrough_snippets.ps1 - Extract and validate code snippets from walkthroughs
#
# This script extracts bash code snippets from walkthrough documentation and
# validates that the commands referenced are valid xchecker commands.
#
# Usage: .\scripts\validate_walkthrough_snippets.ps1

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

Write-Host "=== Validating Walkthrough Code Snippets ===" -ForegroundColor Cyan
Write-Host "Project root: $ProjectRoot"

$script:Errors = 0

# Valid xchecker subcommands
$ValidCommands = @("spec", "resume", "status", "clean", "doctor", "init", "benchmark", "test", "gate", "project", "template")

# Valid global flags (not subcommands)
$ValidGlobalFlags = @("--version", "--help", "-h", "-V")

function Extract-BashBlocks {
    param([string]$FilePath)

    $content = Get-Content $FilePath -Raw
    if ([string]::IsNullOrWhiteSpace($content)) {
        return @()
    }
    $pattern = '```bash\r?\n([\s\S]*?)```'
    $regexMatches = [regex]::Matches($content, $pattern)

    $blocks = @()
    foreach ($m in $regexMatches) {
        $blocks += $m.Groups[1].Value
    }
    return $blocks
}

function Validate-XCheckerCommands {
    param([string]$FilePath)

    Write-Host ""
    Write-Host "Checking commands: $FilePath" -ForegroundColor White

    $blocks = @(Extract-BashBlocks -FilePath $FilePath)

    if ($blocks.Count -eq 0) {
        Write-Host "  [SKIP] No bash code blocks found" -ForegroundColor Yellow
        return
    }

    $allContent = $blocks -join "`n"
    $lines = $allContent -split "`n"

    $xchecker_cmds = @($lines | Where-Object { $_ -match '^\s*xchecker\s+' })

    if ($xchecker_cmds.Count -eq 0) {
        Write-Host "  [SKIP] No xchecker commands found" -ForegroundColor Yellow
        return
    }

    foreach ($cmd in $xchecker_cmds) {
        # Skip empty lines and comments
        if ([string]::IsNullOrWhiteSpace($cmd) -or $cmd.Trim().StartsWith("#")) {
            continue
        }

        # Extract the subcommand
        $parts = $cmd.Trim() -split '\s+'
        if ($parts.Count -ge 2) {
            $subcmd = $parts[1]

            if ($ValidCommands -contains $subcmd) {
                Write-Host "  [PASS] Valid command: xchecker $subcmd" -ForegroundColor Green
            } elseif ($ValidGlobalFlags -contains $subcmd) {
                Write-Host "  [PASS] Valid flag: xchecker $subcmd" -ForegroundColor Green
            } else {
                Write-Host "  [FAIL] Unknown command: xchecker $subcmd" -ForegroundColor Red
                $script:Errors++
            }
        }
    }
}

function Check-InternalLinks {
    param([string]$FilePath)

    Write-Host ""
    Write-Host "Checking internal links: $FilePath" -ForegroundColor White

    $content = Get-Content $FilePath -Raw
    if ([string]::IsNullOrWhiteSpace($content)) {
        Write-Host "  [SKIP] File is empty" -ForegroundColor Yellow
        return
    }
    $pattern = '\[.*?\]\(([^)]+\.md(?:#[^)]*)?)\)'
    $regexMatches = [regex]::Matches($content, $pattern)

    if ($regexMatches.Count -eq 0) {
        Write-Host "  [SKIP] No internal links found" -ForegroundColor Yellow
        return
    }

    $dir = Split-Path -Parent $FilePath

    foreach ($m in $regexMatches) {
        $link = $m.Groups[1].Value

        # Strip anchor fragment (e.g. "FILE.md#section")
        $linkPath = ($link -split '#')[0]

        # Resolve relative path
        if ($linkPath.StartsWith("/")) {
            $target = Join-Path $ProjectRoot $linkPath.Substring(1)
        } else {
            $target = Join-Path $dir $linkPath
        }

        # Normalize path
        try {
            $target = [System.IO.Path]::GetFullPath($target)
        } catch {
            # Keep original if normalization fails
        }

        if (Test-Path $target -PathType Leaf) {
            Write-Host "  [PASS] Link exists: $link" -ForegroundColor Green
        } else {
            Write-Host "  [FAIL] Broken link: $link -> $target" -ForegroundColor Red
            $script:Errors++
        }
    }
}

function Check-JsonExamples {
    param([string]$FilePath)

    Write-Host ""
    Write-Host "Checking JSON examples: $FilePath" -ForegroundColor White

    $content = Get-Content $FilePath -Raw
    if ([string]::IsNullOrWhiteSpace($content)) {
        Write-Host "  [SKIP] File is empty" -ForegroundColor Yellow
        return
    }
    $pattern = '```json\r?\n([\s\S]*?)```'
    $regexMatches = [regex]::Matches($content, $pattern)

    if ($regexMatches.Count -eq 0) {
        Write-Host "  [SKIP] No JSON code blocks found" -ForegroundColor Yellow
        return
    }

    $blockCount = $regexMatches.Count
    Write-Host "  [PASS] Found $blockCount JSON block(s)" -ForegroundColor Green
}

# Main validation
Write-Host ""
Write-Host "=== Walkthrough Files ===" -ForegroundColor Cyan

# Discover walkthrough files: check both legacy locations and new locations
$WalkthroughFiles = @()

# Legacy locations
$legacyPaths = @(
    (Join-Path $ProjectRoot "docs" "WALKTHROUGH_20_MINUTES.md"),
    (Join-Path $ProjectRoot "docs" "WALKTHROUGH_SPEC_TO_PR.md")
)
foreach ($f in $legacyPaths) {
    if (Test-Path $f -PathType Leaf) {
        $WalkthroughFiles += $f
    }
}

# New Diataxis tutorial locations
$tutorialPaths = @(
    (Join-Path $ProjectRoot "docs" "tutorials" "QUICKSTART.md"),
    (Join-Path $ProjectRoot "docs" "tutorials" "SPEC_TO_PR.md")
)
foreach ($f in $tutorialPaths) {
    if (Test-Path $f -PathType Leaf) {
        $WalkthroughFiles += $f
    }
}

if ($WalkthroughFiles.Count -eq 0) {
    Write-Host "[FAIL] No walkthrough files found in docs/ or docs/tutorials/" -ForegroundColor Red
    exit 1
}

Write-Host "Found $($WalkthroughFiles.Count) walkthrough file(s)"

foreach ($file in $WalkthroughFiles) {
    Validate-XCheckerCommands -FilePath $file
    Check-InternalLinks -FilePath $file
    Check-JsonExamples -FilePath $file
}

# Summary
Write-Host ""
Write-Host "=== Summary ===" -ForegroundColor Cyan
if ($script:Errors -eq 0) {
    Write-Host "All walkthrough validations passed!" -ForegroundColor Green
    exit 0
} else {
    $errCount = $script:Errors
    Write-Host "FAILED: Found $errCount error(s)" -ForegroundColor Red
    exit 1
}
