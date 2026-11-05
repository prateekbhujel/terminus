# Contributing to Terminus

Thanks for helping improve Terminus. Bug reports, fixes, tests, documentation, and new admin resources are all welcome.

## Getting started

```bash
git clone https://github.com/prateekbhujel/terminus.git
cd terminus
cargo test
cargo run --example quickstart
```

Create the first local admin accounts in a scratch directory:

```bash
terminus migrate
terminus db:seed   # prints a one-time generated password; local development only
```

## Before opening a pull request

Run the same checks as CI:

```bash
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps
```

- Keep changes focused: one bug or feature per pull request.
- Add a deterministic regression test for every behavior change.
- Do not introduce demonstration credentials, seeded production accounts, or in-memory-only behavior presented as persistent.
- Document at-least-once, locking, or persistence semantics when you touch the queue, sessions, or migrations.
- Update `CHANGELOG.md` under an `Unreleased` heading for user-visible changes.

## Reporting bugs

Open a GitHub issue with your Terminus and Rust versions, a minimal reproduction, expected behavior, and actual behavior (including CLI exit codes or HTTP statuses where relevant).

## Security reports

Do not open public issues for suspected vulnerabilities. Follow [SECURITY.md](SECURITY.md).
