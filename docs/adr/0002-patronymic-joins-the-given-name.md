# The patronymic is a card field, joined into the given name on the way out

A card carries `patronymic` as its own field, and the compiler joins it to `name`
with a space when it emits GEDCOM: `1 NAME Иван Петрович /Иванов/` with
`2 GIVN Иван Петрович`. The card keeps the two apart because they are different
facts; GEDCOM gets them fused because it has nowhere else to put a patronymic.

Neither GEDCOM version has a patronymic. 5.5.1 and 7 both define the same six name
pieces — `NPFX`, `GIVN`, `NICK`, `SPFX`, `SURN`, `NSFX` — and no seventh. So there is
no canonical answer here, only a choice worth recording.

The choice is validated by GEDCOM 7, which states that the `NAME` payload "shall be
seen as the primary name representation, with name pieces as optional auxiliary
information; in particular it is recommended that all name parts in
`PERSONAL_NAME_PIECES` appear within the `PersonalName` payload in some form." Our
patronymic appears in the payload, and the payload is what a reader displays. The
5.5.1 grammar for `NAME_PERSONAL` allows the same shape — its own example is
`William Lee /Parry/`, a multi-word given portion before the slashed surname.

A comma — `2 GIVN Иван,Петрович` — was rejected. 5.5.1 says "Different given names
are separated by a comma", but a patronymic is not a second given name: «Пётр» and
«Петрович» are not two personal names the way «William» and «Lee» are, and the comma
would assert a relationship that does not hold. GEDCOM 7 removed the sentence
entirely, leaving `GIVN` as "a given or earned name" with no separator guidance, so
the only argument for the comma no longer exists in the newer version. Note that a
comma is impossible in the `NAME` line regardless: `NAME_TEXT` is defined as `<TEXT>`
"excluding commas".

A `_PATR` extension was also rejected. GEDCOM 7 does formalize extension tags with
URIs, so it would be legal — but nothing reads it. MyHeritage has no patronymic tag
of any kind: an export checked on 2026-07-19 carries only `GIVN`, `SURN` and their
own `_MARNM` under `NAME`, with no `NPFX`, `NSFX`, `NICK` or `SPFX` anywhere. Writing
`_PATR` would be writing to nobody. It becomes worth revisiting the day a real
consumer appears.

The price is that the join is one-way. Importing our output into a service and
exporting it back returns the patronymic fused inside `GIVN`, and it cannot be split
out again: suffix matching on `-ович`/`-евна` is a guess that breaks on non-Russian
names and on compound given names like «Анна Мария». No such heuristic is in the
compiler, and none should be added — the cards stay the source of truth and the
`.ged` stays a derived artifact, exactly as the README describes.
