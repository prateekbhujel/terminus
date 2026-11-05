# Changelog

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
