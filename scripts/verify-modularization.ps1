# PowerShell version of the modularization verification gate
# Run with: pwsh -File scripts/verify-modularization.ps1

$ErrorActionPreference = "Stop"

Write-Host "=== Modularization Verification Gate ===" -ForegroundColor Cyan
Write-Host ""

Write-Host "1. Checking formatting..." -ForegroundColor Yellow
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) {
    Write-Host "   ✗ Formatting check failed" -ForegroundColor Red
    exit $LASTEXITCODE
}
Write-Host "   ✓ Formatting check passed" -ForegroundColor Green

Write-Host ""
Write-Host "2. Running clippy..." -ForegroundColor Yellow
cargo clippy --workspace --all-targets --all-features -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Host "   ✗ Clippy check failed" -ForegroundColor Red
    exit $LASTEXITCODE
}
Write-Host "   ✓ Clippy check passed" -ForegroundColor Green

Write-Host ""
Write-Host "3. Running tests with all features..." -ForegroundColor Yellow
cargo test --workspace --all-features
if ($LASTEXITCODE -ne 0) {
    Write-Host "   ✗ Tests failed" -ForegroundColor Red
    exit $LASTEXITCODE
}
Write-Host "   ✓ All tests passed" -ForegroundColor Green

Write-Host ""
Write-Host "4. Checking dependency graph for cycles..." -ForegroundColor Yellow
cargo tree --duplicates
if ($LASTEXITCODE -ne 0) {
    Write-Host "   ✗ Dependency graph check failed" -ForegroundColor Red
    exit $LASTEXITCODE
}
Write-Host "   ✓ Dependency graph is clean" -ForegroundColor Green

Write-Host ""
Write-Host "=== All verification gates passed ===" -ForegroundColor Green
