//! The inverse of `compile`: a GEDCOM 5.5.1 file in, `tree.yaml` and one card
//! per `INDI` out. What a card has a field for is read back — the name and sex,
//! the birth, death and burial events with their dates, places, ages and causes,
//! and the relationships the `FAM` records hold: parents, marriages and divorces.
//! A tag with no card field yet (a name piece like `NPFX`, a `SOUR` citation) is
//! not dropped silently but named, so the next `gedc build` cannot quietly lose
//! what this run could not represent.
//!
//! The header is where import is lenient: `SOUR`, `DATE`, `FILE` and the rest are
//! metadata `gedc build` regenerates from scratch, so skipping them loses nothing
//! about a person. Only `LANG` and the submitter's name are read back, since
//! `tree.yaml` needs them. A tool's per-record bookkeeping — a vendor's `_UID`,
//! `_UPD`, the standard-but-key `RIN` — is dropped the same way: see
//! docs/adr/0003-import-transliteration-and-id-derivation.md.
//!
//! `FAM` records are not authored on this workflow — the compiler derives them
//! from the cards — so import reads them back the other way: a family's `HUSB`
//! and `WIFE` become the `father` and `mother` on each child's card, and its
//! `MARR`/`DIV` become the `marriage` block on one spouse's card. Which spouse
//! carries it is chosen so no card carries two, the way a hand-authored tree
//! never would — see `assign_marriages`.

use std::collections::{HashMap, HashSet};

use crate::{Card, Diagnostic};

/// One parsed GEDCOM line: its level, an optional cross-reference id (the
/// `@I1@` on a record's opening line), the tag, and the value after it.
struct Line<'a> {
    level: u8,
    xref: Option<&'a str>,
    tag: &'a str,
    value: Option<&'a str>,
}

/// A life event as import recovers it: the pieces a card's event block holds,
/// each in card spelling. The date is already the ISO subset a card writes,
/// not the GEDCOM one it was read from.
#[derive(Default, Clone)]
struct RawEvent {
    date: Option<String>,
    place: Option<String>,
    age: Option<String>,
    cause: Option<String>,
}

impl RawEvent {
    /// Whether the block carries any fact. A bare `1 BIRT` with nothing under it
    /// asserts an event with no data a card can hold, so import skips it rather
    /// than writing an empty block the compiler would reject.
    fn is_empty(&self) -> bool {
        self.date.is_none() && self.place.is_none() && self.age.is_none() && self.cause.is_none()
    }
}

/// A person as import recovers them, keyed by their `@I1@` xref so the `FAM`
/// records can point back. `given` is the whole `GIVN` value: a patronymic fused
/// into it on the way out cannot be split back off — see
/// docs/adr/0002-patronymic-joins-the-given-name.md — so it stays part of the
/// name rather than being guessed at.
struct Individual<'a> {
    xref: &'a str,
    label: String,
    given: Option<String>,
    surname: Option<String>,
    married_surname: Option<String>,
    sex: Option<String>,
    birth: Option<RawEvent>,
    death: Option<RawEvent>,
    burial: Option<RawEvent>,
}

/// A family as import recovers it. The spouses and children are still xrefs;
/// they become card ids once every person has one. `marriage` and `divorce` are
/// `Some` whenever the `MARR`/`DIV` tag was present, even bare (`1 MARR Y`),
/// which is how a childless couple or an undated divorce still comes through.
struct Family<'a> {
    xref: &'a str,
    husband: Option<&'a str>,
    wife: Option<&'a str>,
    children: Vec<&'a str>,
    marriage: Option<RawEvent>,
    divorce: Option<RawEvent>,
}

/// A marriage as it will sit on one spouse's card: the other spouse by id, the
/// event, and how it ended if it did.
struct CardMarriage {
    spouse: String,
    event: RawEvent,
    divorce: Option<RawEvent>,
}

