# YAML cards vs Markdown-with-frontmatter, and the add-a-relative UX

Researched 2026-07-19. All claims below were checked against the source that owns
them (specs, official docs, crate source), not secondary write-ups.

## The question

1. Is plain YAML a good format for the per-person cards, compared with
   Markdown carrying YAML frontmatter — the format Obsidian works with?
2. For an ordinary, non-technical user who wants to add a relative, what is the
   most convenient realistic workflow?

## Findings

### YAML's classic footguns are a YAML 1.1 problem

- YAML 1.1 resolves `y|Y|yes|Yes|YES|n|N|no|No|NO|true|True|TRUE|false|False|FALSE|on|On|ON|off|Off|OFF`
  as booleans — the "Norway problem" (`country: NO` becomes `false`).
  <https://yaml.org/type/bool.html>
- YAML 1.1 also has base-60 "sexagesimal" integers matching
  `[-+]?[1-9][0-9_]*(:[0-5]?[0-9])+` — e.g. `190:20:30` parses as a number.
  <https://yaml.org/type/int.html>
- YAML 1.1 defines a timestamp type whose valid forms include a bare date,
  `2002-12-14` — so 1.1-schema parsers may auto-type unquoted dates.
  <https://yaml.org/type/timestamp.html>
- YAML 1.2's Core schema removed all of this: booleans are only
  `true|True|TRUE|false|False|FALSE`, null is only `null|Null|NULL|~` or empty,
  integers are base 10/8/16, and there is no sexagesimal or timestamp
  resolution. Anything unmatched is a string.
  <https://yaml.org/spec/1.2.2/#1032-tag-resolution>

### What the parser in this repo actually does

- `Cargo.toml` depends on `serde_norway` 0.9.42, a self-described "hard-fork of
  Serde YAML" (<https://github.com/cafkafk/serde-yaml>). The original
  `serde_yaml` repo is archived (confirmed via the GitHub API: `archived: true`,
  last push 2024-03-25). <https://github.com/dtolnay/serde-yaml>
- Its scalar resolution (`src/de.rs`) follows the 1.2 Core schema, not 1.1:
  `parse_bool` matches only `true|True|TRUE|false|False|FALSE`, `parse_null`
  only `null|Null|NULL|~`, and there is no sexagesimal or timestamp handling —
  `no`, `1947-05-12`, and `190:20:30` all stay strings.
  <https://github.com/cafkafk/serde-yaml/blob/main/src/de.rs>
- On top of that, `src/lib.rs` parses each card into a `Value` and requires
  `Value::String` for every field, reporting `expected a string` otherwise. So
  even a value that does auto-type (e.g. `name: 1234`) becomes a compile
  diagnostic, never silent corruption.
- The remaining footgun is plain YAML syntax, not typing: a value containing
  `: ` or starting with `#`, `[`, `{`, `*`, `&` needs quoting. For name-like
  genealogy data this is rare but real (e.g. a note field like
  `Иванов: купец` would misparse without quotes).

### Obsidian properties (official docs)

Source: <https://help.obsidian.md/properties> (redirects to
`obsidian.md/help/properties`).

- Properties are "stored in YAML format at the top of the file". JSON is also
  accepted but "will be read, interpreted, and saved as YAML".
- Editing is UI-first: a property editor at the top of the note, with a
  `Source` display mode for raw YAML. Property types: Text, List, Number,
  Checkbox, Date, Date & time, plus the special Tags.
- Explicit limitations: "Nested properties" are not supported (only visible in
  source mode), and "Markdown in properties" is intentionally unsupported —
  "properties are meant for small, atomic bits of information".
- Wikilinks in properties are supported but must be quoted: "Internal links in
  text properties must be surrounded with quotes."
- Property links are first-class for backlinks: the Obsidian 1.4.5 changelog
  states "Properties with links will now properly show in backlink entries."
  <https://obsidian.md/changelog/2023-08-31-desktop-v1.4.5/>
  The Graph view help page, however, does not mention property links among what
  the graph shows or filters — graph inclusion is not documented.
  <https://obsidian.md/help/plugins/graph>
- Bases is now a core plugin: "database-like views of your notes" where you
  "view, edit, sort, and filter files and their properties" — i.e. a
  spreadsheet-like table over frontmatter. <https://obsidian.md/help/bases>

### Obsidian genealogy plugins (official registry)

