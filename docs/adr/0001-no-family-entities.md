# A family is not an entity: FAM records are synthesized

The source of truth is a YAML card per person, filled in by hand. `FAM` records,
which GEDCOM 5.5.1 requires, are produced by the compiler instead of being
authored: a family is a unique (father, mother) pair taken from the children's
cards, plus any marriage declared with a `marriage` block on exactly one of the
two spouses' cards.

The alternative — explicit family files mirroring GEDCOM — was rejected. It would
force the author to invent family ids and to edit two places whenever a child is
added, whereas "X is the child of Y and Z" on the child's card keeps each fact in
exactly one place.

The price is the synthesis logic itself and the rule that a marriage is declared
once, with a double declaration being a compile error. Remarriages and children
from different pairings need no special handling: each new parent pair simply
yields another `FAM`.