/// The single import seam: GEDCOM text in, `tree.yaml` text plus one card per
/// `INDI` out, or the diagnostics that say why nothing could be written. Like
/// `compile`, every problem is reported together and, when there is any, nothing
/// is produced.
///
/// `submitter`, when given, is the tree's submitter — the person now running the
/// import naming themselves — in place of whatever `SUBM` the file recorded, which
/// an export often leaves out. Without it, the file's `SUBM` name is read, and a
/// file that carries none is a problem: `tree.yaml` cannot go without.
pub fn import(ged: &str, submitter: Option<&str>) -> Result<(String, Vec<Card>), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    // A UTF-8 BOM leads a file MyHeritage and other Windows tools export;
    // stripped here so the `0 HEAD` it hides in front of parses like any line.
    let ged = ged.strip_prefix('\u{feff}').unwrap_or(ged);
    let lines = parse_lines(ged, &mut diagnostics);

    let mut language: Option<String> = None;
    let mut file_submitter: Option<String> = None;
    let mut individuals: Vec<Individual> = Vec::new();
    let mut families: Vec<Family> = Vec::new();

    // Walk the level-0 records. Each record is the opening line plus every line
    // under it until the next level-0 line.
    let mut start = 0;
    while start < lines.len() {
        let mut next = start + 1;
        while next < lines.len() && lines[next].level != 0 {
            next += 1;
        }
        let body = &lines[start..next];
        let head = &body[0];
        // The header and the submitter record are read narrowly on purpose —
        // one field each, the rest of the metadata ignored — see the module
        // comment. A field only present in a later record still wins, and one
        // absent there never clobbers what an earlier record supplied.
        match head.tag {
            "HEAD" => language = read_level1(body, "LANG").or(language),
            "SUBM" => file_submitter = read_level1(body, "NAME").or(file_submitter),
            "INDI" => individuals.push(read_individual(body, &mut diagnostics)),
            "FAM" => families.push(read_family(body, &mut diagnostics)),
            "TRLR" => {}
            _ => diagnostics.push(record_problem(
                head,
                &format!("unexpected {} record", head.tag),
            )),
        }
        start = next;
    }

    // The supplied submitter wins over the file's: it is the tree's new owner
    // naming themselves, and an export that carried a SUBM often names some
    // exporting account rather than a person anyway.
    let submitter = submitter.map(String::from).or(file_submitter);

    // tree.yaml needs both, and the compiler will refuse a card set without
    // them: better to say so here, naming the missing header fact, than to write
    // a tree.yaml that will not build.
    if language.is_none() {
        diagnostics.push(header_problem(
            "language",
            "no LANG in the header to import",
        ));
    }
    if submitter.is_none() {
        diagnostics.push(header_problem(
            "submitter",
            "no SUBM record with a NAME to import, and no --submitter given",
        ));
    }

    let ids = assign_ids(&individuals, &mut diagnostics);
    let known: HashSet<&str> = individuals.iter().map(|person| person.xref).collect();
    let parents = link_parents(&families, &ids, &known, &mut diagnostics);
    let marriages = assign_marriages(&families, &ids, &known, &mut diagnostics);

    let cards = build_cards(&individuals, &ids, &parents, &marriages);

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let language = language.expect("no diagnostics means the header had a LANG");
    let submitter = submitter.expect("no diagnostics means a submitter was supplied");
    let tree_yaml = format!("submitter: {submitter}\nlanguage: {language}\n");
    Ok((tree_yaml, cards))
}

/// Splits the file into lines, dropping blank ones and reporting any that are
/// not `level tag [value]`. A `\r` from a CRLF file is trimmed, so a tree
/// exported on Windows reads the same as one written here.
fn parse_lines<'a>(ged: &'a str, diagnostics: &mut Vec<Diagnostic>) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    for raw in ged.lines() {
        let text = raw.trim_end_matches('\r');
        if text.trim().is_empty() {
            continue;
        }
        match parse_line(text) {
            Some(line) => lines.push(line),
            None => diagnostics.push(Diagnostic {
                card: None,
                field: None,
                reason: format!("not a GEDCOM line: {text:?}"),
            }),
        }
    }
    lines
}

