# Clippy Policy

xchecker treats Clippy as a governed engineering surface rather than a local
preference file. The workspace policy has three layers:

1. **Root manifest lints** in `Cargo.toml` provide the active compiler and
   Clippy baseline for every workspace member.
2. **Policy ledgers** in `policy/` record active lints, planned Rust 1.94/1.95
   flips, temporary debt, and structured exception allowlists.
3. **`xtask` gates** verify that the manifest, ledgers, member inheritance, and
   exception metadata stay coherent.

## MSRV

The workspace MSRV is Rust **1.93**. The root package, workspace package, and
`policy/clippy-lints.toml` must agree on that version.

## Active baseline

The active baseline is intentionally workspace-wide and applies to production
code and tests. It covers:

- panic-family bans such as `unwrap`, `expect`, `panic!`, `todo!`,
  `unimplemented!`, and `unreachable!`;
- AST, UTF-8, string, and slice safety lints;
- silent-failure lints for discarded futures, locks, must-use values, and
  ignored error conversions;
- async/concurrency footguns;
- unsafe and memory-sensitive operations;
- numeric correctness hazards;
- filesystem, process, path, and open-options hazards;
- API/trait correctness; and
- reviewability lints that reduce allocation noise and unclear control flow.

The authoritative review intent for these lints lives in
`policy/clippy-lints.toml`; `Cargo.toml` is the enforcement surface.

## No test carveouts

Do not add Clippy test carveouts such as:

```toml
allow-unwrap-in-tests = true
allow-expect-in-tests = true
allow-panic-in-tests = true
allow-indexing-slicing-in-tests = true
allow-dbg-in-tests = true
```

Tests should return `Result` and use checked assertions/helpers instead of
panic-driven setup.

## Suppression style

Prefer fixing the lint. If a temporary suppression is required, use a narrow
`#[expect(..., reason = "...")]` at the smallest practical scope and add debt
metadata when the exception is expected to survive review. Broad or silent
`#[allow]` attributes are not the desired steady state.

Existing suppressions are being migrated in follow-up work; this PR establishes
the shared policy surface and gate shape first.

## Debt ledger

Temporary lint exceptions belong in `policy/clippy-debt.toml`. Every debt entry
must include:

- `lint`
- `path`
- `owner`
- `reason`
- `expires` in `YYYY-MM-DD` format

Expired debt fails `cargo xtask check-lint-policy`.

## Planned Rust 1.94 / 1.95 flips

`policy/clippy-lints.toml` tracks planned lint flips before the MSRV bump. The
policy gate rejects planned lints that are accidentally activated early, keeping
upgrade work deliberate and reviewable.

## Structured allowlists

The policy directory also reserves TOML allowlists for exception governance:

- `policy/no-panic-allowlist.toml` uses semantic identity for panic-family
  exceptions: `path + family + selector`; `last_seen` line/column data is only
  advisory.
- `policy/non-rust-allowlist.toml` records why non-Rust programming or platform
  files exist, who owns them, what surface they belong to, and which CI command
  covers them.

## Commands

Run the policy gate with:

```bash
cargo xtask check-lint-policy
```

Additional staged checks are available:

```bash
cargo xtask check-no-panic-family
cargo xtask check-file-policy
cargo xtask policy-report
```
