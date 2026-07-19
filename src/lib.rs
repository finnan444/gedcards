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

struct Person {
    id: String,
    name: String,
    patronymic: Option<String>,
    surname: String,
    /// Surname taken at marriage. The primary surname stays the one at birth,
    /// so maiden names keep displaying correctly after import.
    married_surname: Option<String>,
    sex: String,
}

impl Person {
    /// The given name as GEDCOM spells it: the patronymic is part of it,
    /// even though the card keeps the two apart.
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

/// Pulls a required string field out of a parsed mapping, reporting
/// a diagnostic (attributed to `card`, None for config) when absent or non-string.
fn take_string(
    mapping: &mut serde_norway::Mapping,
    field: &str,
    card: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    match mapping.remove(field) {
        Some(serde_norway::Value::String(value)) => Some(value),
        Some(_) => {
            diagnostics.push(Diagnostic {
                card: card.map(String::from),
                field: Some(field.to_string()),
                reason: "expected a string".to_string(),
            });
            None
        }
        None => {
            diagnostics.push(Diagnostic {
                card: card.map(String::from),
                field: Some(field.to_string()),
                reason: "required field is missing".to_string(),
            });
            None
        }
    }
}

/// Like `take_string`, but an absent field is not a problem. A present
/// field still has to be a string, so a typo'd value is still caught.
fn take_optional_string(
    mapping: &mut serde_norway::Mapping,
    field: &str,
    card: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    match mapping.remove(field) {
        Some(serde_norway::Value::String(value)) => Some(value),
        Some(_) => {
            diagnostics.push(Diagnostic {
                card: card.map(String::from),
                field: Some(field.to_string()),
                reason: "expected a string".to_string(),
            });
            None
        }
        None => None,
    }
}

/// Every key left in the mapping after the known ones were taken out
/// is unknown; reported in sorted order for determinism.
fn report_unknown_keys(
    mapping: serde_norway::Mapping,
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
            field: Some(key),
            reason: "unknown key".to_string(),
        });
    }
}

fn parse_config(config_yaml: &str, diagnostics: &mut Vec<Diagnostic>) -> Option<Config> {
    let mut mapping = parse_mapping(config_yaml, None, diagnostics)?;
    let submitter = take_string(&mut mapping, "submitter", None, diagnostics);
    let language = take_string(&mut mapping, "language", None, diagnostics);
    report_unknown_keys(mapping, None, diagnostics);
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
    report_unknown_keys(mapping, Some(&card.id), diagnostics);
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
    })
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
    }
    ged.push_str(&format!("0 @SUB1@ SUBM\n1 NAME {}\n", config.submitter));
    ged.push_str("0 TRLR\n");
    ged
}
