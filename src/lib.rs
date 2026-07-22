use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

mod schema;

pub use schema::schema;

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

/// A date in the two shapes the compiler needs: the GEDCOM spelling to emit,
/// and the numbers to order children by.
#[derive(Clone)]
struct Date {
    /// Already in GEDCOM spelling — see `parse_date`.
    gedcom: String,
    /// Year, month, day, with the parts the card left out zero, so a year-only
    /// date sorts before every dated day within that year.
    order: (u16, u8, u8),
}

/// A life event. GEDCOM allows either part on its own, and a card with only
/// a place is ordinary: the village is remembered when the year is not.
#[derive(Clone)]
struct Event {
    date: Option<Date>,
    place: Option<String>,
}

/// A marriage as one card declares it. The other spouse's card says nothing:
/// declaring it twice is one fact in two places, and a compile error.
struct Marriage {
    spouse: String,
    event: Event,
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
    father: Option<String>,
    mother: Option<String>,
    marriage: Option<Marriage>,
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

    /// The birth date's numbers, or None when the card gives no birth date:
    /// what siblings are ordered by. The imprecision marker is ignored, since
    /// `~1910` and `1910` say the same thing about the order of births.
    fn birth_order(&self) -> Option<(u16, u8, u8)> {
        self.birth.as_ref()?.date.as_ref().map(|date| date.order)
    }
}

/// A family the cards imply rather than declare: a unique (father, mother)
/// pair — see docs/adr/0001-no-family-entities.md. Either side may be unknown,
/// but a family with neither is nothing at all and is never built.
struct Family {
    father: Option<String>,
    mother: Option<String>,
    marriage: Option<Event>,
    /// In birth order — see `link`.
    children: Vec<String>,
}

impl Family {
    fn new((father, mother): (Option<&str>, Option<&str>)) -> Self {
        Family {
            father: father.map(String::from),
            mother: mother.map(String::from),
            marriage: None,
            children: Vec::new(),
        }
    }
}

/// The single seam: cards + config in, GEDCOM 5.5.1 text or diagnostics out.
/// Output is byte-for-byte deterministic for the same input.
pub fn compile(config_yaml: &str, cards: &[Card]) -> Result<String, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    let config = parse_config(config_yaml, &mut diagnostics);
    // What a `father`, `mother` or `spouse` may name. Every card counts, even
    // one whose body is broken: the file exists, so the reference is not the
    // problem to report.
    let card_ids: HashSet<&str> = cards.iter().map(|card| card.id.as_str()).collect();
    let mut seen_ids = HashSet::new();
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
        if let Some(person) = parse_card(card, &card_ids, &mut diagnostics)
            && !is_duplicate
        {
            people.push(person);
        }
    }
    people.sort_by(|a, b| a.id.cmp(&b.id));
    let families = link(&people, &mut diagnostics);

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let config = config.expect("no diagnostics means config parsed");
    Ok(emit(&config, &people, &families))
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
///
/// This grammar has a twin: `schema` states it as a regex, so an editor can
/// refuse a date before a build does. Change one and change the other.
fn parse_date(text: &str) -> Option<Date> {
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
    let year_number: u16 = year.parse().expect("four ascii digits fit a u16");

    let month = match month {
        Some(month) => two_digits(month, 12)?,
        None => {
            return Some(Date {
                gedcom: format!("{marker}{year}"),
                order: (year_number, 0, 0),
            });
        }
    };
    let month_name = MONTHS[usize::from(month) - 1];
    match day {
        Some(day) => {
            let day = two_digits(day, 31)?;
            Some(Date {
                gedcom: format!("{marker}{day} {month_name} {year}"),
                order: (year_number, month, day),
            })
        }
        None => Some(Date {
            gedcom: format!("{marker}{month_name} {year}"),
            order: (year_number, month, 0),
        }),
    }
}

