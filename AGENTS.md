# AGENTS.md

This file is the map for AI agents working in this repository.
It states what the repository is, the commands that define "done", and the
hard rules. Detailed rules live elsewhere and are linked from here; when a
detail here conflicts with the code or `Cargo.toml`, the code wins and this
file should be fixed in the same PR.

## What This Repository Is

`tsqlremaker` converts SAP ASE (Sybase) T-SQL into other SQL dialects
(MySQL, PostgreSQL, SQLite). It also ships an SAP ASE language server and a
schema-diff tool that computes DDL migration plans.

## Priorities and Scope

The converter is the main line of this repository: `tsql-parser` ->
`common-sql` -> the dialect emitters (and the `wasm` bindings on top).
Progress is measured by conversion coverage and correctness, expressed
as conversion tests (input T-SQL plus expected output per dialect) and
property tests where an invariant can be stated.

The language server (`ase-ls-core`, `ase-ls`) and `schema-diff` are
byproducts of the shared parser/AST core and are in maintenance mode:
bug fixes are welcome, but do not add features or expand their scope
without an explicit decision recorded in an issue or PR. Keeping them
stable is the goal; the converter's needs drive changes to the shared
core, not the other way around.

Out of scope for this repository: whole-program stored-procedure
analysis (cross-procedure call graphs, temp-table data flow). That is a
separate tool's job; do not grow the converter or the language server
toward it.

## Workspace Structure

11 crates under `crates/`. Dependency direction flows downward in this
list; do not add upward or cyclic dependencies.

| Crate | Role |
|---|---|
| `tsql-token` | Token types and keyword lookup |
| `tsql-lexer` | Lexical analysis (cursor-based lexer) |
| `tsql-parser` | T-SQL parser; produces the AST and lowers it to common-sql |
| `common-sql` | Dialect-independent common SQL AST |
| `mysql-emitter` | Emits MySQL SQL from the common AST |
| `postgresql-emitter` | Emits PostgreSQL SQL from the common AST |
| `sqlite-emitter` | Emits SQLite SQL from the common AST |
| `wasm` | WebAssembly bindings (package name: `tsql-remaker-wasm`) |
| `ase-ls-core` | Language-server core logic |
| `ase-ls` | SAP ASE language server binary (VS Code / Zed, tower-lsp) |
| `schema-diff` | Schema diff engine and CLI: migration plans between a catalog and a desired schema; depends on the parser, common-sql, and the emitters |

## Build, Test, and Lint Commands

```bash
cargo build --workspace                 # Build all workspace crates
cargo check --workspace                 # Type-check without artifacts
cargo test --workspace                  # Run all tests (incl. doc tests in CI)
cargo nextest run --workspace           # Test runner used in practice
cargo test --package tsql-lexer         # Test a single crate
cargo fmt --all -- --check              # Formatting gate (CI)
cargo clippy --workspace --all-targets -- -D warnings  # Lint gate (CI)
cargo llvm-cov --workspace              # Coverage (CI uses cargo-llvm-cov)
cargo doc --workspace --no-deps --all-features  # Docs (CI)
```

Before opening a PR, at minimum run the formatting gate, the lint gate,
and the tests for the crates you touched; run the full workspace tests
when a shared crate (`tsql-token`, `tsql-lexer`, `tsql-parser`,
`common-sql`) changes.

## Hard Rules

- **No new `panic!` / `unwrap()` / `expect()` in library code.**
  `[workspace.lints.clippy]` in the root `Cargo.toml` denies `panic`,
  `expect_used`, and `unwrap_used` for crates that inherit workspace
  lints. Return `Result<T, E>` with `thiserror` error types instead.
  Test code may be exempted per crate; see
  `.claude/rules/workspace-lint-allowances.md`.
- **Keep the private `ase-rs` dependency optional.** In `schema-diff`,
  the `ase-types` / `ase-driver` / `ase-dsn` / `ase-tds` git dependencies
  on the private `Sou-Tokuda/ase-rs` repository sit behind the `ase`
  feature (`default = []`). The default build must never resolve that
  private upstream; making it non-optional breaks every developer and
  CI build. Jobs that do fetch it need the `CARGO_ASE_RS_TOKEN` secret
  and the GIT_ASKPASS script under `.github/scripts/` (see the comments
  at the top of `.github/workflows/ci.yml`).
- **Formatting is rustfmt defaults.** No custom rustfmt configuration.
- **Dependency direction stays one-way** as listed in the workspace
  table above (for example, emitters depend on `common-sql`; `common-sql`
  depends on no emitter).

## Code Style

Detailed, normative rules are maintained under `.claude/rules/`
(notably `rust-style.md`, `rust-anti-patterns.md`, `rust-test.md`, and
`project-ast-types.md`). The short version:

- Structs/Enums: `PascalCase`; functions/methods and variables:
  `snake_case`; constants: `SCREAMING_SNAKE_CASE`.
- Derive `Debug` on public structs.
- Use `&str` for borrowed strings, `String` for owned strings; make
  lifetimes explicit on reference-holding types.
- Keywords are module-level `pub const` values with lowercase strings;
  keyword resolution goes through `lookup_ident()` and is
  case-insensitive.
- Comments may be Japanese or English. Use `///` for public API docs
  and document non-obvious logic.

## Testing Conventions

- Unit and integration tests live in each crate (`src` tests and
  `tests/`). Shared dev-dependencies (`rstest`, `proptest`, `criterion`)
  are declared in `[workspace.dependencies]`.
- Parser and emitter changes should come with conversion tests:
  input T-SQL plus the expected output per dialect. Property tests
  (proptest) are used where an invariant can be stated (for example
  round-trip stability).
- Paste actual test output into the PR description; the PR template
  has a Test Results section for this.

## CI

`.github/workflows/ci.yml` runs on pushes and PRs to `master` and
`develop`:

- Lint: `cargo fmt --all -- --check` and clippy with `-D warnings`
- Test: build + `cargo test --workspace` on Ubuntu, Windows, and macOS
- Coverage: cargo-llvm-cov, uploaded to Codecov
- Security: `cargo audit`
- Docs: `cargo doc`, uploaded as an artifact

Other workflows: `release.yml`, `wasm-deploy.yml`, `codecov.yml`, and
`rust.yml`. Do not weaken a CI gate to make a PR pass; fix the
code or propose the gate change separately.

## Where to Read Next

- `CLAUDE.md` — the Kiro-style spec-driven development workflow used
  for feature work (steering: `.kiro/steering/`, specs: `.kiro/specs/`).
  Follow its phase approvals for spec-driven tasks.
- `.claude/rules/` — detailed rules: architecture coupling
  (`architecture-coupling-balance.md`), git branch and commit
  conventions (`git-branch-strategy.md`, `git-commit-attribution.md`),
  pre-commit checks (`pre-commit-rust.md`), and more.
- `CONTRIBUTING.md` — contributor-facing build and workflow guide.
- `docs/` — `SDD.md`, architecture decision records in `docs/adr/`,
  and `SAP_ASE_TSQL_DIALECT_ANALYSIS.md` for dialect details.
- `.github/pull_request_template.md` — required PR sections, including
  Test Results and Architecture Considerations.

## Notes

- Platforms: Linux, macOS, Windows (CI matrix), plus WASM via the
  `wasm` crate.