/// Parses one line. A line is a level, then either an `@xref@` and a tag or a
/// bare tag, then an optional value — the value kept verbatim, spaces and all.
/// A trailing space, which MyHeritage leaves on some lines, is trimmed off the
/// value so it does not ride into a card field the compiler would then reject.
fn parse_line(text: &str) -> Option<Line<'_>> {
    let (level, rest) = text.split_once(' ').unwrap_or((text, ""));
    let level: u8 = level.parse().ok()?;
    let (xref, rest) = if rest.starts_with('@') {
        let (xref, rest) = rest.split_once(' ')?;
        (Some(xref), rest)
    } else {
        (None, rest)
    };
    if rest.is_empty() {
        return None;
    }
    let (tag, value) = match rest.split_once(' ') {
        Some((tag, value)) => (tag, Some(value.trim_end())),
        None => (rest, None),
    };
    let value = value.filter(|value| !value.is_empty());
    Some(Line {
        level,
        xref,
        tag,
        value,
    })
}

/// The value of the last level-1 line with this tag, or None. It is how the one
/// field each of the header (`LANG`) and the submitter record (`NAME`) is pulled
/// out while the rest of that record's metadata is left alone.
fn read_level1(body: &[Line], tag: &str) -> Option<String> {
    body[1..]
        .iter()
        .rfind(|line| line.level == 1 && line.tag == tag)
        .and_then(|line| line.value.map(String::from))
}

/// Reads one `INDI`. `NAME` (with `GIVN`, `SURN`, `_MARNM`), `SEX`, and the
/// `BIRT`/`DEAT`/`BURI` events map to card fields; `FAMC`/`FAMS` are links the
/// `FAM` records carry the substance of, consumed here in silence. Any other tag
/// is a fact this card cannot hold yet, so it is named rather than dropped. A
/// person with a missing name or sex is still returned carrying its diagnostics —
/// `assign_ids` skips the id it cannot have, and the run fails anyway with
/// nothing written.
fn read_individual<'a>(body: &'a [Line], diagnostics: &mut Vec<Diagnostic>) -> Individual<'a> {
    let head = &body[0];
    let mut person = Individual {
        xref: record_label_ref(head),
        label: record_label(head),
        given: None,
        surname: None,
        married_surname: None,
        sex: None,
        birth: None,
        death: None,
        burial: None,
    };

    for (line, children) in level1(body) {
        match line.tag {
            // The NAME line's own value is ignored: GIVN and SURN carry the same
            // pieces split out, which is what the card fields want.
            "NAME" => read_name(head, children, &mut person, diagnostics),
            "SEX" => person.sex = line.value.map(String::from),
            "BIRT" => person.birth = read_event(head, "BIRT", children, diagnostics),
            "DEAT" => person.death = read_event(head, "DEAT", children, diagnostics),
            "BURI" => person.burial = read_event(head, "BURI", children, diagnostics),
            // The relationships these point at are read from the FAM records,
            // so the links themselves are envelope here, dropped like the header.
            "FAMC" | "FAMS" => {}
            tag if is_bookkeeping(tag) => {}
            tag => diagnostics.push(record_problem(head, &format!("{tag} is not imported yet"))),
        }
    }

    if person.given.is_none() {
        diagnostics.push(record_problem(head, "no NAME with a GIVN to import"));
    }
    if person.surname.is_none() {
        diagnostics.push(record_problem(head, "no NAME with a SURN to import"));
    }
    if person.sex.is_none() {
        diagnostics.push(record_problem(head, "no SEX to import"));
    }
    person
}

