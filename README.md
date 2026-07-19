# gedcards

Compile per-person YAML cards into a [GEDCOM 5.5.1](https://gedcom.io/specifications/ged551.pdf) file.

Editing `.ged` by hand is unpleasant and risky: it is a line-oriented format held
together by cross references like `@I42@`, and encodings are a recurring source of
mojibake. `gedcards` lets you keep one readable YAML file per person under version
control and compile them into a `.ged` that genealogy services accept.

The cards are the source of truth. The `.ged` is a derived artifact — never edited
by hand, and safe to delete and rebuild.

## Status

Early. Today a card carries a name, a surname and a sex, and the compiler emits
`INDI` records. Relationships (`father`/`mother`, marriages) and the synthesized
`FAM` records are the next milestone — see [ADR 0001](docs/adr/0001-no-family-entities.md)
for how families are meant to work.

Because a file without a single `FAM` record has no relationships to draw, some
viewers (Topola Viewer among them) will refuse to open the output until
relationships land.

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

Run from that directory:

```bash
gedc build
```

This writes `family.ged`. Output is byte-for-byte deterministic for the same input,
so a rebuild produces no spurious diff.

## Ids

An id is the card's file name without the extension, and it is what other cards will
reference once relationships land. Use a latin transliteration — `ivan-ivanov`, and
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
