# Bolt's Journal

## 2026-01-23 - [GlobSet Optimization]

**Learning:** Checking a file path against multiple `GlobSet`s sequentially is less efficient than combining them into a single `GlobSet` and checking match indices. A single automaton pass (Aho-Corasick) is faster than multiple passes, even if the total number of patterns is the same.
**Action:** When classifying strings against multiple disjoint sets of patterns, combine them into one `GlobSet` (or `RegexSet`) and use the match index to map back to the classification.

## 2024-05-22 - [Dependency Policy Violation]
**Learning:** Adding new dependencies (even dev-dependencies or transient ones like `rayon`) requires explicit approval.
**Action:** Always check `Cargo.toml` constraints and ask before adding deps. Use `std::thread::scope` for simple parallelism instead.
