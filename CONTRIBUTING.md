# Contributing to Garantify

## Workflow

1. Fork the repository and create a branch from `main`
2. Name your branch descriptively: `fix/email-timeout`, `feat/api-export`, etc.
3. Make your changes — one logical change per PR
4. Ensure all checks pass (see below)
5. Open a pull request against `main` with a clear description of what and why

## Code conventions (Rust)

- `cargo fmt` must pass before committing
- `cargo clippy -- -D warnings` must produce no warnings
- Use `thiserror` for error types — `anyhow` is not allowed in production code
- No `unwrap()` or `expect()` in handlers — handle errors explicitly
- Use `sqlx::query()` / `query_as()` runtime variants (not the `query!` macros) since we don't run `cargo sqlx prepare` in CI
- Log with `tracing` macros (`info!`, `warn!`, `error!`) — never `println!`
- No comments unless the reason is non-obvious; never comment what the code does

## Running the tests

```bash
# Unit tests (no database required)
cargo test

# Integration tests require a running Postgres instance
# Set TEST_DATABASE_URL in your environment first:
export TEST_DATABASE_URL=postgres://garantify:dev@localhost:5432/garantify_test
cargo test --test '*'
```

## Running the app locally

See the [Local Development](README.md#local-development) section in the README.

## What we won't merge

- Features that add significant complexity without a clear use case
- PRs that skip error handling or introduce `unwrap()` in handlers
- Changes that break the Docker deployment (`docker compose up` must still work)
