# Performance Guide

This document describes xchecker's performance characteristics, benchmarking methodology, and optimization strategies.

## Performance Requirements (NFR1)

xchecker is designed to meet the following performance targets:

| Operation | Target | Validation Method |
|-----------|--------|-------------------|
| Empty run (`--dry-run`) | ≤ 5s | `xchecker benchmark` |
| Packetization (100 files) | ≤ 200ms | `xchecker benchmark --file-count 100` |
| JCS emission | ≤ 50ms | Internal benchmarking |

## Benchmarking

### Running Benchmarks

```bash
# Run default benchmarks
xchecker benchmark

# Custom file count
xchecker benchmark --file-count 100

# Custom file size
xchecker benchmark --file-size 2048

# Custom iterations
xchecker benchmark --iterations 10

# Full custom benchmark
xchecker benchmark --file-count 50 --file-size 1024 --iterations 5
```

### Benchmark Output

```json
{
  "schema_version": "1",
  "emitted_at": "2025-11-27T12:00:00Z",
  "ok": true,
  "timings_ms": {
    "dry_run": 2341.5,
    "packetization_100_files": 156.3,
    "jcs_emission": 23.7
  },
  "memory": {
    "rss_mb": 45.2,
    "commit_mb": 52.1
  }
}
```

### Benchmark Methodology

1. **Warm-up Pass**: One iteration to warm caches and JIT
2. **Measured Runs**: N≥3 iterations (default 5)
3. **Median Calculation**: Report median to reduce outlier impact
4. **Process-Scoped Memory**: RSS (all OSs) and commit (Windows only)

## Performance Characteristics

### Packet Assembly

**Factors Affecting Performance:**
- Number of files in workspace
- Total size of included files
- Number of glob patterns (include/exclude)
- File system performance (local vs network)
- InsightCache hit rate

**Optimization Strategies:**
- Use specific include patterns to reduce file scanning
- Exclude large directories (e.g., `target/`, `node_modules/`)
- Enable InsightCache for repeated runs
- Use SSD storage for better I/O performance

**Example Configuration:**
```toml
[selectors]
# Specific patterns reduce scanning time
include = [
    "src/**/*.rs",
    "Cargo.toml",
    "README.md"
]

# Exclude large directories
exclude = [
    "target/**",
    "node_modules/**",
    ".git/**"
]
```

### JCS Canonicalization

**Performance Characteristics:**
- O(n log n) for key sorting
- O(n) for value normalization
- Deterministic output guarantees stable diffs

**Optimization:**
- Uses `serde_json_canonicalizer` for RFC 8785 compliance
- Minimal allocations through efficient serialization
- Cached canonicalization for repeated structures

### BLAKE3 Hashing

**Performance Characteristics:**
- Extremely fast (>1 GB/s on modern CPUs)
- Parallel hashing for large files
- Stable across platforms with LF line endings

**Implementation:**
- Uses `blake3` crate with SIMD optimizations
- Computes on canonicalized content for determinism
- Full 64-character hex output (not truncated)

### InsightCache (NFR7)

**Performance Targets:**
- Cache hit rate: >70% on repeated runs
- Cache validation: <10ms per file
- Packet assembly speedup: >50% for large codebases

**Cache Strategy:**
- BLAKE3 content hash as cache key
- Size and mtime validation for quick invalidation
- Memory + disk persistence for cross-run caching
- Phase-specific insights (10-25 bullet points)

**Cache Performance:**
```bash
# First run (cold cache)
$ time xchecker spec my-feature --dry-run
real    0m4.523s

# Second run (warm cache)
$ time xchecker spec my-feature --dry-run
real    0m1.847s  # 59% faster
```

## Memory Usage

### Process Memory Tracking

xchecker tracks process-scoped memory (not system totals):

- **RSS (Resident Set Size)**: Physical memory used by process
- **Commit (Windows only)**: Virtual memory committed

### Memory Characteristics