From `community-plugins.json` in the official
<https://github.com/obsidianmd/obsidian-releases> registry (checked
2026-07-19): **Charted Roots** ("Family tree visualization with GEDCOM/Gramps
import"), **People Tree** ("Interactive family trees … from YAML frontmatter —
with avatars, inline editing"), **Arbor Family Tree**, **Grafily**, and the
generic **Relations**. So a person-note-with-frontmatter vault has an existing
plugin ecosystem for rendering trees.

### Markdown + frontmatter as a convention

The pattern "structured data at the top of a human file" is a long-standing
SSG convention, so tooling for it is abundant:

- Jekyll: "The front matter must be the first thing in the file and must take
  the form of valid YAML set between triple-dashed lines."
  <https://jekyllrb.com/docs/front-matter/>
- Hugo: front matter is "metadata that: Describes the content … Establishes
  relationships with other content"; YAML, TOML and JSON are supported.
  <https://gohugo.io/content-management/front-matter/>

### How a dedicated genealogy app does "add a person" (Gramps 5.2 manual)

<https://gramps-project.org/wiki/index.php/Gramps_5.2_Wiki_Manual_-_Entering_and_editing_data:_brief>

- Adding is a form: "click the Toolbar + button … The Edit Person dialog will
  be shown and you can enter any data you know about this person."
- Adds are context-aware: "adding a Person from within the Family context of
  the Relationships or Charts views automatically inserts the new Person into
  the Family" — no separate step to graft the person into a family.
- Relationships are edited through the Relationships View or the Edit Family
  dialog, never by typing identifiers.

The takeaway for non-technical UX: the benchmark is a form with labeled
fields plus context-aware linking, so the user never invents ids or edits two
places. gedcards' ADR 0001 already gets the "one place" half by declaring
parents on the child's card.

## Comparison against this project's needs

| Need | Plain YAML card | Markdown + YAML frontmatter |
|---|---|---|
| Source of truth for a compiler | The whole file is the record; unknown keys and non-string values are already compile diagnostics. | Frontmatter is the record; the body is undefined territory the compiler must decide to ignore or assign meaning to. Parsing cost is trivial (strip the `---` fence), but the format now has a "data half" and a "free-text half". |
| Git-friendly, deterministic | Yes. | Equally yes. One caveat: Obsidian's UI editor rewrites the YAML block on edit, so formatting churn in diffs is possible. |
| Non-technical editing | Weakest point: a bare text file, syntax errors possible (though caught in batch at compile time, nothing written on error). | Strongest point: Obsidian gives a form-like property editor, quoted `[[wikilinks]]` with autocompletion for relationships, backlinks ("children of X" for free), Bases table views, and genealogy plugins that draw the tree. |
| Schema enforcement | Only the compiler enforces it — same in both cases. | Obsidian's editor knows property *types* but not this schema; `father` vs `fathr` is still only caught by the compiler. |
| Typing footguns | Neutralized: serde_norway resolves per YAML 1.2 Core, and fields must be strings. | Same YAML inside; Obsidian additionally requires quoting wikilinks, an easy thing to forget in source mode. |

The two options are not mutually exclusive: a `.md` card whose frontmatter is
exactly today's YAML mapping (file name still the id, body ignored or reserved
for free-form notes) is a superset reader away, with no change to the data
model. An Obsidian note name would then need to equal the card id for
`"[[pyotr-ivanov-1947]]"`-style parent links to double as vault links.

## The add-a-relative question

Realistic workflows, worst to best for a non-technical user:

1. **CLI wizard (`gedcards add`).** Removes syntax risk and validates
   immediately, but the audience that fears YAML fears the terminal more; and
   every later correction still lands them in a text editor, so the wizard only
   covers the first write. It is also a second way to do the same thing.
2. **Hand-editing YAML.** Fine for the project owner; for others, viable only
   as "copy an existing card, change the values". The compiler's batch
   diagnostics and refuse-to-write-on-error behavior make mistakes cheap, which
   is the strongest mitigation this design already has.
3. **Obsidian over frontmatter cards.** Closest to the Gramps form benchmark
   without writing any GUI: new note from a template, fill labeled properties
   in the UI, pick the father/mother by link autocompletion, see children via
   backlinks, view the tree via a community plugin. Remaining sharp edges: the
   user must not rename note files casually (the file name is the GEDCOM id),
   and wikilink values need quotes.
4. **A real GUI (Gramps-style forms).** The actual gold standard, and out of
   scope for this project — building one contradicts its premise of plain
   files under version control.

## Recommendation

Keep plain YAML cards as the source of truth. The famous YAML hazards are 1.1
hazards; the parser in use resolves scalars per the 1.2 Core schema and the
compiler rejects every non-string field, so nothing auto-types silently. For a
compiler input, one-record-per-file with zero prose is the cleaner contract.

If and when a real Obsidian user appears (per the no-over-engineering rule:
not before), the right move is small: teach the reader to also accept `.md`
files by stripping the `---` frontmatter fence and treating the body as
ignored notes. That single change buys the entire Obsidian editing story —
property editor, link autocompletion, backlinks, Bases tables, tree plugins —
without a new format, a new schema, or a second source of truth.

Do not build a CLI wizard. It serves neither audience: the technical user
copies a card faster, and the non-technical user needs a form, which Obsidian
already is.
