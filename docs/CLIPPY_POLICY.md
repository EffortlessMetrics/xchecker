# Clippy Policy

xchecker treats Clippy as a governed engineering surface rather than a local
style preference. The workspace policy has three goals:

1. keep production and test Rust panic-free by default;
2. prevent silent failure, silent suppression, and unreviewed unsafe/memory
   footguns; and
3. track future Rust/Clippy flips before the MSRV changes.

## Active baseline

The active lint baseline lives in the root `Cargo.toml` under
`[workspace.lints.rust]` and `[workspace.lints.clippy]`. Every workspace member
must inherit it with:

```toml
[lints]
workspace = true
```

The machine-readable ledger is `policy/clippy-lints.toml`. It records the
current MSRV, active lints, policy posture, and planned Rust 1.94/1.95 flips.
The ledger is intentionally redundant with `Cargo.toml` so `cargo xtask
check-lint-policy` can verify that policy and manifest state do not drift.

## No test carveouts

The policy is workspace panic-free, not merely production panic-free. Do not add
Clippy test carveouts such as:

```toml
allow-unwrap-in-tests = true
allow-expect-in-tests = true
allow-panic-in-tests = true
allow-indexing-slicing-in-tests = true
allow-dbg-in-tests = true
```

Prefer tests that return `Result` and use explicit assertion helpers or domain
errors instead of `unwrap`, `expect`, or panic-driven setup.

## Suppression style

Use narrow `#[expect(..., reason = "...")]` suppressions only when the code is
reviewed and the reason is useful to a future maintainer. Silent `#[allow]`
attributes are rejected by policy checks unless a future policy ledger makes a
specific exception explicit.

Good:

```rust
#[expect(
    clippy::arithmetic_side_effects,
    reason = "bounded fixture count checked by the parser table generator"
)]
let next = current + 1;
```

Bad:

```rust
#[allow(clippy::arithmetic_side_effects)]
let next = current + 1;
```

## Debt ledger

Temporary lint debt belongs in `policy/clippy-debt.toml`. Each entry must include
`lint`, `path`, `owner`, `reason`, and `expires`. Expired entries fail the policy
gate. Keeping the file empty is preferred.

## Allowlist model

Policy exceptions use structured TOML receipts:

- `policy/no-panic-allowlist.toml` reserves the semantic no-panic schema based on
  `path + family + selector` identity, with advisory `last_seen` line/column
  hints.
- `policy/non-rust-allowlist.toml` records intentional non-Rust files or globs
  with an owner, reason, surface, classification, and CI coverage.

These files are the maintenance hatches for the strict global default. Exceptions
must be explicit, reviewed, and removable.

## Policy gate

Run the local gate with:

```bash
cargo xtask check-lint-policy
```

For narrower checks, the same binary accepts:

```bash
cargo xtask check-file-policy
cargo xtask check-no-panic-family
cargo xtask policy-report
```

The initial gate verifies lint inheritance, active/planned ledger consistency,
absence of Clippy test carveouts, suppression style, required debt metadata, and
expired debt. The allowlist commands provide the shared command surface for
future AST-aware no-panic and non-Rust coverage enforcement.
