use std::fmt;

/// One person card: `id` is the card file name without extension,
/// `yaml` is the raw file content.
pub struct Card {
    pub id: String,
    pub yaml: String,
}

/// A single compilation problem: which card (None for tree.yaml),
/// which field (None for file-level problems), and why.
#[derive(Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub card: Option<String>,
    pub field: Option<String>,
    pub reason: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let source = self.card.as_deref().unwrap_or("tree.yaml");
        match &self.field {
            Some(field) => write!(f, "{source}: {field}: {}", self.reason),
            None => write!(f, "{source}: {}", self.reason),
        }
    }
}

struct Config {
    submitter: String,
    language: String,
}

/// A life event. GEDCOM allows either part on its own, and a card with only
/// a place is ordinary: the village is remembered when the year is not.
struct Event {
    /// Already in GEDCOM spelling — see `parse_date`.
    date: Option<String>,
    place: Option<String>,
}

struct Person {
    id: String,
    name: String,
    patronymic: Option<String>,
    surname: String,
    /// Surname taken at marriage. The primary surname stays the one at birth,
    /// so maiden names keep displaying correctly after import.
    married_surname: Option<String>,
    sex: String,
    birth: Option<Event>,
    death: Option<Event>,
}

impl Person {
    /// The given name as GEDCOM spells it: the patronymic is part of it,
    /// even though the card keeps the two apart. Neither GEDCOM version has
    /// a patronymic piece — see docs/adr/0002-patronymic-joins-the-given-name.md.
    fn given(&self) -> String {
        match &self.patronymic {
            Some(patronymic) => format!("{} {patronymic}", self.name),
            None => self.name.clone(),
        }
    }
}

