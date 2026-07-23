# A married surname without a birth one is emitted as a name typed `married`

A card carries at least one of `surname` and `married_surname` rather than `surname`
unconditionally: a person a tree knows only by the name they took at marriage never had
a birth surname recorded, and putting the married name in `surname` would say the wrong
thing — to `SURN`, to the id, and to whoever reads the card next. When `surname` is
absent, the married surname is the only name GEDCOM can be given, so it is what the
`NAME` line and `SURN` carry, and the name is tagged `2 TYPE married`.

The type is what keeps the round trip whole. Without it, `SURN` alone says "surname at
birth", so `gedc import` would read the married name back into `surname` and
`build → import → build` would stop reproducing the authored card — the first field in
this schema not to survive its own round trip. `TYPE married` removes the need for that
exception rather than documenting it: the tag says the name was not the one at birth, so
import puts it back where it came from. No `_MARNM` is written in this case; it exists to
contrast a married surname with a birth one, and here there is nothing to contrast.

`NAME_TYPE` is standard in both versions, unlike `_MARNM` — which is MyHeritage's
extension, adopted here for the ordinary case where a card has both surnames (see
[ADR 0002](0002-patronymic-joins-the-given-name.md) for the export it was checked
against). 5.5.1 defines `PERSONAL_NAME_STRUCTURE` as `NAME` with `+1 TYPE <NAME_TYPE>`
before `<<PERSONAL_NAME_PIECES>>`, which is why `TYPE` is emitted before `GIVN` and
`SURN`; `NAME_TYPE` is `[ aka | birth | immigrant | maiden | married | <user defined> ]`,
with `married` glossed as a "previous married name". GEDCOM 7 keeps the same enumeration
spelled in upper case (`AKA`, `BIRTH`, `IMMIGRANT`, `MAIDEN`, `MARRIED`, `PROFESSIONAL`,
`OTHER`). The output declares `2 VERS 5.5.1`, so it is spelled the way that version
spells it — lower case — while import accepts either, since a file may come from a 7.0
tool.

Import reads `married` and no other type. `birth` and `maiden` would land in `surname`
where an untyped `NAME` already lands, and `aka`, `immigrant` and the rest are
distinctions no card field holds; rather than guess, a name of any other type is reported
the way every other unheld fact is. The export checked on 2026-07-19 writes no `TYPE` at
all, so there is no file to be lenient for yet.

The alternative was the one the spec really intends: several `NAME` structures per
person, each typed — a `birth` one and a `married` one — instead of one `NAME` plus
`_MARNM`. It was declined for the same reason a list-valued `married_surname` was: the
model this schema has is one name per person, and nothing has yet needed a second. That
remains the idiomatic answer the day a card must carry two married surnames, or a name
this file format has no field for.

One consequence for ids: a card with only a married surname leads its id with that
surname, because `derive_id` is given the same fallback. The rule stays "the id leads
with the surname the card carries" — for every other card that is the birth surname, and
leading with a married surname while a birth one exists still breaks the byte-identity
of the round trip.
