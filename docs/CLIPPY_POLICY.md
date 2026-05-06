# Effortless Metrics Clippy Policy

xchecker uses the shared Effortless Metrics Rust lint model: one governed
workspace lint baseline, explicit policy ledgers, and expiring debt instead of
repo-local lint taste.

## Goals

The baseline is designed to make the workspace:

- panic-free in production and tests;
- resistant to swallowed work, ignored results, and dropped futures;
- safe by default around AST, parser, UTF-8, slice, filesystem, process, async,
  and numeric boundaries;
- explicit about every suppression; and
- ready for planned Rust 1.94 and 1.95 lint flips before the MSRV changes.

## Workspace baseline

The active lint levels live in the root `Cargo.toml` under
`[workspace.lints.rust]` and `[workspace.lints.clippy]`. Every package must
inherit them with:

```toml
[lints]
workspace = true
```

The machine-readable source of truth is `policy/clippy-lints.toml`. The xtask
policy gate checks that the active policy ledger matches the root Cargo lint
blocks and that planned 1.94/1.95 lints remain planned until the workspace MSRV
is bumped.

## No test carveouts

`clippy.toml` must not contain test carveouts such as:

```toml
allow-unwrap-in-tests = true
allow-expect-in-tests = true
allow-panic-in-tests = true
allow-indexing-slicing-in-tests = true
allow-dbg-in-tests = true
```

The standard is workspace panic-free, not only production panic-free. Tests
should return `Result` and use checked assertions/helpers instead of `unwrap`,
`expect`, `panic!`, or indexing shortcuts.

## Suppression style

New suppressions should use `#[expect(..., reason = "...")]`, scoped to the
smallest item or expression that needs the exception. Broad `#[allow(...)]`
attributes are treated as debt and must be represented in
`policy/clippy-debt.toml` with:

- `lint`
- `path`
- `owner`
- `reason`
- `expires`

Existing suppressions were captured as temporary debt during the initial policy
rollout. Follow-up PRs should migrate them to narrow `#[expect]` attributes or
remove the underlying lint violation.

## Policy files

- `policy/clippy-lints.toml` tracks active lint levels and planned Rust 1.94/1.95
  flips.
- `policy/clippy-debt.toml` tracks temporary suppressions and expires them.
- `policy/no-panic-allowlist.toml` reserves the semantic no-panic allowlist
  schema: identity is `path + family + selector`; `last_seen` is advisory.
- `policy/non-rust-allowlist.toml` records non-Rust surfaces with owner, reason,
  classification, and CI coverage.
- `clippy.toml` is only for repo-local Clippy configuration such as disallowed
  methods/types/macros. It is not for weakening the baseline.

## Gate

Run the policy gate with:

```bash
cargo xtask check-lint-policy
```

The gate verifies MSRV alignment, workspace lint inheritance, ledger consistency,
absence of Clippy test carveouts, planned upgrade flips, and non-expired debt.