/// Like `take_optional_string`, but the value must also be a date, and it
/// comes back in GEDCOM spelling.
fn take_date(
    mapping: &mut serde_norway::Mapping,
    field: &str,
    card: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Date> {
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

/// Levenshtein distance in characters. Ids are ascii slugs, but what a card
/// wrote where an id belongs may be anything at all.
fn edit_distance(wanted: &str, candidate: &str) -> usize {
    let candidate: Vec<char> = candidate.chars().collect();
    let mut row: Vec<usize> = (0..=candidate.len()).collect();
    for (i, wanted_char) in wanted.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, candidate_char) in candidate.iter().enumerate() {
            let substitute = diagonal + usize::from(wanted_char != *candidate_char);
            diagonal = row[j + 1];
            row[j + 1] = substitute.min(row[j] + 1).min(diagonal + 1);
        }
    }
    row[candidate.len()]
}

/// The existing id closest to `wanted`, when one is close enough to be worth
/// naming. Ties go to the first id alphabetically, so the message does not
/// depend on the order the cards were read in.
fn closest<'a>(wanted: &str, ids: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let wanted_length = wanted.chars().count();
    ids.map(|id| (edit_distance(wanted, id), id))
        // A third of the longer id, at least one character: room for the usual
        // slips, not enough to point at somebody else entirely.
        .filter(|(distance, id)| *distance <= (wanted_length.max(id.chars().count()) / 3).max(1))
        .min()
        .map(|(_, id)| id)
}

/// Checks a value naming another card. Only that the card exists is settled
/// here — whether it may play the role is checked in `link`, once every card
/// has been parsed. A mistyped id is the common way this goes wrong, so an
/// unknown one names the id that was probably meant.
fn check_reference(
    id: String,
    field: &str,
    card: &str,
    ids: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let reason = if id == card {
        "must not be the card's own id".to_string()
    } else if ids.contains(id.as_str()) {
        return Some(id);
    } else {
        // The card's own id is no suggestion: taking it would only produce
        // the self-reference diagnostic on the next run.
        match closest(&id, ids.iter().copied().filter(|other| *other != card)) {
            Some(closest) => format!("no card with id {id}, did you mean {closest}?"),
            None => format!("no card with id {id}"),
        }
    };
    diagnostics.push(Diagnostic {
        card: Some(card.to_string()),
        field: Some(field.to_string()),
        reason,
    });
    None
}