/// Reads one `FAM`. `HUSB`, `WIFE` and `CHIL` are the links that become parents
/// on the cards, and `MARR`/`DIV` the marriage a spouse's card will carry. Their
/// presence is kept even when bare, so a childless couple or an undated divorce
/// survives. Anything else the record holds is named rather than dropped.
fn read_family<'a>(body: &'a [Line], diagnostics: &mut Vec<Diagnostic>) -> Family<'a> {
    let head = &body[0];
    let mut family = Family {
        xref: record_label_ref(head),
        husband: None,
        wife: None,
        children: Vec::new(),
        marriage: None,
        divorce: None,
    };

    for (line, children) in level1(body) {
        match line.tag {
            "HUSB" => family.husband = line.value.or(family.husband),
            "WIFE" => family.wife = line.value.or(family.wife),
            "CHIL" => family.children.extend(line.value),
            // A MARR or DIV is asserted by the tag alone; the block, empty or not,
            // stands for the fact. age/cause have no place on a family event, so
            // read_event names them if a file carries them here.
            "MARR" => {
                family.marriage =
                    Some(read_event(head, "MARR", children, diagnostics).unwrap_or_default())
            }
            "DIV" => {
                family.divorce =
                    Some(read_event(head, "DIV", children, diagnostics).unwrap_or_default())
            }
            tag if is_bookkeeping(tag) => {}
            tag => diagnostics.push(record_problem(head, &format!("{tag} is not imported yet"))),
        }
    }
    family
}

/// The level-1 lines of a record, each paired with the block of deeper lines
/// under it — everything until the next level-1 line or the end of the record.
/// It is the one place both `read_individual` and `read_family` walk a record's
/// structure, so the level bookkeeping lives here alone.
fn level1<'a, 'b>(body: &'b [Line<'a>]) -> Vec<(&'b Line<'a>, &'b [Line<'a>])> {
    let mut out = Vec::new();
    let mut i = 1;
    while i < body.len() {
        if body[i].level != 1 {
            i += 1;
            continue;
        }
        let mut next = i + 1;
        while next < body.len() && body[next].level > 1 {
            next += 1;
        }
        out.push((&body[i], &body[i + 1..next]));
        i = next;
    }
    out
}

/// Reads the pieces under a `NAME`. `_MARNM` is the MyHeritage extension for a
/// surname taken at marriage — the same one `gedc build` emits.
fn read_name(
    head: &Line,
    name: &[Line],
    person: &mut Individual,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for line in name {
        if line.level != 2 {
            continue;
        }
        match line.tag {
            "GIVN" => person.given = line.value.map(String::from),
            "SURN" => person.surname = line.value.map(String::from),
            "_MARNM" => person.married_surname = line.value.map(String::from),
            tag if is_bookkeeping(tag) => {}
            tag => diagnostics.push(record_problem(
                head,
                &format!("NAME piece {tag} is not imported yet"),
            )),
        }
    }
}

