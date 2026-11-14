# Changelog

## Unreleased

## 0.0.4 - 2026-11-14

### Security

- Rate limiting now keys direct clients by the TCP peer (`ConnectInfo<SocketAddr>`), ignoring spoofable forwarding headers (#1).
- Applications behind a reverse proxy can explicitly allowlist proxy socket IPs using `App::rate_limit_with_trusted_proxies` (`RateLimiter::with_trusted_proxies`); `X-Forwarded-For` chains are parsed from the right and IPv4-mapped IPv6 addresses are canonicalized (#1).
- PostgreSQL query execution now uses native protocol parameter binding (`ToSql`) with typed `SqlValue` parameters and OID-guided type conversion (`INT2`/`INT4`/`INT8`, `FLOAT4`/`FLOAT8`, `NUMERIC`, `BOOL`, `JSON`/`JSONB`, `BYTEA`, `TEXT[]`/`INT8[]`) instead of string-interpolating `$1, $2, ...` placeholders into SQL text (#3, #5).

### Added

- Versioned `AppSpec` application contract (`contract_version = "1.0"`) with `ContractOrigin`, 3-state `RouteAuth` (`required`, `public`, `unknown`), `UnavailableSpecItem` disclosures for opaque mounts, nested `App::group` path composition, `GET`, `POST`, `PUT`, `PATCH`, and `DELETE` route capture, `AppModule::route_specs()`, and `AdminPanel` resource/route introspection (#2, #6).
- `App::export_spec()`, `App::write_spec()`, `App::spec_endpoint()`, and build-time/CLI manifest export via `--export-spec [PATH]` or `TERMINUS_EXPORT_SPEC` without binding a network port (#2, #6).
- `RELEASING.md` release checklist with supported platforms, architectural limitations, compatibility policy, feature matrix checks, and minimal bug reproduction template (#4, #7).
- `TERMINUS_REQUIRE_EXTERNAL_TESTS` enforcement so external PostgreSQL, MySQL, Redis, and MongoDB integration tests fail loudly if required service URLs are missing (#4, #7).

### Fixed

- `terminus schema [--json] [--spec=<path>]` now loads the exported `AppSpec` manifest and combines it with live `DATABASE_URL` schema inspection across SQLite, PostgreSQL, and MySQL instead of regex-parsing `src/main.rs` or fabricating `"String"` Rust types for database columns (#2, #6).
- `DatabaseChoice::from_str_loose` now parses `postgres`/`postgresql`/`pg` and `mysql`/`mariadb` instead of returning `Sqlite` for all inputs (#4, #7).
- Fixed `unreachable_patterns` and `unused_imports` warnings in `src/queue.rs` and `dead_code` warnings in `tests/platform_features_tests.rs` when compiling with `--no-default-features` or individual feature flags (#4, #7).
- Fixed `Cargo.toml` `documentation` URL (`https://docs.rs/terminus-rs`), `SECURITY.md` supported version table (`0.0.x`), and `README.md` / CLI `agent:guide` documentation alignment (#4, #7).


## 0.0.3 - 2026-11-05

### Fixed

- README version badge renders as an image again (a missing `!` in the badge markdown turned it into a plain text link).

## 0.0.2 - 2026-11-05

### Documentation

- README now documents all supported database backends (SQLite, PostgreSQL, MySQL/MariaDB, Redis, MongoDB), `DATABASE_URL` selection, backend-agnostic repositories, and session/queue configuration per backend. No code changes.

## 0.0.1 - 2026-11-05

Initial public release.

### Added

- Fluent Axum application engine (`App`) with routing, groups, modules, CORS, rate limiting, and HTTP metrics.
- Admin panel with typed resources, Argon2id password authentication via `AdminPanel::authenticate_with`, CSRF protection, bearer tokens for active repository users, and durable logout revocation.
- Sessions on memory, SQLite, PostgreSQL, MySQL, and Redis with signed cookies, expiry, and revocation.
- Multi-database ORM and migrations (SQLite, PostgreSQL, MySQL/MariaDB) selected by `DATABASE_URL`; server queues with atomic claims, expiring leases, and at-least-once delivery.
- Cache drivers (memory, SQLite, Redis), MongoDB document store, transactional mail, scheduler, events, local/S3 file storage with signed URLs, private/presence broadcasting, localization, OAuth social login, ranked database search, RBAC, OpenAPI/TypeScript SDK generation, and agent introspection (`terminus schema`, `terminus agent:context`).
- Interactive scaffolder (backend/fullstack presets, StarterKit dashboards) and the `terminus` CLI.

### Verification

- 142 tests pass, including live PostgreSQL/MySQL/Redis/MongoDB integration tests.
