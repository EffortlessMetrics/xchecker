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
$script:Warnings = 0

# Valid xchecker subcommands
$ValidCommands = @("spec", "resume", "status", "clean", "doctor", "init", "benchmark", "test", "gate", "project", "template")

# Valid global flags (not subcommands)
$ValidGlobalFlags = @("--version", "--help", "-h", "-V")

function Extract-BashBlocks {
    param([string]$FilePath)
    
    $content = Get-Content $FilePath -Raw
    $pattern = '```bash\r?\n([\s\S]*?)```'
    $matches = [regex]::Matches($content, $pattern)
    
    $blocks = @()
    foreach ($match in $matches) {
        $blocks += $match.Groups[1].Value
    }
    return $blocks
}

function Validate-XCheckerCommands {
    param([string]$FilePath)
    
    Write-Host ""
    Write-Host "Checking: $FilePath" -ForegroundColor White
    
    $blocks = Extract-BashBlocks -FilePath $FilePath
    
    if ($blocks.Count -eq 0) {
        Write-Host "  No bash code blocks found" -ForegroundColor Yellow
        return
    }
    
    $allContent = $blocks -join "`n"
    $lines = $allContent -split "`n"
    
    $xchecker_cmds = $lines | Where-Object { $_ -match '^\s*xchecker\s+' }
    
    if ($xchecker_cmds.Count -eq 0) {
        Write-Host "  No xchecker commands found" -ForegroundColor Yellow
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
                Write-Host "  OK Valid command: xchecker $subcmd" -ForegroundColor Green
            } elseif ($ValidGlobalFlags -contains $subcmd) {
                Write-Host "  OK Valid flag: xchecker $subcmd" -ForegroundColor Green
            } else {
                Write-Host "  FAIL Unknown command: xchecker $subcmd" -ForegroundColor Red
                $script:Errors++
            }
        }
    }
}

function Check-InternalLinks {
    param([string]$FilePath)
    
    Write-Host ""
    Write-Host "Checking internal links in: $FilePath" -ForegroundColor White
    
    $content = Get-Content $FilePath -Raw
    $pattern = '\[.*?\]\(([^)]+\.md)\)'
    $matches = [regex]::Matches($content, $pattern)
    
    if ($matches.Count -eq 0) {
        Write-Host "  No internal links found" -ForegroundColor Yellow
        return
    }
    
    $dir = Split-Path -Parent $FilePath
    
    foreach ($match in $matches) {
        $link = $match.Groups[1].Value
        
        # Resolve relative path
        if ($link.StartsWith("/")) {
            $target = Join-Path $ProjectRoot $link.Substring(1)
        } else {
            $target = Join-Path $dir $link
        }
        
        # Normalize path
        try {
            $target = [System.IO.Path]::GetFullPath($target)
        } catch {
            # Keep original if normalization fails
        }
        
        if (Test-Path $target) {
            Write-Host "  OK Link exists: $link" -ForegroundColor Green
        } else {
            Write-Host "  FAIL Broken link: $link" -ForegroundColor Red
            $script:Errors++
        }
    }
}

function Check-JsonExamples {
    param([string]$FilePath)
    
    Write-Host ""
    Write-Host "Checking JSON examples in: $FilePath" -ForegroundColor White
    
    $content = Get-Content $FilePath -Raw
    $pattern = '```json\r?\n([\s\S]*?)```'
    $matches = [regex]::Matches($content, $pattern)
    
    if ($matches.Count -eq 0) {
        Write-Host "  No JSON code blocks found" -ForegroundColor Yellow
        return
    }
    
    $blockCount = $matches.Count
    Write-Host "  Found $blockCount JSON block(s)"
    Write-Host "  OK JSON blocks present (manual validation recommended)" -ForegroundColor Green
}

# Main validation
Write-Host ""
Write-Host "=== Walkthrough Files ===" -ForegroundColor Cyan

$WalkthroughFiles = @(
    (Join-Path $ProjectRoot "docs\WALKTHROUGH_20_MINUTES.md"),
    (Join-Path $ProjectRoot "docs\WALKTHROUGH_SPEC_TO_PR.md")
)

foreach ($file in $WalkthroughFiles) {
    if (Test-Path $file) {
        Validate-XCheckerCommands -FilePath $file
        Check-InternalLinks -FilePath $file
        Check-JsonExamples -FilePath $file
    } else {
        Write-Host "File not found: $file" -ForegroundColor Red
        $script:Errors++
    }
}

# Summary
Write-Host ""
Write-Host "=== Summary ===" -ForegroundColor Cyan
if ($script:Errors -eq 0) {
    Write-Host "All walkthrough validations passed!" -ForegroundColor Green
    exit 0
} else {
    $errCount = $script:Errors
    Write-Host "Found $errCount error(s)" -ForegroundColor Red
    exit 1
}
