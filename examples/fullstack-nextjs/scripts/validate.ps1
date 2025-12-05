# Validation script for fullstack-nextjs example (PowerShell)
# This script validates the example structure and configuration

$ErrorActionPreference = "Stop"

Write-Host "=== Validating fullstack-nextjs example ===" -ForegroundColor Cyan

# Get script directory
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ExampleDir = Split-Path -Parent $ScriptDir

Push-Location $ExampleDir

try {
    # Check workspace.yaml exists and is valid YAML
    Write-Host "`nChecking workspace.yaml..." -ForegroundColor Yellow
    if (-not (Test-Path "workspace.yaml")) {
        Write-Host "ERROR: workspace.yaml not found" -ForegroundColor Red
        exit 1
    }
    Write-Host "  workspace.yaml exists" -ForegroundColor Green

    # Check .xchecker/config.toml exists
    Write-Host "`nChecking .xchecker/config.toml..." -ForegroundColor Yellow
    if (-not (Test-Path ".xchecker/config.toml")) {
        Write-Host "ERROR: .xchecker/config.toml not found" -ForegroundColor Red
        exit 1
    }
    Write-Host "  .xchecker/config.toml exists" -ForegroundColor Green

    # Check spec directory structure
    Write-Host "`nChecking spec directory structure..." -ForegroundColor Yellow
    $SpecDir = ".xchecker/specs/task-manager"
    if (-not (Test-Path $SpecDir)) {
        Write-Host "ERROR: Spec directory not found: $SpecDir" -ForegroundColor Red
        exit 1
    }
    Write-Host "  Spec directory exists" -ForegroundColor Green

    # Check context directory
    $ContextDir = "$SpecDir/context"
    if (-not (Test-Path $ContextDir)) {
        Write-Host "ERROR: Context directory not found: $ContextDir" -ForegroundColor Red
        exit 1
    }
    Write-Host "  Context directory exists" -ForegroundColor Green

    # Check problem statement
    $ProblemStatement = "$ContextDir/problem-statement.md"
    if (-not (Test-Path $ProblemStatement)) {
        Write-Host "ERROR: Problem statement not found: $ProblemStatement" -ForegroundColor Red
        exit 1
    }
    Write-Host "  Problem statement exists" -ForegroundColor Green

    # Check README exists
    Write-Host "`nChecking README.md..." -ForegroundColor Yellow
    if (-not (Test-Path "README.md")) {
        Write-Host "ERROR: README.md not found" -ForegroundColor Red
        exit 1
    }
    Write-Host "  README.md exists" -ForegroundColor Green

    Write-Host "`n=== All validations passed ===" -ForegroundColor Green
    exit 0
}
finally {
    Pop-Location
}
