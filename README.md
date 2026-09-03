# Authkestra skills

Official agent skill pack for **integrating** [Authkestra](https://github.com/marcjazz/authkestra) —
the framework-agnostic authentication orchestrator for Rust — into your own application.

Authkestra is a composition library: nothing works until you have picked the right feature flags,
satisfied a typestate builder, implemented storage traits against your own database, and wired an
adapter's extractors. These skills carry that knowledge in a form a coding agent can act on, so it
stops guessing at an API surface where guessing produces authentication bugs.

## Install

```
/plugin marketplace add Authkestra/skills
/plugin install authkestra@authkestra
```

The skills are plain Markdown with YAML frontmatter, so they are equally usable by any other agent
tool — point it at `skills/`, or just read them.

## Skills

| Skill | Answers |
| --- | --- |
| `authkestra-quickstart` | "Add auth to my axum/actix app." Features, TLS backend, the typestate builder, store choice, extractor wiring. |
| `authkestra-features` | "Why doesn't this compile?" Missing Cargo feature vs. unmet typestate prerequisite. |
| `authkestra-store` | "Use my own Postgres/Diesel/SeaORM." The store traits, validated with `authkestra-store-testsuite`. |
| `authkestra-social-login` | GitHub/Google/Discord or a custom provider, and the stateless-OAuth cookie contract. |
| `authkestra-op` | "Be my own identity provider." PKCE, loopback redirects, DPoP, discovery/JWKS, custom grants. |
| `authkestra-resource-server` | Token validation at an API edge: `ValidationConfig`, multi-issuer JWKS, extractors. |
| `authkestra-mfa` | Passkeys, TOTP, and device-bound signatures via `authkestra-devsig`. |
| `authkestra-upgrade` | Code written against the crates RFC-001 dissolved, and release-to-release breaks. |
| `authkestra-troubleshoot` | Runtime symptoms → cause: rustls panics, lost OAuth state, clock skew. |

Skills ship as they are written; the table above is the target set. See
[the tracking epic](https://github.com/marcjazz/authkestra/issues/315) for status.

## Versioning

This pack's minor version tracks the crate's: **`skills` 0.7.x targets `authkestra` 0.7.x**. Every
`SKILL.md` also states the version it was written against, and cites examples in the main repository
by tag rather than by `main`, so a citation always points at code matching that version.

| Pack | `authkestra` |
| --- | --- |
| 0.7.x | 0.7.x |

## How these stay true

The pack lives outside the crate repository, so nothing mechanically forces a skill to be updated when
an API changes. That gap is closed deliberately:

- **`fixtures/` are the tests.** Each fixture is a minimal application depending on `authkestra`
  **from crates.io** — never a path dependency, never `--all-features`. CI builds them on every push
  and on a daily schedule, so a published release that invalidates a skill turns this repo red.
- **Releases notify.** The crate repository dispatches to this one after publishing.
- **The crate repository's `AGENTS.md`** requires checking this pack when a public API name, signature,
  route path or feature flag changes.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Corrections from real integrations are the most valuable thing
you can send: if a skill led your agent somewhere wrong, that is a bug here, not a mistake you made.

## License

MIT or Apache-2.0, at your option — matching Authkestra itself.
