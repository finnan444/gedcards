//! The inverse of `compile`: a GEDCOM 5.5.1 file in, `tree.yaml` and one card
//! per `INDI` out. It is deliberately partial — only the names and sex a card
//! has a field for are read. Everything else a `.ged` may carry (birth and death
//! dates, the `FAM` records that hold relationships) has no card field yet, and
//! rather than drop it silently import stops and names it, so the next
//! `gedc build` cannot quietly lose what this run could not represent. Those
//! fields arrive as the format is parsed further, alongside issues #1 and #4.
//!
//! The header is the one place import is lenient: `SOUR`, `DATE`, `FILE` and the
//! rest are metadata `gedc build` regenerates from scratch, so skipping them
//! loses nothing about a person. Only `LANG` and the submitter's name are read
//! back, since `tree.yaml` needs them.

use crate::{Card, Diagnostic};

/// One parsed GEDCOM line: its level, an optional cross-reference id (the
/// `@I1@` on a record's opening line), the tag, and the value after it.
struct Line<'a> {
    level: u8,
    xref: Option<&'a str>,
    tag: &'a str,
    value: Option<&'a str>,
}

/// A person as import recovers them, before ids are assigned. `given` is the
/// whole `GIVN` value: a patronymic fused into it on the way out cannot be split
/// back off — see docs/adr/0002-patronymic-joins-the-given-name.md — so it stays
/// part of the name rather than being guessed at.
struct Imported {
    label: String,
    given: Option<String>,
    surname: Option<String>,
    married_surname: Option<String>,
    sex: Option<String>,
}

/// The single import seam: GEDCOM text in, `tree.yaml` text plus one card per
/// `INDI` out, or the diagnostics that say why nothing could be written. Like
/// `compile`, every problem is reported together and, when there is any, nothing
/// is produced.
pub fn import(ged: &str) -> Result<(String, Vec<Card>), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let lines = parse_lines(ged, &mut diagnostics);

    let mut language: Option<String> = None;
    let mut submitter: Option<String> = None;
    let mut imported: Vec<Imported> = Vec::new();

    // Walk the level-0 records. Each record is the opening line plus every line
    // under it until the next level-0 line.
    let mut record = 0;
    while record < lines.len() {
        let mut next = record + 1;
        while next < lines.len() && lines[next].level != 0 {
            next += 1;
        }
        let body = &lines[record..next];
        let head = &body[0];
        match head.tag {
            "HEAD" => read_header(body, &mut language),
            "SUBM" => read_submitter(body, &mut submitter),
            "INDI" => {
                if let Some(person) = read_individual(body, &mut diagnostics) {
                    imported.push(person);
                }
            }
            "TRLR" => {}
            "FAM" => diagnostics.push(record_problem(
                head,
                "relationships are not imported yet — they arrive with FAM records (#4)",
            )),
            _ => diagnostics.push(record_problem(
                head,
                &format!("unexpected {} record", head.tag),
            )),
        }
        record = next;
    }

    // tree.yaml needs both, and the compiler will refuse a card set without
    // them: better to say so here, naming the missing header fact, than to write
    // a tree.yaml that will not build.
    if language.is_none() {
        diagnostics.push(Diagnostic {
            card: None,
            field: Some("language".to_string()),
            reason: "no LANG in the header to import".to_string(),
        });
    }
    if submitter.is_none() {
        diagnostics.push(Diagnostic {
            card: None,
            field: Some("submitter".to_string()),
            reason: "no SUBM record with a NAME to import".to_string(),
        });
    }

    let people = assign_ids(imported, &mut diagnostics);

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let language = language.expect("no diagnostics means the header had a LANG");
    let submitter = submitter.expect("no diagnostics means a SUBM record had a NAME");
    let tree_yaml = format!("submitter: {submitter}\nlanguage: {language}\n");
    Ok((tree_yaml, people))
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
        Some((tag, value)) => (tag, Some(value)),
        None => (rest, None),
    };
    Some(Line {
        level,
        xref,
        tag,
        value,
    })
}

/// Reads `LANG` out of the header and ignores the rest: the header is metadata
/// `gedc build` writes fresh, so nothing else in it is a person's fact to lose.
fn read_header(body: &[Line], language: &mut Option<String>) {
    for line in &body[1..] {
        if line.level == 1 && line.tag == "LANG" {
            *language = line.value.map(String::from);
        }
    }
}

