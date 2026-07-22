# gedcards

Compile per-person YAML cards into a [GEDCOM 5.5.1](https://gedcom.io/specifications/ged551.pdf) file.

Editing `.ged` by hand is unpleasant and risky: it is a line-oriented format held
together by cross references like `@I42@`, and encodings are a recurring source of
mojibake. `gedcards` lets you keep one readable YAML file per person under version
control and compile them into a `.ged` that genealogy services accept.

The cards are the source of truth. The `.ged` is a derived artifact — never edited
by hand, and safe to delete and rebuild.

## Status

Early. Today a card carries a full name, a sex, birth/death events and its
relationships, and the compiler emits `INDI` and `FAM` records — enough for a
tree a viewer can draw.

## Install

```bash
cargo install --git https://github.com/finnan444/gedcards
```

## Use

Lay out a directory like this:

```
tree.yaml
people/
  ivan-ivanov.yaml
  maria-sidorova.yaml
```

`tree.yaml` holds what goes into the GEDCOM header:

```yaml
submitter: Иван Иванов
language: Russian
```

Each card is one person. The file name without its extension is that person's id:

```yaml
name: Иван
surname: Иванов
sex: M
```

`patronymic` and `married_surname` are optional:

```yaml
name: Мария
patronymic: Петровна
surname: Сидорова
married_surname: Иванова
sex: F
```

The patronymic is part of the name — it compiles to `1 NAME Мария Петровна /Сидорова/`
with `2 GIVN Мария Петровна` — but it stays its own field on the card rather than being
glued onto `name`.

`surname` is always the surname at birth. A `married_surname` is emitted as `2 _MARNM`,
the extension MyHeritage uses, which is what makes maiden names display correctly after
import.

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

A date is written as an ISO subset, optionally prefixed with an imprecision
marker:

| Card | GEDCOM |
|---|---|
| `1995-07-25` | `25 JUL 1995` |
| `1995-07` | `JUL 1995` |
| `1995` | `1995` |
| `~1910` | `ABT 1910` |
| `<1910` | `BEF 1910` |
| `>1910` | `AFT 1910` |

Anything else is a compile error. Precision the card does not state is never
invented: a year-only date stays a year, and a place with no date is a perfectly
good event.

One YAML wrinkle: `>` starts a block scalar, so an after-date has to be quoted —
`date: '>1910'`. The other five forms are written as they appear above.

## Relationships

A card names its parents by id, and one card of a married pair carries the
`marriage`:

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
Children naming the same pair land in the same family and get `FAMC`, the
parents get `FAMS`, and a declared marriage becomes `MARR` with whatever date
and place it carried. Remarriages need no special handling: another pairing is
another pair, and so another `FAM`.

Either parent may be left out — a child with only a known mother yields a family
with one spouse. A `marriage` carrying neither date nor place is still worth
writing: it is what pairs a childless couple.

Children come out in birth order, the ones with no birth date last.

The marriage goes on exactly one of the two cards; declaring it on both is a
compile error. So is naming an id no card has — and because the usual cause is
a typo, that diagnostic names the closest id there is.

Run from that directory:

```bash
gedc build
```

This writes `family.ged`. Output is byte-for-byte deterministic for the same input,
so a rebuild produces no spurious diff.

## Ids

An id is the card's file name without the extension, and it is what `father`,
`mother` and `spouse` reference. Use a latin transliteration — `ivan-ivanov`, and
for namesakes a birth-year suffix, `pyotr-ivanov-1947`. Lowercase latin letters,
digits and single inner hyphens only.

## Errors

Every problem in the input is reported in one run, so you fix a batch at a time
rather than one error per rebuild. When anything is wrong, nothing is written:

```
error: иван: id must be a slug of lowercase latin letters, digits and hyphens
error: иван: sex: expected M or F
error: иван: age: unknown key
3 problem(s) found, family.ged not written
```

## Library

The CLI is a thin wrapper over a single seam, which takes text and returns text —
no filesystem involved, which is also how the tests drive it:

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

## Development

```bash
just          # list recipes
just build    # build the binary
just test     # run the test suite
just lint     # rustfmt + clippy
just check    # lint + test
```

## License

MIT or Apache-2.0, at your option.
