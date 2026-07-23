# gedcards

Write one small YAML file per person, keep the folder in git, and compile it into
a [GEDCOM 5.5.1](https://gedcom.io/specifications/ged551.pdf) file that genealogy
software imports. You never touch `.ged` syntax: no `@I42@` cross references to keep
straight, no encoding roulette, and a diff that shows a person changing rather than
a line number moving.

It is built for Slavic names in particular. A patronymic is its own field on the card,
and a married surname is emitted so the maiden name still displays after import — two
things GEDCOM 5.5.1 has no standard place for, and which this compiler decides once so
you don't have to.

- **Cards are the source of truth.** The `.ged` is a derived artifact: never edited by
  hand, safe to delete and rebuild.
- **Slavic names are first-class.** `patronymic` is a separate field, joined into the
  given name on the way out; `married_surname` becomes `_MARNM`, the extension
  MyHeritage reads for a surname taken at marriage.
- **Deterministic.** Byte-for-byte identical output for the same input, so a rebuild
  produces no spurious diff. A golden test compiles the same cards in shuffled order
  and asserts the bytes match.
- **Every error in one run.** All problems are reported together, so you fix a batch
  instead of one per rebuild — and when anything is wrong, nothing is written.
- **Typos get a suggestion.** An unknown `father: pyotr-ivanof` answers with
  `did you mean pyotr-ivanov?`.
- **UTF-8 throughout.** Cyrillic goes in and comes out unchanged, with `1 CHAR UTF-8`
  in the header.
- **Small.** One direct dependency (`serde_norway`), 14 crates in a release build. The
  JSON Schema validator the tests check the schema with is a dev-dependency: it is built
  by `just test`, never by `cargo install`.

One card in:

```yaml
# people/maria-sidorova.yaml
name: Мария
patronymic: Петровна
surname: Сидорова
married_surname: Иванова
sex: F
birth:
  date: ~1925
```

`gedc build`, and that person in `family.ged`:

```
0 @I1@ INDI
1 NAME Мария Петровна /Сидорова/
2 GIVN Мария Петровна
2 SURN Сидорова
2 _MARNM Иванова
1 SEX F
1 BIRT
2 DATE ABT 1925
```

> **Status: early.** A card carries a full name, a sex, birth and death events, and
> its relationships; the compiler emits `INDI` and `FAM` records — enough for a tree
> a viewer can draw. The output is written to the 5.5.1 spec, but it has not yet been
> round-tripped through a real genealogy service, so treat import compatibility as
> untested rather than promised.

---

## Contents

- [Getting started](#getting-started)
- [Names](#names)
- [Dates and events](#dates-and-events)
- [Relationships](#relationships)
- [Ids](#ids)
- [Importing an existing tree](#importing-an-existing-tree)
- [Errors](#errors)
- [Editor completion](#editor-completion)
- [Library](#library)
- [Development](#development)
- [License](#license)

---

## Getting started

**1. Install Rust** (1.85 or newer — the crate uses edition 2024), if you don't have it:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**2. Install the `gedc` binary:**

```bash
cargo install --git https://github.com/finnan444/gedcards
```

**3. Make a directory for your tree:**

```bash
mkdir -p my-tree/people
cd my-tree
```

**4. Write `tree.yaml`** — it holds what goes into the GEDCOM header:

```yaml
submitter: Иван Иванов
language: Russian
```

**5. Write one card per person in `people/`.** The file name without its extension is
that person's id, so `people/ivan-ivanov.yaml` is the person `ivan-ivanov`:

```yaml
name: Иван
surname: Иванов
sex: M
```

**6. Build:**

```bash
gedc build
```

This writes `family.ged` next to `tree.yaml`, and prints how many people it wrote.
Your directory now looks like this:

```
my-tree/
  tree.yaml
  family.ged      <- generated; add it to .gitignore
  people/
    ivan-ivanov.yaml
```

**7. Import `family.ged`** into whatever genealogy service or desktop app you use.
Rebuild and re-import whenever the cards change — the `.ged` is disposable.

---

## Names

`name`, `surname` and `sex` (`M` or `F`) are required. `patronymic` and
`married_surname` are optional:

```yaml
name: Мария
patronymic: Петровна
surname: Сидорова
married_surname: Иванова
sex: F
```

The patronymic is part of the name — it compiles to `1 NAME Мария Петровна /Сидорова/`
with `2 GIVN Мария Петровна` — but it stays its own field on the card rather than being
glued onto `name`. Why, and what the alternatives cost, is in
[ADR 0002](docs/adr/0002-patronymic-joins-the-given-name.md).

`surname` is always the surname at birth. A `married_surname` is emitted as `2 _MARNM`,
the extension MyHeritage uses, which is what makes maiden names display correctly after
import. It is the surname taken at marriage, and a [divorce](#relationships) leaves it
alone: whether the name was kept afterwards is not something the card says either way.

---

## Dates and events

`birth` and `death` are blocks carrying a `date`, a `place`, or both:

```yaml
name: Пётр
surname: Иванов
sex: M
birth:
  date: 1947-03-12
  place: Тверь
death:
  place: Москва
```

A date is written as an ISO subset, optionally prefixed with an imprecision marker:

| Card | GEDCOM |
|---|---|
| `1995-07-25` | `25 JUL 1995` |
| `1995-07` | `JUL 1995` |
| `1995` | `1995` |
| `~1910` | `ABT 1910` |
| `<1910` | `BEF 1910` |
| `>1910` | `AFT 1910` |

Anything else is a compile error. **Precision the card does not state is never
invented:** a year-only date stays a year, and a place with no date is a perfectly
good event.

> One YAML wrinkle: `>` starts a block scalar, so an after-date has to be quoted —
> `date: '>1910'`. The other five forms are written as they appear above.

---

## Relationships

A card names its parents by id, and one card of a married pair carries the `marriage`:

```yaml
name: Иван
surname: Иванов
sex: M
father: pyotr-ivanov
mother: maria-sidorova
marriage:
  spouse: anna-petrova
  date: 1970-09-12
  place: Тверь
```

`FAM` records are never authored: the compiler derives them, one per distinct
(father, mother) pair — see [ADR 0001](docs/adr/0001-no-family-entities.md).
Children naming the same pair land in the same family and get `FAMC`, the parents get
`FAMS`, and a declared marriage becomes `MARR` with whatever date and place it carried.
**Remarriages need no special handling:** another pairing is another pair, and so
another `FAM`.

Either parent may be left out — a child with only a known mother yields a family with
one spouse. A `marriage` carrying neither date nor place is still worth writing: it is
what pairs a childless couple.

A marriage that ended carries a `divorce`, written inside it:

```yaml
marriage:
  spouse: anna-petrova
  date: 1970-09-12
  place: Тверь
  divorce:
    date: 1981-04
```

That becomes `1 DIV` right after the `MARR`, with whatever date and place it carried —
both optional, like the marriage's own. A bare `divorce:` says the marriage ended without
saying when, and compiles to `1 DIV Y`. It nests rather than sitting beside the marriage
so that with several marriages there is no question which one ended: the one the block is
written in.

Children come out in birth order, the ones with no birth date last.

The marriage goes on exactly one of the two cards; declaring it on both is a compile
error. So is naming an id no card has.

---

## Ids

An id is the card's file name without the extension, and it is what `father`, `mother`
and `spouse` reference. Use a latin transliteration — `ivan-ivanov`, and for namesakes
a birth-year suffix, `pyotr-ivanov-1947`. Lowercase latin letters, digits and single
inner hyphens only.

---

## Importing an existing tree

Already have a tree — a MyHeritage export, a file from another tool? `gedc import`
reads a GEDCOM 5.5.1 file and writes `tree.yaml` and one card per person into the
current directory, so you can move it onto this workflow without retyping anyone:

```bash
mkdir my-tree && cd my-tree
gedc import ~/Downloads/my-heritage-export.ged
```

Each person's id is derived from their name — the first given name and the surname,
transliterated to a latin slug (`Пётр Сергеевич /Иванов/` becomes `pyotr-ivanov`),
with a numeric suffix for namesakes. The scheme, and why it is not GOST or ICAO, is in
[ADR 0003](docs/adr/0003-import-transliteration-and-id-derivation.md).

> **Import is partial, and says so.** Today it reads the name (`NAME` with `GIVN`,
> `SURN` and `_MARNM`) and `SEX`. Birth and death dates and the `FAM` records that
> carry relationships have no card field yet — so rather than drop them silently and
> lose them on the next build, import stops and names every tag it cannot represent,
> writing nothing. Those fields land as the format is parsed further. A patronymic is
> a casualty of the same partiality in reverse: `GIVN` fuses it into the given name
> with no way to split it back out, so it stays in `name` rather than being guessed at
> (see [ADR 0002](docs/adr/0002-patronymic-joins-the-given-name.md)).

Import refuses to run where a `tree.yaml` already sits, rather than write over cards
it did not author.

---

## Errors

Every problem in the input is reported in one run, so you fix a batch at a time rather
than one error per rebuild. Because a mistyped id is the usual way a reference goes
wrong, an unknown one names the closest id there is:

```
error: ivan-ivanov: father: no card with id pyotr-ivanof, did you mean pyotr-ivanov?
error: pyotr-ivanov: birth.date: expected a date like 1995-07-25, 1995-07 or 1995, optionally prefixed with ~, < or >
error: pyotr-ivanov: age: unknown key
3 problem(s) found, family.ged not written
```

> **When anything is wrong, nothing is written.** A failed build leaves the previous
> `family.ged` untouched, and exits non-zero.

---

## Editor completion

`gedc schema` prints a [JSON Schema](https://json-schema.org) for the cards of the
current tree, with the ids baked into it — `father` lists the men, `mother` the women,
`marriage.spouse` everybody:

```bash
gedc schema > .vscode/people.schema.json
```

Point your editor at it. In VS Code, with the
[YAML extension](https://marketplace.visualstudio.com/items?itemName=redhat.vscode-yaml),
that is `.vscode/settings.json`:

```json
{
  "yaml.schemas": {
    "./.vscode/people.schema.json": "people/*.yaml"
  }
}
```

Typing `father:` now offers the ids there are, and a mistyped one, an unknown key, a
`sex` that is neither `M` nor `F` or a date the compiler would refuse gets a squiggle
where you typed it — rather than at the next build.

> **The schema goes stale when you add a person.** JSON Schema cannot read the card
> directory, which is why the ids have to be written into it; regenerate it when the
> cast changes. The output is deterministic, so regenerating without a change produces
> no diff.

---

## Library

The CLI is a thin wrapper over two seams, each taking text and returning text —
no filesystem involved, which is also how the tests drive them:

```rust
use gedcards::{Card, compile};

let config = "submitter: Иван Иванов\nlanguage: Russian\n";
let cards = [Card {
    id: "ivan-ivanov".to_string(),
    yaml: "name: Иван\nsurname: Иванов\nsex: M\n".to_string(),
}];

match compile(config, &cards) {
    Ok(ged) => print!("{ged}"),
    Err(diagnostics) => {
        for diagnostic in diagnostics {
            eprintln!("{diagnostic}");
        }
    }
}
```

`gedcards::schema(&cards)` is the other half of it: the same cards in, the JSON Schema
`gedc schema` prints out.

---

## Development

```bash
just          # list recipes
just build    # build the binary
just test     # run the test suite (68 tests)
just lint     # rustfmt + clippy
just check    # lint + test
```

`just install-tools` installs the extra cargo subcommands `just lint` depends on, and
`just install-hooks` installs the lefthook pre-push hook that runs `just check`.

Design decisions that were expensive to make are written down in [docs/adr/](docs/adr/),
and the background reading behind them in [docs/research/](docs/research/).

---

## License

MIT or Apache-2.0, at your option.
