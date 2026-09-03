# Contributing

## Layout

```
skills/<skill-name>/SKILL.md   # one skill, frontmatter + prose
fixtures/<fixture-name>/       # a minimal crate proving a skill's advice compiles
```

A skill's directory name must equal its frontmatter `name`.

## Writing a skill

- **Frontmatter**: `name` and `description`. The description is what an agent matches against, so
  write it as the situation a user is in ("wants to add login to a Rust app"), not as a topic label.
- **State the target version.** Open with the `authkestra` version the skill was written against.
- **Cite, don't invent.** Every API claim should be traceable to the crate docs or to a runnable
  example in the main repository. Link examples by tag, never by `main`.
- **Say why, once.** A rule an agent understands survives paraphrase; a rule it merely obeys does not.
  "`state` lives in an encrypted cookie *so the deployment stays horizontally scalable*" prevents the
  agent from helpfully "fixing" it later.
- **Prefer the failure mode.** Integrators arrive with a compiler error, not a topic. Skills that map
  symptoms onto causes get used; skills organised like a table of contents do not.

## Adding a fixture

A fixture is a small crate that compiles the code a skill tells people to write.

- Depend on `authkestra` **from crates.io**, at the version the skill targets.
- No path dependencies to a local checkout, and no `--all-features` — a real integrator has neither,
  and both hide exactly the errors this pack exists to prevent.
- Keep it to the smallest thing that proves the point. Fixtures are evidence, not demos; runnable
  demos belong in the main repository's `crates/authkestra/examples/`.

## CI

`scripts/validate.py` checks manifests and skill frontmatter; the workflow then builds every fixture.
Both run on push and on a daily schedule, so a new crate release that breaks a skill shows up here
without anyone remembering to look.