**Typical Memory Usage:**
- Base process: ~20-30 MB
- Packet assembly: +10-20 MB per 100 files
- InsightCache: +5-10 MB per 1000 cached files
- Peak during Claude execution: ~50-100 MB

**Memory Optimization:**
- Ring buffers cap stdout (2 MiB) and stderr (256 KiB)
- Streaming packet assembly (no full-file buffering)
- Efficient BLAKE3 hashing (minimal allocations)
- Cache eviction for stale entries

## Platform-Specific Performance

### Linux

**Best Performance:**
- Native execution (no WSL overhead)
- Excellent file system performance
- SIMD optimizations for BLAKE3

**Typical Benchmarks:**
- Dry run: 1.5-2.5s
- Packetization (100 files): 80-120ms
- JCS emission: 15-25ms

### macOS

**Good Performance:**
- Native execution
- Good file system performance
- SIMD optimizations for BLAKE3

**Typical Benchmarks:**
- Dry run: 2.0-3.0s
- Packetization (100 files): 100-150ms
- JCS emission: 20-30ms

### Windows (Native)

**Good Performance:**
- Native execution
- Windows-specific optimizations (Job Objects)
- SIMD optimizations for BLAKE3

**Typical Benchmarks:**
- Dry run: 2.5-3.5s
- Packetization (100 files): 120-180ms
- JCS emission: 25-35ms

**Windows-Specific Considerations:**
- Antivirus scanning can slow file operations
- Windows Defender may scan new files
- Retry logic handles transient locks (≤250ms)

### Windows (WSL)

**Moderate Performance:**
- WSL translation overhead
- Cross-filesystem performance impact
- Path translation adds latency

**Typical Benchmarks:**
- Dry run: 3.0-4.5s
- Packetization (100 files): 150-200ms
- JCS emission: 25-35ms

**WSL-Specific Considerations:**
- Use WSL2 for better performance
- Keep files in WSL filesystem when possible
- Path translation uses `wslpath` for correctness

## Optimization Strategies

### 1. Reduce File Scanning

**Problem:** Scanning large directories slows packet assembly.

**Solution:**
```toml
[selectors]
# Be specific about what to include
include = ["src/**/*.rs", "Cargo.toml"]

# Exclude large directories
exclude = ["target/**", "node_modules/**"]
```

### 2. Enable InsightCache

**Problem:** Repeated runs reprocess unchanged files.

**Solution:**
```bash
# Cache is enabled by default
# Verify cache hits in verbose mode
xchecker spec my-feature --verbose
```

### 3. Adjust Packet Limits

**Problem:** Large packets slow Claude invocation.

**Solution:**
```bash
# Reduce packet size for faster iteration
xchecker spec my-feature --packet-max-bytes 32768 --packet-max-lines 800
```

### 4. Use Dry Run for Testing

**Problem:** Full Claude runs are slow during development.

**Solution:**
```bash
# Test configuration without Claude calls
xchecker spec my-feature --dry-run
```

### 5. Optimize File System

**Problem:** Network or slow file systems impact performance.

**Solution:**
- Use local SSD storage
- Avoid network file systems for `.xchecker/`
- On Windows, exclude `.xchecker/` from antivirus scanning

### 6. Tune Timeout Values

**Problem:** Long timeouts delay error detection.

**Solution:**
```bash
# Reduce timeout for faster failure detection
xchecker spec my-feature --phase-timeout 300
```

## Performance Monitoring

### Verbose Logging

Enable verbose logging to see performance details:

```bash
xchecker spec my-feature --verbose
```

**Logged Metrics:**
- File selection time
- Packet assembly time
- BLAKE3 hashing time
- Cache hit/miss statistics
- Claude execution time
- Receipt writing time

### Benchmark Regression Detection

Run benchmarks in CI to detect performance regressions:

```yaml
# GitHub Actions example
- name: Run performance benchmarks
  run: |
    xchecker benchmark --json > benchmark.json
    # Fail if any target exceeded
    jq -e '.ok == true' benchmark.json
```