/// Reads a `BIRT`/`DEAT`/`BURI` (or a family's `MARR`/`DIV`) event from the lines
/// under it: `DATE`, `PLAC`, `AGE` and `CAUS`, the four a card's event block can
/// hold. A `DATE` is turned into the card spelling here; one GEDCOM writes in a
/// form the card grammar has no room for is named rather than dropped. Returns
/// None for an event carrying nothing a card can hold, which the caller skips.
fn read_event(
    head: &Line,
    event: &str,
    children: &[Line],
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<RawEvent> {
    let mut raw = RawEvent::default();
    for line in children {
        if line.level != 2 {
            continue;
        }
        match line.tag {
            "DATE" => {
                raw.date = line
                    .value
                    .and_then(|value| card_date(value, head, event, diagnostics))
            }
            "PLAC" => raw.place = line.value.map(String::from),
            "AGE" => raw.age = line.value.map(String::from),
            "CAUS" => raw.cause = line.value.map(String::from),
            tag if is_bookkeeping(tag) => {}
            tag => diagnostics.push(record_problem(
                head,
                &format!("{event} detail {tag} is not imported yet"),
            )),
        }
    }
    (!raw.is_empty()).then_some(raw)
}

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// The imprecision markers GEDCOM spells with a word, and the card's one-char
/// prefix for each — the inverse of the table `parse_date` reads.
const MARKERS: [(&str, &str); 3] = [("ABT ", "~"), ("BEF ", "<"), ("AFT ", ">")];

/// Turns a GEDCOM date into the card spelling — the inverse of `parse_date`.
/// `25 JUL 1995` becomes `1995-07-25`, `JUL 1995` becomes `1995-07`, `1995`
/// stays `1995`, and an `ABT`/`BEF`/`AFT` marker becomes a leading `~`/`<`/`>`.
/// A form the card grammar cannot express (a range, a `CAL`/`EST` qualifier) is
/// named rather than dropped, and the field is left unset.
fn card_date(
    gedcom: &str,
    head: &Line,
    event: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    match parse_gedcom_date(gedcom) {
        Some(date) => Some(date),
        None => {
            diagnostics.push(record_problem(
                head,
                &format!("{event} date {gedcom:?} is not a form a card can hold"),
            ));
            None
        }
    }
}

/// The parsing half of `card_date`, split out so the diagnostic stays in one
/// place. None when the text is not one of the three shapes a card date takes.
fn parse_gedcom_date(gedcom: &str) -> Option<String> {
    let (marker, rest) = MARKERS
        .iter()
        .find_map(|(in_gedcom, on_card)| Some((*on_card, gedcom.strip_prefix(in_gedcom)?)))
        .unwrap_or(("", gedcom));
    let parts: Vec<&str> = rest.split_whitespace().collect();
    let (day, month, year) = match parts.as_slice() {
        [year] => (None, None, *year),
        [month, year] => (None, Some(*month), *year),
        [day, month, year] => (Some(*day), Some(*month), *year),
        _ => return None,
    };
    if year.len() != 4 || !year.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let Some(month) = month else {
        return Some(format!("{marker}{year}"));
    };
    let month = MONTHS.iter().position(|name| *name == month)? + 1;
    let Some(day) = day else {
        return Some(format!("{marker}{year}-{month:02}"));
    };
    let day: u8 = day.parse().ok()?;
    if !(1..=31).contains(&day) {
        return None;
    }
    Some(format!("{marker}{year}-{month:02}-{day:02}"))
}

/// Whether a tag is a tool's bookkeeping rather than a person's fact, and so may
/// be dropped without a diagnostic — the same leniency the header gets, extended
/// to the record. A `_`-prefixed tag is a vendor extension (GEDCOM reserves the
/// underscore for them); the one this schema has adopted, `_MARNM`, is matched
/// before this is asked, so only the unrecognised ones — MyHeritage's `_UID`,
/// `_UPD` and the like — reach here. `RIN` is a standard tag but a database key
/// all the same. A standard genealogical tag with no field yet (`OCCU`, `RESI`)
/// is not bookkeeping: it is a fact, and is named rather than dropped.
fn is_bookkeeping(tag: &str) -> bool {
    tag == "RIN" || tag.starts_with('_')
}

/// Gives each person a deterministic id: a latin slug of the surname and the
/// first given name, per the README's id rules, keyed by their xref so the FAM
/// records resolve. Namesakes — names that transliterate alike — take a birth-year
/// suffix (`ivanov-pyotr-1947`), and where even that collides, or no birth year is known,
/// a numeric one in the order the file lists them. The same file always assigns
/// the same ids.
fn assign_ids<'a>(
    individuals: &[Individual<'a>],
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<&'a str, String> {
    let mut taken: HashSet<String> = HashSet::new();
    let mut ids = HashMap::new();
    for person in individuals {
        // A person missing a name already carries its own diagnostic; skip the
        // id it cannot have.
        let (Some(given), Some(surname)) = (&person.given, &person.surname) else {
            continue;
        };
        let Some(base) = derive_id(given, surname) else {
            diagnostics.push(Diagnostic {
                card: Some(person.label.clone()),
                field: None,
                reason: "name has no latin letters to build an id from".to_string(),
            });
            continue;
        };
        let mut id = base.clone();
        // The birth year is the README's disambiguator for namesakes; it only
        // earns its place once the base is taken, so the first of a name keeps
        // the bare slug and the rest gain the year.
        if taken.contains(&id)
            && let Some(year) = birth_year(person)
        {
            id = format!("{base}-{year}");
        }
        // Same name, same year, or no year at all: a numeric suffix is the last
        // resort, and it is enough for the id to be unique and deterministic.
        if taken.contains(&id) {
            let stem = id.clone();
            let mut suffix = 2;
            loop {
                id = format!("{stem}-{suffix}");
                if !taken.contains(&id) {
                    break;
                }
                suffix += 1;
            }
        }
        taken.insert(id.clone());
        ids.insert(person.xref, id);
    }
    ids
}

/// The four-digit birth year, when the card's birth date carries one. It is what
/// a namesake's id is suffixed with. The marker and any month or day are dropped:
/// `~1910` and `1910-06-02` both give `1910`.
fn birth_year(person: &Individual) -> Option<String> {
    let date = person.birth.as_ref()?.date.as_ref()?;
    let year: String = date
        .trim_start_matches(['~', '<', '>'])
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (year.len() == 4).then_some(year)
}

/// The parents each person's card names, keyed by xref: a family's `HUSB` is
/// every listed child's `father`, its `WIFE` their `mother`. A child a second
/// family also claims, or a link to a person no `INDI` declares, is named rather
/// than resolved to the wrong card.
fn link_parents<'a>(
    families: &[Family<'a>],
    ids: &HashMap<&'a str, String>,
    known: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<&'a str, (Option<String>, Option<String>)> {
    let mut parents: HashMap<&str, (Option<String>, Option<String>)> = HashMap::new();
    for family in families {
        let father = resolve(family.husband, family, "HUSB", ids, known, diagnostics);
        let mother = resolve(family.wife, family, "WIFE", ids, known, diagnostics);
        for &child in &family.children {
            if !known.contains(child) {
                diagnostics.push(family_problem(
                    family,
                    &format!("CHIL {child} names no INDI record"),
                ));
                continue;
            }
            if parents
                .insert(child, (father.clone(), mother.clone()))
                .is_some()
            {
                diagnostics.push(Diagnostic {
                    card: Some(child.to_string()),
                    field: None,
                    reason: "listed as a child in more than one family".to_string(),
                });
            }
        }
    }
    parents
}

/// The marriage each spouse's card carries, keyed by the xref of the spouse that
/// hosts it. A `FAM` becomes a marriage when it holds a `MARR`/`DIV`, or when a
/// couple with no children has only their pairing to record. It is hosted on one
/// spouse — the husband if free, else the wife — so no card carries two, the way
/// a hand-authored tree never would; a couple where both already host is named
/// rather than declared twice.
fn assign_marriages<'a>(
    families: &[Family<'a>],
    ids: &HashMap<&'a str, String>,
    known: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<&'a str, CardMarriage> {
    let mut hosted: HashMap<&str, CardMarriage> = HashMap::new();
    for family in families {
        let childless_pair =
            family.husband.is_some() && family.wife.is_some() && family.children.is_empty();
        if family.marriage.is_none() && family.divorce.is_none() && !childless_pair {
            continue;
        }
        let (Some(husband), Some(wife)) = (family.husband, family.wife) else {
            diagnostics.push(family_problem(
                family,
                "a marriage needs both a husband and a wife to import",
            ));
            continue;
        };
        // Resolve the pair; a spouse with no id already carries its own missing
        // diagnostic, and the run fails, so there is nothing to add here.
        let (Some(_), Some(_)) = (ids.get(husband), ids.get(wife)) else {
            resolve(Some(husband), family, "HUSB", ids, known, diagnostics);
            resolve(Some(wife), family, "WIFE", ids, known, diagnostics);
            continue;
        };
        let (host, spouse) = if !hosted.contains_key(husband) {
            (husband, wife)
        } else if !hosted.contains_key(wife) {
            (wife, husband)
        } else {
            diagnostics.push(family_problem(
                family,
                "both spouses already carry a marriage, so this one has nowhere to go",
            ));
            continue;
        };
        hosted.insert(
            host,
            CardMarriage {
                spouse: ids[spouse].clone(),
                event: family.marriage.clone().unwrap_or_default(),
                divorce: family.divorce.clone(),
            },
        );
    }
    hosted
}

/// An xref as the card id it maps to, or None when it names no importable
/// person. A link to a person with no `INDI` record is named; one to a person
/// whose card failed to build carries its own diagnostic already, so it is left
/// to resolve to None in silence.
fn resolve(
    xref: Option<&str>,
    family: &Family,
    role: &str,
    ids: &HashMap<&str, String>,
    known: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let xref = xref?;
    if !known.contains(xref) {
        diagnostics.push(family_problem(
            family,
            &format!("{role} {xref} names no INDI record"),
        ));
        return None;
    }
    ids.get(xref).cloned()
}

/// Turns the recovered people into cards, in the field order the README writes
/// them. Only a person with an id — a name that yielded one — becomes a card;
/// the rest already carry the diagnostics that will stop the run.
fn build_cards(
    individuals: &[Individual],
    ids: &HashMap<&str, String>,
    parents: &HashMap<&str, (Option<String>, Option<String>)>,
    marriages: &HashMap<&str, CardMarriage>,
) -> Vec<Card> {
    let mut cards = Vec::new();
    for person in individuals {
        let Some(id) = ids.get(person.xref) else {
            continue;
        };
        let (father, mother) = parents.get(person.xref).cloned().unwrap_or((None, None));
        cards.push(Card {
            id: id.clone(),
            yaml: card_yaml(person, father, mother, marriages.get(person.xref)),
        });
    }
    cards
}

/// The card body for a recovered person. No `patronymic`: `GIVN` fuses the given
/// name and any patronymic into one string that cannot be split back apart, so it
/// all stays in `name`.
fn card_yaml(
    person: &Individual,
    father: Option<String>,
    mother: Option<String>,
    marriage: Option<&CardMarriage>,
) -> String {
    let mut yaml = format!(
        "name: {}\nsurname: {}\n",
        person.given.as_deref().unwrap_or(""),
        person.surname.as_deref().unwrap_or(""),
    );
    if let Some(married) = &person.married_surname {
        yaml.push_str(&format!("married_surname: {married}\n"));
    }
    yaml.push_str(&format!("sex: {}\n", person.sex.as_deref().unwrap_or("")));
    push_event(&mut yaml, "birth", person.birth.as_ref());
    push_event(&mut yaml, "death", person.death.as_ref());
    push_event(&mut yaml, "burial", person.burial.as_ref());
    if let Some(father) = &father {
        yaml.push_str(&format!("father: {father}\n"));
    }
    if let Some(mother) = &mother {
        yaml.push_str(&format!("mother: {mother}\n"));
    }
    if let Some(marriage) = marriage {
        push_marriage(&mut yaml, marriage);
    }
    yaml
}

/// Writes an event block if it carries anything, in the field order the compiler
/// reads: date, place, age, cause. Two levels of indentation, as YAML nests them.
fn push_event(yaml: &mut String, key: &str, event: Option<&RawEvent>) {
    let Some(event) = event else {
        return;
    };
    yaml.push_str(&format!("{key}:\n"));
    push_event_body(yaml, event, "  ");
}

/// The date/place/age/cause lines of an event, at the given indent. Shared by the
/// top-level events and the divorce nested in a marriage.
fn push_event_body(yaml: &mut String, event: &RawEvent, indent: &str) {
    if let Some(date) = &event.date {
        yaml.push_str(&format!("{indent}date: {}\n", yaml_date(date)));
    }
    if let Some(place) = &event.place {
        yaml.push_str(&format!("{indent}place: {place}\n"));
    }
    if let Some(age) = &event.age {
        yaml.push_str(&format!("{indent}age: {age}\n"));
    }
    if let Some(cause) = &event.cause {
        yaml.push_str(&format!("{indent}cause: {cause}\n"));
    }
}

/// Writes the marriage block: the spouse, then the date and place it carried, and
/// the divorce nested inside it — a bare `divorce:` when the marriage ended with
/// no date or place to record, the way the compiler reads `1 DIV Y`.
fn push_marriage(yaml: &mut String, marriage: &CardMarriage) {
    yaml.push_str(&format!("marriage:\n  spouse: {}\n", marriage.spouse));
    push_event_body(yaml, &marriage.event, "  ");
    if let Some(divorce) = &marriage.divorce {
        if divorce.is_empty() {
            yaml.push_str("  divorce:\n");
        } else {
            yaml.push_str("  divorce:\n");
            push_event_body(yaml, divorce, "    ");
        }
    }
}

/// A card date as YAML must spell it. An after-date starts with `>`, which YAML
/// reads as a block scalar, so it is quoted; every other form is a plain scalar
/// written as it stands — a bare year as an integer, the rest as strings.
fn yaml_date(date: &str) -> String {
    if date.starts_with('>') {
        format!("'{date}'")
    } else {
        date.to_string()
    }
}

/// A latin id from the surname and the first given name: `Иванов` and
/// `Пётр Сергеевич` become `ivanov-pyotr`. The surname leads so a `people/`
/// directory sorts into families rather than scattering by given name. The
/// patronymic is left out of the id the way the README's ids leave it out, even
/// though it stays in the `name` field. None when nothing survives
/// transliteration to build a slug from.
fn derive_id(given: &str, surname: &str) -> Option<String> {
    let first = given.split_whitespace().next().unwrap_or("");
    let text = transliterate(&format!("{surname} {first}"));
    let slug = text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    (!slug.is_empty()).then_some(slug)
}

/// Cyrillic to latin, lowercasing as it goes. The scheme is written down in
/// docs/adr/0003-import-transliteration-and-id-derivation.md; anything already
/// ascii passes through lowercased, and anything else becomes a separator so the
/// slug breaks on it rather than carrying it.
fn transliterate(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        let lower = c.to_ascii_lowercase();
        match cyrillic(c) {
            Some(latin) => out.push_str(latin),
            None if lower.is_ascii_alphanumeric() => out.push(lower),
            None => out.push(' '),
        }
    }
    out
}

