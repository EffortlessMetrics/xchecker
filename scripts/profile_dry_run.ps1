# Profile dry-run performance for NFR1 validation
# This script measures the performance of various operations during a dry-run

Write-Host "=== Dry-Run Performance Profiling ===" -ForegroundColor Cyan
Write-Host ""

# Build in release mode first
Write-Host "Building in release mode..." -ForegroundColor Yellow
cargo build --release 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "Build complete" -ForegroundColor Green
Write-Host ""

# Test 1: Basic dry-run (no prior state)
Write-Host "Test 1: Basic dry-run (clean state)" -ForegroundColor Yellow
$spec_id = "profile-test-$(Get-Date -Format 'yyyyMMdd-HHmmss')"
$iterations = 5

$timings = @()
for ($i = 1; $i -le $iterations; $i++) {
    Write-Host "  Run $i/$iterations..." -NoNewline
    $timing = Measure-Command {
        cargo run --release -- spec $spec_id --dry-run 2>&1 | Out-Null
    }
    $timings += $timing.TotalMilliseconds
    $roundedTime = [math]::Round($timing.TotalMilliseconds, 1)
    Write-Host " ${roundedTime}ms" -ForegroundColor Gray
    
    # Clean up for next run
    if (Test-Path ".xchecker/specs/$spec_id") {
        Remove-Item -Recurse -Force ".xchecker/specs/$spec_id" 2>&1 | Out-Null
    }
}

$median = ($timings | Sort-Object)[[math]::Floor($timings.Count / 2)]
$avg = ($timings | Measure-Object -Average).Average
$min = ($timings | Measure-Object -Minimum).Minimum
$max = ($timings | Measure-Object -Maximum).Maximum

Write-Host ""
Write-Host "Results:" -ForegroundColor Cyan
$roundedMedian = [math]::Round($median, 1)
$roundedAvg = [math]::Round($avg, 1)
$roundedMin = [math]::Round($min, 1)
$roundedMax = [math]::Round($max, 1)
Write-Host "  Median: ${roundedMedian}ms"
Write-Host "  Average: ${roundedAvg}ms"
Write-Host "  Min: ${roundedMin}ms"
Write-Host "  Max: ${roundedMax}ms"
Write-Host "  Target: 5000ms (5s)"

if ($median -le 5000) {
    Write-Host "  Status: PASS" -ForegroundColor Green
} else {
    Write-Host "  Status: FAIL" -ForegroundColor Red
}
Write-Host ""

# Test 2: Config loading performance
Write-Host "Test 2: Config loading (with .xchecker/config.toml)" -ForegroundColor Yellow

# Create a config file
$config_dir = ".xchecker"
if (!(Test-Path $config_dir)) {
    New-Item -ItemType Directory -Path $config_dir | Out-Null
}

$configPath = Join-Path $config_dir "config.toml"
$configLines = @(
    "[runner]",
    "mode = ""native""",
    "phase_timeout = 600",
    "",
    "[packet]",
    "max_bytes = 65536",
    "max_lines = 1200",
    "",
    "[secrets]",
    "extra_patterns = []",
    "ignore_patterns = []"
)
$configLines | Out-File -FilePath $configPath -Encoding utf8

$config_timings = @()
for ($i = 1; $i -le $iterations; $i++) {
    Write-Host "  Run $i/$iterations..." -NoNewline
    $timing = Measure-Command {
        cargo run --release -- spec $spec_id --dry-run 2>&1 | Out-Null
    }
    $config_timings += $timing.TotalMilliseconds
    $roundedTime = [math]::Round($timing.TotalMilliseconds, 1)
    Write-Host " ${roundedTime}ms" -ForegroundColor Gray
    
    # Clean up for next run
    if (Test-Path ".xchecker/specs/$spec_id") {
        Remove-Item -Recurse -Force ".xchecker/specs/$spec_id" 2>&1 | Out-Null
    }
}

$config_median = ($config_timings | Sort-Object)[[math]::Floor($config_timings.Count / 2)]
Write-Host ""
Write-Host "Results with config:" -ForegroundColor Cyan
$roundedConfigMedian = [math]::Round($config_median, 1)
$overhead = [math]::Round($config_median - $median, 1)
Write-Host "  Median: ${roundedConfigMedian}ms"
Write-Host "  Overhead: ${overhead}ms"
Write-Host ""

# Clean up config
Remove-Item -Path $configPath -Force 2>&1 | Out-Null

# Test 3: Status command performance
Write-Host "Test 3: Status command (with existing spec)" -ForegroundColor Yellow

# Create a spec with some artifacts
cargo run --release -- spec $spec_id --dry-run 2>&1 | Out-Null

$status_timings = @()
for ($i = 1; $i -le $iterations; $i++) {
    Write-Host "  Run $i/$iterations..." -NoNewline
    $timing = Measure-Command {
        cargo run --release -- status $spec_id --json 2>&1 | Out-Null
    }
    $status_timings += $timing.TotalMilliseconds
    $roundedTime = [math]::Round($timing.TotalMilliseconds, 1)
    Write-Host " ${roundedTime}ms" -ForegroundColor Gray
}

