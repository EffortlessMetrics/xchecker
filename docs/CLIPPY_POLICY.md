# Clippy Policy

xchecker treats Clippy as a governed engineering surface, not as a local taste file.
The workspace policy has three parts:

1. a strict workspace lint block in `Cargo.toml`;
2. machine-readable policy ledgers in `policy/`; and
3. `cargo xtask check-lint-policy` as the CI gate for policy coherence.

## Baseline

The root manifest owns the active lint baseline under `[workspace.lints.rust]` and
`[workspace.lints.clippy]`. Every workspace package must inherit it with:

```toml
[lints]
workspace = true
```

The active baseline is intentionally panic-free across production code and tests.
`unsafe_code` is currently warn-level because Rust 2024 environment mutation and process-group test helpers still need a follow-up safety/debt migration before the workspace can ratchet to `forbid`.
It denies unchecked `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, and
`unreachable!` shapes, blocks silent-failure patterns such as ignored `Result`s,
and starts suppression governance by rejecting blanket Clippy category suppressions while legacy narrow `#[allow]` sites are migrated.

## No test carveouts

Do not add Clippy test carveouts in `clippy.toml`, including:

```toml
allow-unwrap-in-tests = true
allow-expect-in-tests = true
allow-panic-in-tests = true
allow-indexing-slicing-in-tests = true
allow-dbg-in-tests = true
```

Tests should return `Result` when setup can fail and should use explicit assertion
helpers rather than panic-driven fixture setup.

## Suppression style

Use narrow `#[expect(..., reason = "...")]` suppressions when a local exception is
needed. The reason must explain why the exception is safe and temporary enough to
review. Blanket `#[allow(clippy::all)]`, `pedantic`, `nursery`, and `restriction` suppressions are rejected by policy. Legacy narrow `#[allow]` sites remain visible debt for follow-up cleanup; new suppressions should use `#[expect(..., reason = "...")]`.

## Policy files

`policy/clippy-lints.toml` is the machine-readable ledger for the active policy
posture and planned Rust 1.94/1.95 flips. It records the workspace MSRV, test
panic posture, suppression style, and planned lints that should remain inactive
until the matching MSRV bump.

`policy/clippy-debt.toml` records temporary exceptions with `lint`, `path`,
`owner`, `reason`, and `expires`. Debt is allowed only when it is explicit,
reviewable, and expiring.

`policy/no-panic-allowlist.toml` is reserved for semantic panic-family exceptions.
Entries are keyed by `path + family + selector`; line and column data belong only
in advisory `last_seen` fields so refactors do not turn policy into line-number
whack-a-mole.

`policy/non-rust-allowlist.toml` documents non-Rust files that are intentionally
part of the repository. Entries use `path` or `glob` plus `kind`, `owner`,
`reason`, `surface`, `classification`, `covered_by`, and optional `expires`.

## Local check

Run the policy gate with:

```bash
cargo xtask check-lint-policy
```

The gate verifies the MSRV ledger, workspace lint inheritance, active Cargo lint
levels, absence of Clippy test carveouts, planned upgrade lints, blanket
suppression posture, and debt shape/expiry.