/// The single seam: cards + config in, GEDCOM 5.5.1 text or diagnostics out.
/// Output is byte-for-byte deterministic for the same input.
pub fn compile(config_yaml: &str, cards: &[Card]) -> Result<String, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    let config = parse_config(config_yaml, &mut diagnostics);
    let mut seen_ids = std::collections::HashSet::new();
    let mut people = Vec::new();
    for card in cards {
        // A duplicate id still gets its body parsed, so every problem in the
        // card surfaces in the same run instead of one per fix-and-rerun.
        let is_duplicate = !seen_ids.insert(card.id.as_str());
        if is_duplicate {
            diagnostics.push(Diagnostic {
                card: Some(card.id.clone()),
                field: None,
                reason: "duplicate id".to_string(),
            });
        }
        if let Some(person) = parse_card(card, &mut diagnostics)
            && !is_duplicate
        {
            people.push(person);
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let config = config.expect("no diagnostics means config parsed");
    people.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(emit(&config, &people))
}

fn parse_mapping(
    yaml: &str,
    card: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<serde_norway::Mapping> {
    match serde_norway::from_str::<serde_norway::Value>(yaml) {
        Ok(serde_norway::Value::Mapping(mapping)) => Some(mapping),
        Ok(_) => {
            diagnostics.push(Diagnostic {
                card: card.map(String::from),
                field: None,
                reason: "expected a YAML mapping of key: value pairs".to_string(),
            });
            None
        }
        Err(err) => {
            diagnostics.push(Diagnostic {
                card: card.map(String::from),
                field: None,
                reason: format!("invalid YAML: {err}"),
            });
            None
        }
    }
}

/// Checks a value that a field was set to. A non-string, a blank string or
/// one padded with whitespace is reported and yields None; anything else is
/// the value verbatim. Padding is refused rather than trimmed away, so the
/// card and the GEDCOM line always read the same.
fn check_value(
    value: serde_norway::Value,
    field: &str,
    card: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let reason = match value {
        serde_norway::Value::String(value) => {
            if value.trim().is_empty() {
                "must not be empty"
            } else if value.trim() != value {
                "must not have leading or trailing whitespace"
            } else {
                return Some(value);
            }
        }
        _ => "expected a string",
    };
    diagnostics.push(Diagnostic {
        card: card.map(String::from),
        field: Some(field.to_string()),
        reason: reason.to_string(),
    });
    None
}

/// Everything below names a field by the path a diagnostic prints it as —
/// `place` at the top level, `birth.place` inside an event block — while the
/// mapping holding it is keyed by the last segment alone. `report_unknown_keys`
/// takes the block name instead, because it finds its own leaf.
fn key_of(field: &str) -> &str {
    field
        .rsplit('.')
        .next()
        .expect("rsplit yields at least one segment")
}

/// Pulls a required string field out of a parsed mapping, reporting
/// a diagnostic (attributed to `card`, None for config) when absent
/// or rejected by `check_value`. A key written with no value at all
/// (`name:`, `name: null`, `name: ~`) is as absent as no key.
fn take_string(
    mapping: &mut serde_norway::Mapping,
    field: &str,
    card: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    match mapping.remove(key_of(field)) {
        Some(serde_norway::Value::Null) | None => {
            diagnostics.push(Diagnostic {
                card: card.map(String::from),
                field: Some(field.to_string()),
                reason: "required field is missing".to_string(),
            });
            None
        }
        Some(value) => check_value(value, field, card, diagnostics),
    }
}

/// Like `take_string`, but an absent field is not a problem. A present one
/// still goes through `check_value`, so a typo'd value is still caught.
/// A valueless key is a mistake rather than a way to say "absent": leaving
/// the key out already says that, and one way is enough.
fn take_optional_string(
    mapping: &mut serde_norway::Mapping,
    field: &str,
    card: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    match mapping.remove(key_of(field))? {
        serde_norway::Value::Null => {
            diagnostics.push(Diagnostic {
                card: card.map(String::from),
                field: Some(field.to_string()),
                reason: "remove the key instead of leaving it empty".to_string(),
            });
            None
        }
        value => check_value(value, field, card, diagnostics),
    }
}

/// Every key left in the mapping after the known ones were taken out
/// is unknown; reported in sorted order for determinism. `block` names the
/// event block the keys sit in, so the reader is told which one to look at.
fn report_unknown_keys(
    mapping: serde_norway::Mapping,
    block: Option<&str>,
    card: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut keys: Vec<String> = mapping
        .keys()
        .map(|key| match key {
            serde_norway::Value::String(key) => key.clone(),
            other => serde_norway::to_string(other)
                .unwrap_or_default()
                .trim_end()
                .to_string(),
        })
        .collect();
    keys.sort();
    for key in keys {
        diagnostics.push(Diagnostic {
            card: card.map(String::from),
            field: Some(match block {
                Some(block) => format!("{block}.{key}"),
                None => key,
            }),
            reason: "unknown key".to_string(),
        });
    }
}

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// The imprecision markers a card date may carry, and how GEDCOM spells them.
/// The GEDCOM spelling carries the space that separates it from the date.
const MARKERS: [(char, &str); 3] = [('~', "ABT "), ('<', "BEF "), ('>', "AFT ")];

const DATE_SYNTAX: &str =
    "expected a date like 1995-07-25, 1995-07 or 1995, optionally prefixed with ~, < or >";

/// Exactly two ASCII digits within `1..=max`. `str::parse` alone would not do:
/// it accepts a leading `+` and a single digit.
fn two_digits(text: &str, max: u8) -> Option<u8> {
    if text.len() != 2 || !text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let value: u8 = text.parse().ok()?;
    (1..=max).contains(&value).then_some(value)
}

/// Turns a card date into the GEDCOM one. The card grammar is an ISO subset —
/// `1995-07-25`, `1995-07`, `1995` — each optionally prefixed with `~`, `<`
/// or `>`; anything else yields None. Precision missing from the card is never
/// invented: a year-only date stays a year.
///
/// A day is only range-checked, not measured against its month: dates this old
/// are as often Julian as Gregorian, and rejecting `1918-02-30` would mean
/// picking a calendar the card never named.
fn parse_date(text: &str) -> Option<String> {
    let (marker, rest) = MARKERS
        .iter()
        .find_map(|(on_card, in_gedcom)| Some((*in_gedcom, text.strip_prefix(*on_card)?)))
        .unwrap_or(("", text));

    let mut parts = rest.split('-');
    let year = parts.next().expect("split yields at least one segment");
    let month = parts.next();
    let day = parts.next();
    // Four digits, and no year zero — neither calendar GEDCOM knows has one.
    if parts.next().is_some()
        || year.len() != 4
        || !year.chars().all(|c| c.is_ascii_digit())
        || year == "0000"
    {
        return None;
    }

    let month = match month {
        Some(month) => MONTHS[usize::from(two_digits(month, 12)?) - 1],
        None => return Some(format!("{marker}{year}")),
    };
    match day {
        Some(day) => Some(format!("{marker}{} {month} {year}", two_digits(day, 31)?)),
        None => Some(format!("{marker}{month} {year}")),
    }
}

/// Like `take_optional_string`, but the value must also be a date, and it
/// comes back in GEDCOM spelling.
fn take_date(
    mapping: &mut serde_norway::Mapping,
    field: &str,
    card: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    // A bare year is a YAML integer where every other form is a string, so it
    // is read as one rather than made to carry quotes the author cannot guess.
    if let Some(value) = mapping.get_mut(key_of(field))
        && let Some(year) = value.as_i64()
    {
        *value = serde_norway::Value::String(year.to_string());
    }
    let text = take_optional_string(mapping, field, Some(card), diagnostics)?;
    let date = parse_date(&text);
    if date.is_none() {
        diagnostics.push(Diagnostic {
            card: Some(card.to_string()),
            field: Some(field.to_string()),
            reason: DATE_SYNTAX.to_string(),
        });
    }
    date
}

/// Reads a `birth`/`death` block. Either part may be left out, but a block
/// carrying neither says nothing and is a mistake rather than an empty event.
fn take_event(
    mapping: &mut serde_norway::Mapping,
    field: &str,
    card: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Event> {
    let reason = match mapping.remove(field)? {
        serde_norway::Value::Mapping(mut block) if !block.is_empty() => {
            let date = take_date(&mut block, &format!("{field}.date"), card, diagnostics);
            let place = take_optional_string(
                &mut block,
                &format!("{field}.place"),
                Some(card),
                diagnostics,
            );
            report_unknown_keys(block, Some(field), Some(card), diagnostics);
            return Some(Event { date, place });
        }
        serde_norway::Value::Mapping(_) => "needs a date or a place",
        serde_norway::Value::Null => "remove the key instead of leaving it empty",
        _ => "expected a block with a date and/or a place",
    };
    diagnostics.push(Diagnostic {
        card: Some(card.to_string()),
        field: Some(field.to_string()),
        reason: reason.to_string(),
    });
    None
}

fn parse_config(config_yaml: &str, diagnostics: &mut Vec<Diagnostic>) -> Option<Config> {
    let mut mapping = parse_mapping(config_yaml, None, diagnostics)?;
    let submitter = take_string(&mut mapping, "submitter", None, diagnostics);
    let language = take_string(&mut mapping, "language", None, diagnostics);
    report_unknown_keys(mapping, None, None, diagnostics);
    Some(Config {
        submitter: submitter?,
        language: language?,
    })
}

/// Ids are slugs per the project glossary: latin translit like
/// `ivan-ivanov` or `pyotr-ivanov-1947`. Cyrillic is not allowed.
fn is_slug(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && !id.contains("--")
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn parse_card(card: &Card, diagnostics: &mut Vec<Diagnostic>) -> Option<Person> {
    // Reported first, but the body is parsed anyway so a bad id doesn't hide
    // the rest of the card's problems.
    let id_is_slug = is_slug(&card.id);
    if !id_is_slug {
        diagnostics.push(Diagnostic {
            card: Some(card.id.clone()),
            field: None,
            reason: "id must be a slug of lowercase latin letters, digits and hyphens".to_string(),
        });
    }
    let mut mapping = parse_mapping(&card.yaml, Some(&card.id), diagnostics)?;
    let name = take_string(&mut mapping, "name", Some(&card.id), diagnostics);
    let patronymic = take_optional_string(&mut mapping, "patronymic", Some(&card.id), diagnostics);
    let surname = take_string(&mut mapping, "surname", Some(&card.id), diagnostics);
    let married_surname =
        take_optional_string(&mut mapping, "married_surname", Some(&card.id), diagnostics);
    let sex = take_string(&mut mapping, "sex", Some(&card.id), diagnostics).and_then(|sex| {
        if sex == "M" || sex == "F" {
            Some(sex)
        } else {
            diagnostics.push(Diagnostic {
                card: Some(card.id.clone()),
                field: Some("sex".to_string()),
                reason: "expected M or F".to_string(),
            });
            None
        }
    });
    let birth = take_event(&mut mapping, "birth", &card.id, diagnostics);
    let death = take_event(&mut mapping, "death", &card.id, diagnostics);
    report_unknown_keys(mapping, None, Some(&card.id), diagnostics);
    if !id_is_slug {
        return None;
    }
    Some(Person {
        id: card.id.clone(),
        name: name?,
        patronymic,
        surname: surname?,
        married_surname,
        sex: sex?,
        birth,
        death,
    })
}

fn emit_event(ged: &mut String, tag: &str, event: Option<&Event>) {
    let Some(event) = event else {
        return;
    };
    ged.push_str(&format!("1 {tag}\n"));
    if let Some(date) = &event.date {
        ged.push_str(&format!("2 DATE {date}\n"));
    }
    if let Some(place) = &event.place {
        ged.push_str(&format!("2 PLAC {place}\n"));
    }
}

fn emit(config: &Config, people: &[Person]) -> String {
    let mut ged = String::new();
    ged.push_str("0 HEAD\n");
    ged.push_str("1 SOUR gedc\n");
    ged.push_str("1 SUBM @SUB1@\n");
    ged.push_str("1 GEDC\n");
    ged.push_str("2 VERS 5.5.1\n");
    ged.push_str("2 FORM LINEAGE-LINKED\n");
    ged.push_str("1 CHAR UTF-8\n");
    ged.push_str(&format!("1 LANG {}\n", config.language));
    for (index, person) in people.iter().enumerate() {
        ged.push_str(&format!("0 @I{}@ INDI\n", index + 1));
        let given = person.given();
        ged.push_str(&format!("1 NAME {given} /{}/\n", person.surname));
        ged.push_str(&format!("2 GIVN {given}\n"));
        ged.push_str(&format!("2 SURN {}\n", person.surname));
        // _MARNM is not in GEDCOM 5.5.1; it is the extension MyHeritage
        // reads and writes for a surname taken at marriage. Shape checked
        // against a MyHeritage export (2026-07-19): level 2, directly after
        // SURN, and the value is a bare surname — not a slashed full name.
        if let Some(married_surname) = &person.married_surname {
            ged.push_str(&format!("2 _MARNM {married_surname}\n"));
        }
        ged.push_str(&format!("1 SEX {}\n", person.sex));
        // Name, sex, then events: the order the 5.5.1 INDIVIDUAL_RECORD
        // grammar lists them in.
        emit_event(&mut ged, "BIRT", person.birth.as_ref());
        emit_event(&mut ged, "DEAT", person.death.as_ref());
    }
    ged.push_str(&format!("0 @SUB1@ SUBM\n1 NAME {}\n", config.submitter));
    ged.push_str("0 TRLR\n");
    ged
}
