---
name: release
description: Cut a gedcards release — version bump, annotated tag, GitHub release notes.
disable-model-invocation: true
---

# Cut a release

A release is a `chore(release)` bump commit on `main`, an annotated `vX.Y.Z` tag on that
commit, and a GitHub release whose notes are the only changelog this project keeps. There
is no crates.io publish and no release CI: users install with
`cargo install --git`, so the tag and the notes _are_ the release.

## 1. See what is being released

`git fetch`, then confirm `main` is checked out, the tree is clean, and `main` matches
`origin/main`. List the content of the release:

```bash
git log $(git describe --tags --abbrev=0)..HEAD --oneline
```

Done when you can name each merged PR since the last tag.

## 2. Pick the version

Pre-1.0, so: **patch** when existing cards still build to the same bytes (a new optional
field, a fix, docs); **minor** when they don't — a card's meaning or the emitted GEDCOM
changes, so an existing tree rebuilds differently (`0.1 → 0.2` was ids going surname-first).

Done when the version is chosen and its one-line reason stated.

## 3. Go green

```bash
just check
```

Runs fmt, clippy, `cargo sort`, `cargo machete`, `cargo deny` and the tests. Lefthook runs
the same thing on pre-push, so a red tree stops the push after the tag already exists —
fix red here instead.

Done when `just check` exits 0.

## 4. The bump commit

Edit `version` in `Cargo.toml`, then `cargo check` to refresh `Cargo.lock`. Commit those
two files alone:

```
chore(release): bump version to X.Y.Z
```

Committing prompts for the user's approval — that is the global policy, not a problem.

Done when `HEAD` is the bump commit and `git show --stat HEAD` lists only `Cargo.toml` and
`Cargo.lock`.

## 5. Write the notes

The real work of a release; everything else is bookkeeping. Read the last two first —
`gh release view v0.2.3`, `gh release view v0.2.2` — and match their voice:

- **The title names the change, not the version**: "A note on the person".
- Open with an `## ` heading that says what a card author can now do.
- Show a card: a small YAML block in the README's register — real Cyrillic names, real fields.
- Say what it emits, and what `gedc import` reads back, since `build → import → build`
  byte-identity is this project's promise.
- Close with `Closes #N` and the ADR link when the change has one, then
  `**Full changelog:** https://github.com/finnan444/gedcards/compare/vPREV...vNEW`.

Write it to `/tmp/release-vX.Y.Z.md` and show the user the text.

Done when the user has approved the notes.

## 6. Tag the bump commit

```bash
git tag -a vX.Y.Z -m "<the release title>"
```

Done when `git cat-file -t vX.Y.Z` prints `tag` and `git rev-list -1 vX.Y.Z` is the bump commit.

## 7. Hand the push to the user

`git push` is hook-blocked, so ask the user to run:

```bash
git push origin main
git push origin vX.Y.Z
```

Done when `git ls-remote --tags origin vX.Y.Z` returns a line.

## 8. Publish

```bash
gh release create vX.Y.Z --verify-tag --title "<title>" --notes-file /tmp/release-vX.Y.Z.md
```

Remove the scratch notes file and report the release URL.

Done when `gh release view vX.Y.Z` shows the approved notes.
