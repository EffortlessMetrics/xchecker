# Validation script for fullstack-nextjs example (PowerShell)
# This script validates the example structure and configuration.
# Designed to run in CI (GitHub Actions) on Windows.

$ErrorActionPreference = "Stop"

Write-Host "=== Validating fullstack-nextjs example ===" -ForegroundColor Cyan

# Get script directory
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ExampleDir = Split-Path -Parent $ScriptDir

Write-Host "Example directory: $ExampleDir"

Push-Location $ExampleDir

$script:Errors = 0

function Check-FileExists {
    param(
        [string]$Path,
        [string]$Label
    )
    if (Test-Path $Path -PathType Leaf) {
        Write-Host "  [PASS] $Label exists: $Path" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] $Label not found: $Path" -ForegroundColor Red
        $script:Errors++
    }
}

function Check-DirExists {
    param(
        [string]$Path,
        [string]$Label
    )
    if (Test-Path $Path -PathType Container) {
        Write-Host "  [PASS] $Label exists: $Path" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] $Label not found: $Path" -ForegroundColor Red
        $script:Errors++
    }
}

try {
    # Check workspace.yaml exists
    Write-Host "`nChecking workspace.yaml..." -ForegroundColor Yellow
    Check-FileExists -Path "workspace.yaml" -Label "workspace.yaml"

    # Check .xchecker/config.toml exists
    Write-Host "`nChecking .xchecker/config.toml..." -ForegroundColor Yellow
    Check-FileExists -Path (Join-Path ".xchecker" "config.toml") -Label ".xchecker/config.toml"

    # Check spec directory structure
    Write-Host "`nChecking spec directory structure..." -ForegroundColor Yellow
    $SpecDir = Join-Path ".xchecker" "specs" "task-manager"
    Check-DirExists -Path $SpecDir -Label "Spec directory"

    # Check context directory
    $ContextDir = Join-Path $SpecDir "context"
    Check-DirExists -Path $ContextDir -Label "Context directory"

    # Check problem statement
    Check-FileExists -Path (Join-Path $ContextDir "problem-statement.md") -Label "Problem statement"

    # Check README exists
    Write-Host "`nChecking README.md..." -ForegroundColor Yellow
    Check-FileExists -Path "README.md" -Label "README.md"

    # Summary
    Write-Host ""
    if ($script:Errors -eq 0) {
        Write-Host "=== All validations passed ===" -ForegroundColor Green
        exit 0
    } else {
        Write-Host "=== FAILED: $($script:Errors) error(s) found ===" -ForegroundColor Red
        exit 1
    }
}
finally {
    Pop-Location
}
