# Religion, baptism/christening, and godparents in the GEDCOM spec

Researched 2026-07-24. All claims below were checked against the source that owns
them — the GEDCOM 5.5.1 PDF and the FamilySearch GEDCOM 7 registry at gedcom.io —
not secondary write-ups. Two versions are covered because they differ: **5.5.1**
(the long-dominant release, 1999) and **7.0** (the current maintained standard;
latest point release 7.0.18, February 2026, maintained by FamilySearch).
<https://gedcom.io/specifications/FamilySearchGEDCOMv7.html>

## The question

Does GEDCOM standardize (1) religious affiliation, (2) baptism/christening dates,
and (3) godparents/sponsors? For each: is it a standard tag or a custom
extension, what sub-structure does it carry, and how do 5.5.1 and 7.0 differ?

## Findings

### 1. Religion / religious affiliation — `RELI`, standard, free text, two roles

`RELI` is standard in both versions and its value is **free text**, never an
enumerated set.

- 5.5.1 Appendix A defines it: "RELI {RELIGION} — A religious denomination to
  which a person is affiliated or for which a record applies." Its primitive is
  `RELIGIOUS_AFFILIATION`, a free-text string. (5.5.1 spec, Appendix A, p.91;
  primitive p.60.) <https://gedcom.io/specifications/ged551.pdf>
- In 5.5.1 `RELI` plays **two structural roles**. It is a standalone individual
  attribute: the `INDIVIDUAL_ATTRIBUTE_STRUCTURE` lists `RELI
  <RELIGIOUS_AFFILIATION> {1:1}` alongside `OCCU`, `EDUC`, `NATI`, etc. (5.5.1
  spec, p.33). It is *also* a sub-structure of any event: `EVENT_DETAIL` lists
  `RELI <RELIGIOUS_AFFILIATION> {0:1}`, so a baptism, marriage, or death can
  carry the denomination it happened under (5.5.1 spec, p.32).
- GEDCOM 7 keeps both roles under two distinct terms. The individual attribute
  is `INDI-RELI`: "A religious denomination to which a person is affiliated or
  for which a record applies", payload an XSD string, `{0:M}` under an
  individual. <https://gedcom.io/terms/v7/INDI-RELI> The event/attribute
  sub-structure is `RELI`: "A religious denomination associated with the event
  or attribute described by the superstructure", also a free-text string,
  `{0:1}`, valid under 54 superstructures including `BAPM`, `CHR`, `BIRT`,
  `MARR`, `DEAT`. <https://gedcom.io/terms/v7/RELI>

### 2. Baptism / christening — a family of standard event tags

Both versions define a set of distinct religious-event tags. They are all
standard individual events; the LDS ordinances are a separate, differently
structured group. Quoting 5.5.1 Appendix A (pp.84–90):

- `BAPM {BAPTISM}` — "The event of baptism (not LDS), performed in infancy or
  later."
- `CHR {CHRISTENING}` — "The religious event (not LDS) of baptizing and/or
  naming a child."
- `CHRA {ADULT_CHRISTENING}` — "The religious event (not LDS) of baptizing
  and/or naming an adult person."
- `CONF {CONFIRMATION}` — "The religious event (not LDS) of conferring the gift
  of the Holy Ghost and, among protestants, full church membership."
- `FCOM {FIRST_COMMUNION}` — "A religious rite, the first act of sharing in the
  Lord's supper as part of church worship."
- `BLES {BLESSING}` — "A religious event of bestowing divine care or
  intercession. Sometimes given in connection with a naming ceremony."
- `ORDN {ORDINATION}` — "A religious event of receiving authority to act in
  religious matters."

In 5.5.1 these are grouped in the `INDIVIDUAL_EVENT_STRUCTURE`: `[ BIRT | CHR ]`,
`[ BAPM | BARM | BASM | BLES ]`, and `[ CHRA | CONF | FCOM | ORDN ]` (5.5.1 spec,
p.34). Each takes `INDIVIDUAL_EVENT_DETAIL`, which is `EVENT_DETAIL` plus
`AGE {0:1}` — so every one of them can carry `DATE`, a `PLACE_STRUCTURE`
(`PLAC`), `ADDRESS_STRUCTURE`, `AGE`, `AGNC`, `RELI`, `CAUS`, `TYPE`, notes and
sources (5.5.1 spec, pp.32, 34). The `CHR`/`BIRT` group additionally allows
`FAMC` (link to the family the child belongs to). Presence without a date/place
is asserted with a `Y` value, e.g. `1 BAPM Y` (5.5.1 spec, p.35).

The **LDS ordinances are separate** and carry a different sub-structure
(`TEMP` temple code, `STAT` ordinance status), not the ordinary event detail:
`BAPL {BAPTISM-LDS}` "performed at age eight or later by priesthood authority of
the LDS Church", and `CONL {CONFIRMATION_LDS}` "the religious event by which a
person receives membership in the LDS Church", both inside
`LDS_INDIVIDUAL_ORDINANCE` as `[ BAPL | CONL ]` with `DATE`, `TEMP`, `PLAC`,
`STAT` (5.5.1 spec, Appendix A p.84, structure p.36).