/// Reads the submitter's `NAME`. Like the header, the record is lenient: any
/// other detail a tool wrote into it is metadata, not a card's fact.
fn read_submitter(body: &[Line], submitter: &mut Option<String>) {
    for line in &body[1..] {
        if line.level == 1 && line.tag == "NAME" {
            *submitter = line.value.map(String::from);
        }
    }
}

/// Reads one `INDI`. Only `NAME` (with its `GIVN`, `SURN` and `_MARNM`) and
/// `SEX` map to a card field; any other tag is a fact this card cannot hold yet,
/// so it is named rather than dropped.
fn read_individual(body: &[Line], diagnostics: &mut Vec<Diagnostic>) -> Option<Imported> {
    let head = &body[0];
    let label = record_label(head);
    let mut person = Imported {
        label: label.clone(),
        given: None,
        surname: None,
        married_surname: None,
        sex: None,
    };

    let mut i = 1;
    while i < body.len() {
        let line = &body[i];
        if line.level != 1 {
            i += 1;
            continue;
        }
        // The children of this level-1 line: everything under it until the next
        // level-1 line or the end of the record.
        let mut next = i + 1;
        while next < body.len() && body[next].level > 1 {
            next += 1;
        }
        match line.tag {
            // The NAME line's own value is ignored: GIVN and SURN carry the same
            // pieces split out, which is what the card fields want.
            "NAME" => read_name(head, &body[i..next], &mut person, diagnostics),
            "SEX" => person.sex = line.value.map(String::from),
            tag => diagnostics.push(record_problem(head, &format!("{tag} is not imported yet"))),
        }
        i = next;
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
    Some(person)
}

/// Reads the pieces under a `NAME`. `_MARNM` is the MyHeritage extension for a
/// surname taken at marriage — the same one `gedc build` emits.
fn read_name(head: &Line, name: &[Line], person: &mut Imported, diagnostics: &mut Vec<Diagnostic>) {
    for line in &name[1..] {
        if line.level != 2 {
            continue;
        }
        match line.tag {
            "GIVN" => person.given = line.value.map(String::from),
            "SURN" => person.surname = line.value.map(String::from),
            "_MARNM" => person.married_surname = line.value.map(String::from),
            tag => diagnostics.push(record_problem(
                head,
                &format!("NAME piece {tag} is not imported yet"),
            )),
        }
    }
}

/// Turns the recovered people into cards, giving each a deterministic id: a
/// latin slug of the first given name and the surname, per the README's id
/// rules. Namesakes with no birth year to tell them apart — dates arrive with
/// issue #1 — get a numeric suffix in the order the file lists them, so the same
/// file always assigns the same ids.
fn assign_ids(imported: Vec<Imported>, diagnostics: &mut Vec<Diagnostic>) -> Vec<Card> {
    let mut taken: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut cards = Vec::new();
    for person in imported {
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
        let mut suffix = 1;
        while !taken.insert(id.clone()) {
            suffix += 1;
            id = format!("{base}-{suffix}");
        }
        cards.push(Card {
            id,
            yaml: card_yaml(&person),
        });
    }
    cards
}

/// The card body for a recovered person, in the field order the README writes
/// them. No `patronymic`: `GIVN` fuses the given name and any patronymic into
/// one string that cannot be split back apart, so it all stays in `name`.
fn card_yaml(person: &Imported) -> String {
    let mut yaml = format!(
        "name: {}\nsurname: {}\n",
        person.given.as_deref().unwrap_or(""),
        person.surname.as_deref().unwrap_or(""),
    );
    if let Some(married) = &person.married_surname {
        yaml.push_str(&format!("married_surname: {married}\n"));
    }
    yaml.push_str(&format!("sex: {}\n", person.sex.as_deref().unwrap_or("")));
    yaml
}

/// A latin id from the first given name and the surname: `Пётр Сергеевич` and
/// `Иванов` become `pyotr-ivanov`. The patronymic is left out of the id the way
/// the README's ids leave it out, even though it stays in the `name` field.
/// None when nothing survives transliteration to build a slug from.
fn derive_id(given: &str, surname: &str) -> Option<String> {
    let first = given.split_whitespace().next().unwrap_or("");
    let text = transliterate(&format!("{first} {surname}"));
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
/// somehow has none. It is what lets a reader find the record in the file.
fn record_label(head: &Line) -> String {
    head.xref.unwrap_or(head.tag).to_string()
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