/// One Russian Cyrillic letter as latin, or None if the char is not one. The
/// soft and hard signs map to nothing, dropping out of the slug.
fn cyrillic(c: char) -> Option<&'static str> {
    let mapping = match c.to_lowercase().next().unwrap_or(c) {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "g",
        'д' => "d",
        'е' => "e",
        'ё' => "yo",
        'ж' => "zh",
        'з' => "z",
        'и' => "i",
        'й' => "y",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'у' => "u",
        'ф' => "f",
        'х' => "kh",
        'ц' => "ts",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "shch",
        'ъ' => "",
        'ы' => "y",
        'ь' => "",
        'э' => "e",
        'ю' => "yu",
        'я' => "ya",
        _ => return None,
    };
    Some(mapping)
}

/// How a record is named in a diagnostic: its `@xref@`, or the tag when a record
/// somehow has none. The `_ref` form borrows it, for keying by xref; the owned
/// one is for a diagnostic that outlives the lines.
fn record_label_ref<'a>(head: &Line<'a>) -> &'a str {
    head.xref.unwrap_or(head.tag)
}

fn record_label(head: &Line) -> String {
    record_label_ref(head).to_string()
}

/// A diagnostic attributed to a whole record, naming it so the tag it complains
/// about can be found.
fn record_problem(head: &Line, reason: &str) -> Diagnostic {
    Diagnostic {
        card: Some(record_label(head)),
        field: None,
        reason: reason.to_string(),
    }
}

/// A diagnostic naming a family, for a link one of its records got wrong.
fn family_problem(family: &Family, reason: &str) -> Diagnostic {
    Diagnostic {
        card: Some(family.xref.to_string()),
        field: None,
        reason: reason.to_string(),
    }
}

/// A diagnostic about a header fact tree.yaml needs but the file did not carry.
fn header_problem(field: &str, reason: &str) -> Diagnostic {
    Diagnostic {
        card: None,
        field: Some(field.to_string()),
        reason: reason.to_string(),
    }
}
