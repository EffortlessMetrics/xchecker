# Bolt's Journal

## 2026-01-23 - [GlobSet Optimization]

**Learning:** Checking a file path against multiple `GlobSet`s sequentially is less efficient than combining them into a single `GlobSet` and checking match indices. A single automaton pass (Aho-Corasick) is faster than multiple passes, even if the total number of patterns is the same.
**Action:** When classifying strings against multiple disjoint sets of patterns, combine them into one `GlobSet` (or `RegexSet`) and use the match index to map back to the classification.

## 2024-05-22 - [Dependency Policy Violation]
**Learning:** Adding new dependencies (even dev-dependencies or transient ones like `rayon`) requires explicit approval.
**Action:** Always check `Cargo.toml` constraints and ask before adding deps. Use `std::thread::scope` for simple parallelism instead.

## 2024-05-22 - [Optimized File Reading]
**Learning:** In high-throughput file processing (like packet assembly), using `fs::metadata(path)` followed by `fs::read_to_string(path)` introduces a TOCTOU race condition and redundant syscalls (resolving the path twice).
**Action:** Use `fs::File::open(path)` to get a handle, then `file.metadata()` (fstat) and `file.read_to_string()` to read content. This is faster and safer. Also pre-allocate buffers using `String::with_capacity(metadata.len())`.