### Profiling

For detailed profiling, use standard Rust profiling tools:

```bash
# CPU profiling with perf (Linux)
perf record --call-graph dwarf xchecker spec my-feature --dry-run
perf report

# Memory profiling with valgrind
valgrind --tool=massif xchecker spec my-feature --dry-run
ms_print massif.out.*

# Flamegraph generation
cargo flamegraph --bin xchecker -- spec my-feature --dry-run
```

## Performance Troubleshooting

### Slow Packet Assembly

**Symptoms:**
- Packetization takes >200ms for 100 files
- High CPU usage during file scanning

**Diagnosis:**
```bash
xchecker spec my-feature --verbose --dry-run
# Look for "File selection" and "Packet assembly" timings
```

**Solutions:**
1. Reduce include patterns
2. Add more exclude patterns
3. Check file system performance
4. Verify InsightCache is working

### Slow Dry Run

**Symptoms:**
- `--dry-run` takes >5s
- High memory usage

**Diagnosis:**
```bash
xchecker benchmark --json
# Check "dry_run" timing
```

**Solutions:**
1. Reduce number of files in workspace
2. Optimize glob patterns
3. Check for slow file system
4. Verify no antivirus interference (Windows)

### High Memory Usage

**Symptoms:**
- Process uses >200 MB RSS
- Out of memory errors

**Diagnosis:**
```bash
xchecker benchmark --json
# Check "memory.rss_mb"
```

**Solutions:**
1. Reduce packet size limits
2. Clear InsightCache: `rm -rf .xchecker/cache/`
3. Reduce buffer sizes: `--stdout-cap-bytes 1048576`
4. Check for memory leaks (report issue)

### WSL Performance Issues

**Symptoms:**
- WSL mode significantly slower than native
- High latency for file operations

**Diagnosis:**
```bash
# Compare native vs WSL
xchecker benchmark --runner-mode native
xchecker benchmark --runner-mode wsl
```

**Solutions:**
1. Use WSL2 instead of WSL1
2. Keep files in WSL filesystem
3. Use native mode if Claude CLI available
4. Optimize WSL configuration

## Performance Best Practices

1. **Use Specific Patterns**: Prefer specific include patterns over broad exclusions
2. **Enable Caching**: Let InsightCache work (enabled by default)
3. **Optimize File System**: Use local SSD storage
4. **Monitor Benchmarks**: Run `xchecker benchmark` regularly
5. **Profile in CI**: Detect regressions early
6. **Tune for Workload**: Adjust limits based on actual usage
7. **Use Dry Run**: Test configuration changes quickly
8. **Exclude Large Dirs**: Always exclude `target/`, `node_modules/`, `.git/`

## Performance Roadmap

### Completed Optimizations
- ✅ InsightCache for file reuse
- ✅ Ring buffers for memory efficiency
- ✅ BLAKE3 for fast hashing
- ✅ JCS canonicalization optimization
- ✅ Priority-based packet assembly
- ✅ Process-scoped memory tracking

### Future Optimizations
- ⏳ Parallel file hashing
- ⏳ Incremental packet assembly
- ⏳ Compressed cache storage
- ⏳ Streaming JCS emission
- ⏳ Memory-mapped file reading
- ⏳ Async I/O for file operations

## References

- [NFR1 Performance Requirements](../requirements.md#nfr1-performance)
- [NFR7 Caching Efficiency](../requirements.md#nfr7-caching-efficiency)
- [Benchmark Implementation](../src/benchmark.rs)
- [InsightCache Implementation](../src/cache.rs)
- [PacketBuilder Implementation](../src/packet.rs)

## See Also

- [CONFIGURATION.md](CONFIGURATION.md) - Performance-related configuration options
- [SECURITY.md](SECURITY.md) - Security scanning performance considerations
- [INDEX.md](INDEX.md) - Documentation index
