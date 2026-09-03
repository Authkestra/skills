#!/usr/bin/env python3
"""Validate the plugin manifests and every SKILL.md in this pack.

Run from the repository root: python3 scripts/validate.py
"""
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
errors = []


def err(msg):
    errors.append(msg)


def load_json(path):
    try:
        return json.loads(path.read_text())
    except FileNotFoundError:
        err(f"{path.relative_to(ROOT)}: missing")
    except json.JSONDecodeError as exc:
        err(f"{path.relative_to(ROOT)}: invalid JSON ({exc})")
    return None


def frontmatter(text):
    """Return the YAML frontmatter block as a dict of top-level scalars."""
    if not text.startswith("---\n"):
        return None
    end = text.find("\n---", 4)
    if end == -1:
        return None
    fields = {}
    for line in text[4:end].splitlines():
        if not line.strip() or line.startswith("#") or line.startswith(" "):
            continue
        key, _, value = line.partition(":")
        fields[key.strip()] = value.strip()
    return fields


marketplace = load_json(ROOT / ".claude-plugin" / "marketplace.json")
plugin = load_json(ROOT / ".claude-plugin" / "plugin.json")

if marketplace and plugin:
    listed = {p.get("name") for p in marketplace.get("plugins", [])}
    if plugin.get("name") not in listed:
        err(f"plugin.json name {plugin.get('name')!r} is not listed in marketplace.json {listed}")
    for entry in marketplace.get("plugins", []):
        if entry.get("name") == plugin.get("name") and entry.get("version") != plugin.get("version"):
            err(
                f"version mismatch: marketplace.json {entry.get('version')!r} "
                f"vs plugin.json {plugin.get('version')!r}"
            )

skills = sorted((ROOT / "skills").glob("*/SKILL.md"))
if not skills:
    err("no skills found under skills/*/SKILL.md")

for skill in skills:
    rel = skill.relative_to(ROOT)
    fields = frontmatter(skill.read_text())
    if fields is None:
        err(f"{rel}: missing or malformed YAML frontmatter")
        continue
    for key in ("name", "description"):
        if not fields.get(key):
            err(f"{rel}: frontmatter is missing {key!r}")
    if fields.get("name") and fields["name"] != skill.parent.name:
        err(f"{rel}: frontmatter name {fields['name']!r} != directory {skill.parent.name!r}")
    if len(fields.get("description", "")) < 40:
        err(f"{rel}: description is too short to match against; describe the user's situation")

print(f"checked {len(skills)} skill(s)")
for message in errors:
    print(f"error: {message}", file=sys.stderr)
sys.exit(1 if errors else 0)
