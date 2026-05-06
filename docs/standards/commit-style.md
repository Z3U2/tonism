# Commit Style Guide

## Format

Follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).

```
[type]([domain]): [description] #JIRAXXXX
```

Example: `feat(offer-catalog): add best offer column in offer list #JIRASELLERPV-2516`

## Commit Types

- `feat` — new feature
- `fix` — bug fix
- `docs` — documentation only
- `style` — formatting, no code change
- `refactor` — code restructuring, no behavior change
- `perf` — performance improvement
- `test` — adding or updating tests
- `chore` — maintenance, dependencies, tooling

## Scope (domain)

Use the functional domain as scope (e.g. `offer-catalog`, `finance`, `settings`, `orders`).

## Language

Commit messages in **English**.
