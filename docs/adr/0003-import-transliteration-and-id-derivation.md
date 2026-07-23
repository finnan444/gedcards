# Import derives ids by transliteration, and stops on what it cannot yet represent

`gedc import` reads a GEDCOM 5.5.1 file back into cards. Three decisions the issue
(#7) left open are settled here: how a card gets its id, what transliteration turns
a Cyrillic name into a latin one, and what happens to a tag no card field holds yet.

## The id is the surname and the first given name, transliterated

An id is a latin slug — `ivanov-ivan` — and a `.ged` gives names in Cyrillic, so
import has to transliterate. The id is built from the `SURN` and the first word of
`GIVN`: `Пётр Сергеевич /Иванов/` becomes `ivanov-pyotr`. The surname leads so a
`people/` directory sorts into families — every Ivanov adjacent, the maiden-name
women beside their parents — rather than scattering by given name. The patronymic is
left out of the id, the way the README's authored ids leave it out, even though it
stays in the `name` field (see below).

The transliteration is a fixed table, Russian Cyrillic to latin, chosen to read the
way these names usually do in latin script rather than to satisfy a formal standard:

```
а a  б b  в v  г g  д d  е e  ё yo  ж zh  з z  и i  й y  к k  л l  м m  н n  о o
п p  р r  с s  т t  у u  ф f  х kh  ц ts  ч ch  ш sh  щ shch  ъ ‹drop›  ы y
ь ‹drop›  э e  ю yu  я ya
```

GOST 7.79 and ICAO Doc 9303 were the named alternatives. Both are built for
reversible passport-style transcription and spell `ё` as `e`, `ю` as `iu`, `я` as
`ia` — `Пётр` becomes `Petr`, not `Pyotr`. This table matches the ids already in the
tree (`ivanov-pyotr`, `sidorova`, `kuznetsova`) because it favours the conventional
reading. Reversibility is not a goal: an id is a handle, not a name to reconstruct.

The table is only defined for Russian. A letter it does not know is dropped from the
slug; a name with no latin-mappable letters at all cannot yield an id, and is
reported rather than given an empty one.

**Ids do not round-trip to the authored ones, and need not.** `Мария` transliterates
to `mariya` where the card was hand-named `maria`; a patronymic that shaped an
authored id (`ivanova-olga` for a woman with surname `Смирнова`) is gone. This is
fine: an id appears nowhere in the emitted `.ged` — cross-references are `@I1@` by
sorted position — so as long as the derived ids sort in the same order, a
build → import → build round trip is byte-identical regardless.

## Namesakes get the birth-year suffix, and a numeric one where that will not do

Two people whose names transliterate to the same slug collide. The README's answer is
a birth-year suffix (`ivanov-pyotr-1947`), and birth dates now import, so that is what
import uses: the first of a name keeps the bare slug, and a namesake gains the year of
their birth. Where even that will not separate them — two namesakes born the same year,
or one with no birth date — a numeric suffix in file order is the last resort:
`ivanov-ivan`, `ivanov-ivan-2`, `ivanov-ivan-3`. Both are deterministic for a given
file, which is all an id has to be.

## A tag with no card *field* stops the import, naming it — but bookkeeping does not

A `.ged` written by another tool carries tags this schema still has no field for —
name pieces like `NPFX`, a `SOUR` citation, an `OCCU`, a date in a form the card
grammar cannot spell. Dropping them silently would mean the next `gedc build` quietly
emits a `.ged` missing facts the original had. So import does what `compile` does:
reports every such tag in one run, names the record it sits in, and writes nothing
when there is anything to report. Import stays honest about being partial rather than
lossy, and grows a field at a time — the birth, death and burial events, and the `FAM`
records that hold relationships, have since grown theirs.

The line this draws is between a person's **fact** and the **envelope** it arrived in.
A fact — a date, a place, a marriage, a name piece — is named when it cannot be
represented. Envelope is dropped in silence, because regenerating it loses nothing:

- The **header**. `SOUR`, `DEST`, `DATE`, `FILE` and the rest are metadata `gedc build`
  writes fresh, so import reads only `LANG` and the submitter's `NAME` and ignores the
  rest.
- **Record bookkeeping.** A real export is dense with a tool's own database keys and
  timestamps — MyHeritage alone writes `_UID`, `_UPD`, `RIN` on nearly every `INDI`,
  and a full tree drowns in them. These are not facts about a person; they are how one
  program tracked its rows, meaningless to another. So a `_`-prefixed vendor extension
  (GEDCOM reserves the underscore for exactly this — the one such tag we have adopted,
  `_MARNM`, is read as the fact it is) and the standard-but-key `RIN` are dropped like
  header metadata. A standard genealogical tag stays strict: `BIRT` is a fact, and is
  named.

Strict about people, lenient about the envelope they came in — whether that envelope
is the file's header or a vendor's bookkeeping stamped onto each record.