GEDCOM 7 carries the same tags forward with the same meanings and richer
per-tag pages:

- `BAPM` — "Baptism, performed in infancy or later. (See also `BAPL` and
  `CHR`.)", payload `Y|<NULL>`, `{0:M}` on an individual, substructures include
  `DATE`, `PLAC`, `AGE`, `RELI`, `CAUS`, `TYPE`, `ASSO`, notes, sources.
  <https://gedcom.io/terms/v7/BAPM>
- `CHR` — a child christening event, same detail set plus `FAMC`.
  <https://gedcom.io/terms/v7/CHR>
- `CHRA` — "Baptism or naming events for an adult person."
  <https://gedcom.io/terms/v7/CHRA>
- `CONF` — "Conferring full church membership." <https://gedcom.io/terms/v7/CONF>
- `FCOM` — "The first act of sharing in the Lord's supper as part of church
  worship." <https://gedcom.io/terms/v7/FCOM>
- `BLES` — "Bestowing divine care or intercession. Sometimes given in connection
  with a naming ceremony." <https://gedcom.io/terms/v7/BLES>
- `BAPL` — still an LDS ordinance with `TEMP` and `ord-STAT`, no `RELI`/`AGE`.
  <https://gedcom.io/terms/v7/BAPL>

### 3. Godparents / sponsors — no dedicated tag; use an association with a role

**Neither version has a `GODP`/`GODF`/`GODM` godparent tag as a top-level
event or attribute.** Recording a godparent means linking two individuals with
an association and naming the role.

- 5.5.1 has `ASSOCIATION_STRUCTURE`: `ASSO @<XREF:INDI>@ {1:1}` with a
  **required** `+1 RELA <RELATION_IS_DESCRIPTOR> {1:1}`, plus source and note.
  "The association pointer only associates INDIvidual records to INDIvidual
  records." (5.5.1 spec, p.31.) `RELA {RELATIONSHIP}` is "A relationship value
  between the indicated contexts" — **free text**, no enumerated set (Appendix A
  p.91). So in 5.5.1 a godparent is `1 ASSO @I2@` / `2 RELA Godfather` — standard
  grammar, but the word "Godfather" is uncontrolled free text, and the ASSO
  hangs off the person, not off the specific baptism event. `ASSO {ASSOCIATES}`
  itself is defined only as "An indicator to link friends, neighbors, relatives,
  or associates of an individual" (Appendix A p.84). Any explicitly named
  `_GODP`-style tag would be a **custom extension** — 5.5.1 requires user tags to
  begin with an underscore to signal a non-standard construct (Appendix A intro,
  p.83).

- GEDCOM 7 makes this materially more standard. `ROLE` takes an **enumerated
  value**, and the set at `enumset-ROLE` includes `GODP`, defined as "Godparent
  or related role in other religions."
  <https://gedcom.io/terms/v7/enum-GODP> The full role set is `CHIL, CLERGY,
  FATH, FRIEND, GODP, HUSB, MOTH, MULTIPLE, NGHBR, OFFICIATOR, PARENT, SPOU,
  WIFE, WITN, OTHER`. <https://gedcom.io/terms/v7/enumset-ROLE> In v7 `ASSO`
  is "A pointer to an associated individual" whose nature is given by a
  **required** `ROLE` substructure, with `PHRASE` for free-text refinement.
  <https://gedcom.io/terms/v7/ASSO> Crucially, `ASSO` is a `{0:M}` substructure
  of the event tags themselves (it appears in the `BAPM` substructure list), so a
  godparent can be attached directly to a baptism: `1 BAPM` / `2 ASSO @I2@` /
  `3 ROLE GODP`. That is the standard, enumerated, event-scoped path 5.5.1 lacked.
  A `_GODP` custom tag remains unnecessary in v7.

## Practical implications for this tool

- **Safe to emit as standard GEDCOM (both 5.5.1 and 7.0):** religious
  affiliation as `RELI` (either an individual attribute or a sub-line of an
  event), and baptism/christening as the dedicated events `BAPM`, `CHR`, `CHRA`,
  `CONF`, `FCOM`, `BLES`, `ORDN`. Each accepts `DATE`, `PLAC`, and `AGE`, so a
  card can render a baptism date/place/age with no extension tags. `RELI` values
  are free text — do not try to enumerate denominations.
- **Do not emit LDS ordinance tags** (`BAPL`, `CONL`) for ordinary religious
  baptisms — they mean the LDS ordinance specifically and carry `TEMP`/`STAT`,
  not the general event detail. For a normal christening, `CHR`/`BAPM` is
  correct.
- **Godparents have no standard dedicated tag.** If targeting 5.5.1, the only
  standard option is `ASSO`+`RELA` with a free-text descriptor (person-scoped),
  or a clearly-marked `_GODP`-style custom extension. If targeting GEDCOM 7,
  prefer the standard path: an `ASSO` with the enumerated `ROLE GODP`, ideally
  nested under the `BAPM`/`CHR` event so the godparent is tied to that specific
  rite. This is the one topic where moving to v7 buys a genuinely more standard
  representation.