$status_median = ($status_timings | Sort-Object)[[math]::Floor($status_timings.Count / 2)]
Write-Host ""
Write-Host "Results:" -ForegroundColor Cyan
$roundedStatusMedian = [math]::Round($status_median, 1)
Write-Host "  Median: ${roundedStatusMedian}ms"
Write-Host ""

# Clean up
if (Test-Path ".xchecker/specs/$spec_id") {
    Remove-Item -Recurse -Force ".xchecker/specs/$spec_id" 2>&1 | Out-Null
}

# Test 4: Artifact enumeration performance
Write-Host "Test 4: Artifact enumeration (multiple artifacts)" -ForegroundColor Yellow

# Create a spec with multiple phases
cargo run --release -- spec $spec_id --dry-run 2>&1 | Out-Null

# Create some dummy artifacts
$artifacts_dir = ".xchecker/specs/$spec_id/artifacts"
if (Test-Path $artifacts_dir) {
    for ($i = 1; $i -le 10; $i++) {
        $content = "# Test Artifact $i`n`nContent here"
        Set-Content -Path "$artifacts_dir/test-artifact-$i.md" -Value $content
    }
}

$enum_timings = @()
for ($i = 1; $i -le $iterations; $i++) {
    Write-Host "  Run $i/$iterations..." -NoNewline
    $timing = Measure-Command {
        cargo run --release -- status $spec_id --json 2>&1 | Out-Null
    }
    $enum_timings += $timing.TotalMilliseconds
    $roundedTime = [math]::Round($timing.TotalMilliseconds, 1)
    Write-Host " ${roundedTime}ms" -ForegroundColor Gray
}

$enum_median = ($enum_timings | Sort-Object)[[math]::Floor($enum_timings.Count / 2)]
Write-Host ""
Write-Host "Results:" -ForegroundColor Cyan
$roundedEnumMedian = [math]::Round($enum_median, 1)
$enumOverhead = [math]::Round($enum_median - $status_median, 1)
Write-Host "  Median: ${roundedEnumMedian}ms"
Write-Host "  Overhead vs empty: ${enumOverhead}ms"
Write-Host ""

# Clean up
if (Test-Path ".xchecker/specs/$spec_id") {
    Remove-Item -Recurse -Force ".xchecker/specs/$spec_id" 2>&1 | Out-Null
}

# Summary
Write-Host "=== Performance Summary ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "Operation                      Median Time   Target    Status"
Write-Host "------------------------------------------------------------"
$medianStr = [math]::Round($median, 1).ToString().PadLeft(7)
$status1 = if ($median -le 5000) { "PASS" } else { "FAIL" }
Write-Host "Dry-run (clean)                ${medianStr}ms    5000ms    $status1"

$configMedianStr = [math]::Round($config_median, 1).ToString().PadLeft(7)
$status2 = if ($config_median -le 5000) { "PASS" } else { "FAIL" }
Write-Host "Dry-run (with config)          ${configMedianStr}ms    5000ms    $status2"

$statusMedianStr = [math]::Round($status_median, 1).ToString().PadLeft(7)
Write-Host "Status (empty spec)            ${statusMedianStr}ms    N/A       N/A"

$enumMedianStr = [math]::Round($enum_median, 1).ToString().PadLeft(7)
Write-Host "Status (with artifacts)        ${enumMedianStr}ms    N/A       N/A"
Write-Host ""

# Overall assessment
$all_pass = ($median -le 5000) -and ($config_median -le 5000)
if ($all_pass) {
    Write-Host "Overall: ALL TARGETS MET" -ForegroundColor Green
} else {
    Write-Host "Overall: SOME TARGETS NOT MET" -ForegroundColor Red
}
Write-Host ""

# Performance breakdown
Write-Host "=== Performance Breakdown ===" -ForegroundColor Cyan
Write-Host ""
$configOverhead = [math]::Round($config_median - $median, 1)
$artifactOverhead = [math]::Round($enum_median - $status_median, 1)
Write-Host "Config loading overhead:         ${configOverhead}ms"
Write-Host "Artifact enumeration overhead:   ${artifactOverhead}ms"
Write-Host ""

# Recommendations
Write-Host "=== Recommendations ===" -ForegroundColor Cyan
Write-Host ""
if ($median -lt 200) {
    Write-Host "Dry-run performance is excellent (<200ms)" -ForegroundColor Green
} elseif ($median -lt 1000) {
    Write-Host "Dry-run performance is good (<1s)" -ForegroundColor Green
} elseif ($median -lt 5000) {
    Write-Host "Dry-run performance meets target but could be improved" -ForegroundColor Yellow
} else {
    Write-Host "Dry-run performance needs optimization" -ForegroundColor Red
}

if ($configOverhead -gt 50) {
    Write-Host "Config loading adds significant overhead (>50ms)" -ForegroundColor Yellow
    Write-Host "  Consider caching or optimizing config parsing" -ForegroundColor Gray
}

if ($artifactOverhead -gt 50) {
    Write-Host "Artifact enumeration adds significant overhead (>50ms)" -ForegroundColor Yellow
    Write-Host "  Consider optimizing file system operations" -ForegroundColor Gray
}

Write-Host ""
Write-Host "Profiling complete!" -ForegroundColor Green
