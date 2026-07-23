# Import derives ids by transliteration, and stops on what it cannot yet represent

`gedc import` reads a GEDCOM 5.5.1 file back into cards. Three decisions the issue
(#7) left open are settled here: how a card gets its id, what transliteration turns
a Cyrillic name into a latin one, and what happens to a tag no card field holds yet.

## The id is the first given name and the surname, transliterated

An id is a latin slug — `ivan-ivanov` — and a `.ged` gives names in Cyrillic, so
import has to transliterate. The id is built from the first word of `GIVN` and the
`SURN`: `Пётр Сергеевич /Иванов/` becomes `pyotr-ivanov`. The patronymic is left out
of the id, the way the README's authored ids leave it out, even though it stays in
the `name` field (see below).

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
tree (`pyotr-ivanov`, `sidorova`, `kuznetsova`) because it favours the conventional
reading. Reversibility is not a goal: an id is a handle, not a name to reconstruct.

The table is only defined for Russian. A letter it does not know is dropped from the
slug; a name with no latin-mappable letters at all cannot yield an id, and is
reported rather than given an empty one.

**Ids do not round-trip to the authored ones, and need not.** `Мария` transliterates
to `mariya` where the card was hand-named `maria`; a patronymic that shaped an
authored id (`olga-ivanova` for a woman with surname `Смирнова`) is gone. This is
fine: an id appears nowhere in the emitted `.ged` — cross-references are `@I1@` by
sorted position — so as long as the derived ids sort in the same order, a
build → import → build round trip is byte-identical regardless.

## Namesakes get a numeric suffix in file order

Two people whose names transliterate to the same slug collide. The README's answer
is a birth-year suffix (`pyotr-ivanov-1947`), but birth dates are not imported yet
(they arrive with #1), so there is no year to suffix with. Until then, collisions
take a numeric suffix in the order the file lists them: `ivan-ivanov`,
`ivan-ivanov-2`, `ivan-ivanov-3`. It is deterministic for a given file, which is all
an id has to be. When #1 lands, the year is the better disambiguator and this becomes
the fallback for the yearless.

## A tag with no card field stops the import, naming it

A `.ged` written by another tool carries tags this schema has no field for yet —
`BIRT` and `DEAT` dates (#1), the `FAM` records that hold relationships (#4), name
pieces like `NPFX`. Dropping them silently would mean the next `gedc build` quietly
emits a `.ged` missing facts the original had. So import does what `compile` does:
reports every such tag in one run, names the record it sits in, and writes nothing
when there is anything to report. Import stays honest about being partial rather than
lossy, and grows a field at a time as #1 and #4 land.

The header is the exception. `SOUR`, `DEST`, `DATE`, `FILE` and the rest are metadata
`gedc build` regenerates from scratch — none of it is a person's fact — so import
reads only `LANG` and the submitter's `NAME` out of the header and ignores the rest
without complaint. Strict about people, lenient about the envelope they came in.