/// Reads a `marriage` block. The spouse is required — it is what the family
/// is synthesized from — while the date and the place are not: that two people
/// married is worth recording even when neither is known.
fn take_marriage(
    mapping: &mut serde_norway::Mapping,
    card: &str,
    ids: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Marriage> {
    let reason = match mapping.remove("marriage")? {
        serde_norway::Value::Mapping(mut block) => {
            let spouse = take_string(&mut block, "marriage.spouse", Some(card), diagnostics)
                .and_then(|id| check_reference(id, "marriage.spouse", card, ids, diagnostics));
            let date = take_date(&mut block, "marriage.date", card, diagnostics);
            let place = take_optional_string(&mut block, "marriage.place", Some(card), diagnostics);
            report_unknown_keys(block, Some("marriage"), Some(card), diagnostics);
            return Some(Marriage {
                spouse: spouse?,
                event: Event { date, place },
            });
        }
        serde_norway::Value::Null => "remove the key instead of leaving it empty",
        _ => "expected a block with a spouse, and optionally a date and a place",
    };
    diagnostics.push(Diagnostic {
        card: Some(card.to_string()),
        field: Some("marriage".to_string()),
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
///
/// `schema` states this shape as a regex, for the tree that has no id to
/// offer; change one and change the other.
fn is_slug(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && !id.contains("--")
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn parse_card(
    card: &Card,
    ids: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Person> {
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
    let father = take_optional_string(&mut mapping, "father", Some(&card.id), diagnostics)
        .and_then(|id| check_reference(id, "father", &card.id, ids, diagnostics));
    let mother = take_optional_string(&mut mapping, "mother", Some(&card.id), diagnostics)
        .and_then(|id| check_reference(id, "mother", &card.id, ids, diagnostics));
    let marriage = take_marriage(&mut mapping, &card.id, ids, diagnostics);
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
        father,
        mother,
        marriage,
    })
}

/// Checks that a referenced card can play the role the field gives it. The
/// compiler has to know which spouse is the husband to emit HUSB and WIFE, so
/// a card naming a woman as `father` is a mix-up rather than a family whose
/// shape the emitter should invent.
///
/// A card that failed to parse is not in `people`; its own diagnostics already
/// say so, and this runs again once it is fixed.
fn check_role(
    people: &HashMap<&str, &Person>,
    person: &Person,
    field: &str,
    reference: &str,
    sex: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(other) = people.get(reference) else {
        return;
    };
    if other.sex != sex {
        diagnostics.push(Diagnostic {
            card: Some(person.id.clone()),
            field: Some(field.to_string()),
            reason: format!("{reference} has sex {}, expected {sex}", other.sex),
        });
    }
}

/// Checks every reference the cards make against the card it names.
fn check_relationships(
    by_id: &HashMap<&str, &Person>,
    people: &[Person],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for person in people {
        if let Some(father) = &person.father {
            check_role(by_id, person, "father", father, "M", diagnostics);
        }
        if let Some(mother) = &person.mother {
            check_role(by_id, person, "mother", mother, "F", diagnostics);
        }
        let Some(marriage) = &person.marriage else {
            continue;
        };
        let spouse_sex = if person.sex == "M" { "F" } else { "M" };
        check_role(
            by_id,
            person,
            "marriage.spouse",
            &marriage.spouse,
            spouse_sex,
            diagnostics,
        );
        // Declared on both cards, the same marriage is one fact in two places
        // and the two can disagree. Reported on the card that sorts later, so
        // which of the pair carries the diagnostic never moves.
        if let Some(spouse) = by_id.get(marriage.spouse.as_str())
            && spouse
                .marriage
                .as_ref()
                .is_some_and(|m| m.spouse == person.id)
            && spouse.id < person.id
        {
            diagnostics.push(Diagnostic {
                card: Some(person.id.clone()),
                field: Some("marriage".to_string()),
                reason: format!("also declared on {}'s card, keep only one", spouse.id),
            });
        }
    }
}

/// Turns the references on the cards into families. Nothing here is declared:
/// a family is a unique (father, mother) pair, children are the cards naming
/// that pair, and the marriage is whatever one of the two spouses wrote down —
/// see docs/adr/0001-no-family-entities.md.
///
/// `people` is sorted by id, and the families come back in (father, mother)
/// order, which is what makes the `@F1@` numbering independent of card order.
fn link(people: &[Person], diagnostics: &mut Vec<Diagnostic>) -> Vec<Family> {
    let by_id: HashMap<&str, &Person> = people
        .iter()
        .map(|person| (person.id.as_str(), person))
        .collect();
    check_relationships(&by_id, people, diagnostics);

    let mut families: BTreeMap<(Option<&str>, Option<&str>), Family> = BTreeMap::new();
    for person in people {
        if person.father.is_none() && person.mother.is_none() {
            continue;
        }
        let key = (person.father.as_deref(), person.mother.as_deref());
        families
            .entry(key)
            .or_insert_with(|| Family::new(key))
            .children
            .push(person.id.clone());
    }
    for person in people {
        let Some(marriage) = &person.marriage else {
            continue;
        };
        // Which of the two is the husband follows from the declaring card's
        // sex, so the pair keys the same family the children's cards name.
        let key = if person.sex == "M" {
            (Some(person.id.as_str()), Some(marriage.spouse.as_str()))
        } else {
            (Some(marriage.spouse.as_str()), Some(person.id.as_str()))
        };
        families
            .entry(key)
            .or_insert_with(|| Family::new(key))
            .marriage = Some(marriage.event.clone());
    }

    for family in families.values_mut() {
        // Birth order, the undated last, ties broken by id. The cards say
        // nothing else about the order siblings come in, and a viewer given
        // file order would show a meaningless one.
        family.children.sort_by(|a, b| {
            let order = |id: &str| {
                let born = by_id[id].birth_order();
                (born.is_none(), born)
            };
            order(a).cmp(&order(b)).then_with(|| a.cmp(b))
        });
    }
    families.into_values().collect()
}

fn emit_event(ged: &mut String, tag: &str, event: Option<&Event>) {
    let Some(event) = event else {
        return;
    };
    // 5.5.1: "The occurrence of an event is asserted by the presence of either
    // a DATE tag and value or a PLACe tag and value in the event structure.
    // When neither the date value nor the place value are known then a Y(es)
    // value on the parent event tag line is required to assert that the event
    // happened." Only a marriage gets here bare — a birth or death block with
    // neither part is refused on the card.
    let asserted = if event.date.is_none() && event.place.is_none() {
        " Y"
    } else {
        ""
    };
    ged.push_str(&format!("1 {tag}{asserted}\n"));
    if let Some(date) = &event.date {
        ged.push_str(&format!("2 DATE {}\n", date.gedcom));
    }
    if let Some(place) = &event.place {
        ged.push_str(&format!("2 PLAC {place}\n"));
    }
}

/// Writes the file. Every id reached here resolves: emitting only happens once
/// the cards are free of diagnostics, so every card referenced is a person.
fn emit(config: &Config, people: &[Person], families: &[Family]) -> String {
    let xref: HashMap<&str, String> = people
        .iter()
        .enumerate()
        .map(|(index, person)| (person.id.as_str(), format!("@I{}@", index + 1)))
        .collect();
    // The links back from a person to their families. A person is a child in
    // at most one — they have the single pair of parents their card names —
    // and a spouse in as many as they had partners.
    let mut child_in: HashMap<&str, String> = HashMap::new();
    let mut spouse_in: HashMap<&str, Vec<String>> = HashMap::new();
    for (index, family) in families.iter().enumerate() {
        let fam = format!("@F{}@", index + 1);
        for spouse in [&family.father, &family.mother].into_iter().flatten() {
            spouse_in
                .entry(spouse.as_str())
                .or_default()
                .push(fam.clone());
        }
        for child in &family.children {
            child_in.insert(child.as_str(), fam.clone());
        }
    }

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
        // FAMC before FAMS, the order the grammar lists the two links in.
        if let Some(family) = child_in.get(person.id.as_str()) {
            ged.push_str(&format!("1 FAMC {family}\n"));
        }
        for family in spouse_in.get(person.id.as_str()).into_iter().flatten() {
            ged.push_str(&format!("1 FAMS {family}\n"));
        }
    }
    for (index, family) in families.iter().enumerate() {
        ged.push_str(&format!("0 @F{}@ FAM\n", index + 1));
        // The 5.5.1 FAM_RECORD grammar puts the family events ahead of the
        // links, the other way round from INDIVIDUAL_RECORD.
        emit_event(&mut ged, "MARR", family.marriage.as_ref());
        if let Some(father) = &family.father {
            ged.push_str(&format!("1 HUSB {}\n", xref[father.as_str()]));
        }
        if let Some(mother) = &family.mother {
            ged.push_str(&format!("1 WIFE {}\n", xref[mother.as_str()]));
        }
        for child in &family.children {
            ged.push_str(&format!("1 CHIL {}\n", xref[child.as_str()]));
        }
    }
    ged.push_str(&format!("0 @SUB1@ SUBM\n1 NAME {}\n", config.submitter));
    ged.push_str("0 TRLR\n");
    ged
}
