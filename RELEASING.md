# Release Readiness & Compatibility Guide

This document defines the supported environments, current architectural boundaries, pre-release verification gate, and minimal bug reproduction format for `terminus-rs`.

## Supported Platforms & Toolchain

| Environment | Support Level | Notes |
|---|---|---|
| Linux (`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`) | Tier 1 (CI gated) | Primary production and CI target |
| macOS (`aarch64-apple-darwin`, `x86_64-apple-darwin`) | Tier 1 | Full development, CLI, and server support |
| Windows (`x86_64-pc-windows-msvc`) | Best effort | Library and `install.ps1` supported; POSIX-specific shell helpers require Git Bash or WSL |
| Rust Toolchain | Stable `1.75+` (2021 edition) | Bundled SQLite (`rusqlite`) compiles from C source via `cc` |

## Current Limitations

- **Single-process rate limiting and scheduling**: `RateLimiter` and `Scheduler` store state in process memory. Multi-instance deployments should enforce edge rate limits at a reverse proxy or shared gateway and run a single scheduler leader.
- **Trusted proxy allowlisting**: `App::rate_limit_with_trusted_proxies` matches explicit socket `IpAddr` entries; CIDR range matching is not yet built in.
- **Queue delivery semantics**: `PersistentJobQueue` / `SqliteJobQueue` provides at-least-once execution using expiring database leases. Job handlers must be idempotent.
- **Contract export (`terminus schema`)**: Authoritative route, resource, and module metadata comes from an exported `AppSpec` manifest (`--export-spec` or `TERMINUS_EXPORT_SPEC`). Raw Axum `Router` mounts (`App::route`, `App::api`, `App::merge`) cannot be introspected by Axum and are recorded under `unavailable` in the spec.
- **Client contract targets (`terminus make:sdk`)**: Supports `--lang=ts` (TypeScript) and `--lang=openapi` (OpenAPI 3.0.3).

## Compatibility Policy (`0.0.x`)

While `terminus-rs` is in the `0.0.x` series:
- Patch releases (`0.0.x`) prioritize bug fixes, security hardening, and documentation accuracy, though minor adjustments to experimental APIs may occur before `0.1.0` / `1.0.0`.
- Breaking changes to `terminus::prelude` or the `AppSpec` contract (`contract_version`) are documented in [CHANGELOG.md](CHANGELOG.md).
- Only the latest `0.0.x` release receives security updates per [SECURITY.md](SECURITY.md).

## Pre-Release Verification Checklist

Every release must pass all checks below on a clean working tree before tagging:

1. **Formatting and Lints**
   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets -- -D warnings
   cargo clippy --no-default-features --all-targets -- -D warnings
   RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
   ```

2. **Cargo Feature Matrix**
   ```bash
   cargo check --no-default-features --all-targets
   for feature in postgres mysql redis mongodb s3 oauth; do
     cargo check --no-default-features --features "$feature" --all-targets
   done
   ```

3. **Self-Contained and No-Default-Features Tests**
   ```bash
   cargo test --all-targets
   cargo test --no-default-features
   ```

4. **External Service Integration Tests (No Silent Skips)**
   ```bash
   TERMINUS_REQUIRE_EXTERNAL_TESTS=1 \
     TERMINUS_TEST_POSTGRES_URL="postgres://terminus:secret@127.0.0.1:5432/terminus" \
     TERMINUS_TEST_MYSQL_URL="mysql://root@127.0.0.1:3306/terminus" \
     TERMINUS_TEST_REDIS_URL="redis://127.0.0.1:6379" \
     cargo test --test database_backend_tests --test fullstack_backend_tests --test stores_features_tests --test schema_contract_tests
   ```

5. **CLI Installation & Quickstart Smoke Test**
   ```bash
   cargo install --path . --root /tmp/terminus-release-smoke
   /tmp/terminus-release-smoke/bin/terminus version
   TERMINUS_EXPORT_SPEC=/tmp/terminus-release-smoke/app-spec.json cargo run --example quickstart
   /tmp/terminus-release-smoke/bin/terminus schema --json --spec=/tmp/terminus-release-smoke/app-spec.json
   ```

6. **Package Hygiene & Metadata Alignment**
   - Verify `Cargo.toml` version, `README.md` dependency snippets and version badge, `CHANGELOG.md` release heading, and `SECURITY.md` supported version table agree.
   - Inspect packaged file list:
     ```bash
     cargo package --allow-dirty --list
     ```
   - Confirm no `.db`, `.sqlite`, `.env`, or scratch artifacts appear in the package list.

## Minimal Bug Report Reproduction Template

When reporting a bug in GitHub Issues, include:

```markdown
### Environment
- `terminus-rs` version: (e.g. `0.0.4`)
- `rustc --version`:
- OS / architecture:
- Cargo features (`default`, `--no-default-features`, or specific list):
- Database / store backend (`sqlite`, `postgres`, `mysql`, `redis`, `mongodb`):

### Minimal Reproduction
Provide a self-contained `src/main.rs` snippet or exact `terminus` CLI command sequence:

```rust
use terminus::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Minimal code that triggers the issue
    Ok(())
}
```

### Expected Behavior
What should happen (HTTP status, database state, or CLI exit code / stdout).

### Actual Behavior
Exact compiler error, panic backtrace, HTTP response, or CLI output observed.
```
