# gedcards

One YAML card per person, compiled to a GEDCOM 5.5.1 file that genealogy software
imports. The words below are the ones this project has settled on; the reasoning
behind the harder choices is in [docs/adr/](docs/adr/).

## Language

**Tree**:
One family's whole input — a directory holding `tree.yaml` and a `people/` folder of
cards. It is what `gedc build` compiles and what `gedc import` bootstraps.
_Avoid_: project, dataset, database

**Card**:
One person's YAML file, and the source of truth about that person. The `.ged` is a
derived artifact, never edited by hand.
_Avoid_: person file, record, profile, node

**Id**:
A card's file name without the extension, and the only way one card names another
(`father`, `mother`, `spouse`). It leads with the surname at birth, or with the
married surname on a card that has no other.
_Avoid_: key, handle, reference

**Slug**:
The shape an id must take: lowercase latin letters, digits and single inner hyphens.
Import transliterates a Cyrillic name into a slug to build an id from.
_Avoid_: transliteration (for the result), identifier

**Event block**:
Something dated and/or placed that happened to one person — `birth`, `death`,
`burial` and the rites are all the same block, carrying any of `date`, `place`,
`coords`, `note`, `age` and `cause`.
_Avoid_: event record, fact

**Marriage block**:
The `marriage` written on exactly one spouse's card, naming the other by id and
optionally nesting a `divorce`. It is what pairs two people; declaring it on both
cards is a compile error.
_Avoid_: union, spouse block

**Rite**:
One of the four religious events a card can carry — `christening`, `baptism`,
`confirmation`, `first_communion`. These are the ordinary events, not the LDS
ordinances, which mean something else.
_Avoid_: sacrament, ordinance

**Family**:
A (father, mother) pair the cards imply rather than declare, synthesized at build
time — one per distinct pair. It is never authored and has no id of its own.
_Avoid_: household, family file, FAM entity

**Diagnostic**:
One problem found in one build, naming the card, the field and why. There is no
severity axis: every diagnostic is fatal, and one of them means nothing is written.
_Avoid_: warning, validation error
