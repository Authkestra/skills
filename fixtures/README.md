# Fixtures

Each subdirectory is a minimal crate that compiles the code a skill tells integrators to write. They
are the pack's test suite: if a fixture stops building against the published crate, the skill it backs
is now wrong.

Rules (see [CONTRIBUTING.md](../CONTRIBUTING.md)):

- Depend on `authkestra` from crates.io at the version the skill targets.
- No path dependencies, no `--all-features`.
- Smallest thing that proves the skill's claim.

Every fixture directory containing a `Cargo.toml` is built by CI automatically.
