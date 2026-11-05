# Security Policy

## Supported versions

| Version | Supported |
|---|---|
| 1.0.x | Yes |
| < 1.0 | No |

## Reporting a vulnerability

Do not open a public issue for a suspected vulnerability.

Email `prateekbhujelpb@gmail.com` with:

- A description of the issue and the affected component
- A minimal reproduction or proof of concept
- The Terminus and Rust versions you tested

You will get an acknowledgement within a few days. Confirmed fixes ship as patch releases with a regression test, and the reporter is credited in the changelog unless anonymity is requested.

## Scope notes

Terminus applications must supply their own credentials: the admin panel denies sign-in until the application configures `AdminPanel::authenticate_with`. Production deployments should use a stable `APP_KEY` of at least 32 random bytes, SQLite-backed sessions (`SessionConfig::sqlite`), and `Secure` session cookies behind HTTPS.
