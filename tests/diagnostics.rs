use gedcards::{Card, Diagnostic, compile};

const CONFIG: &str = "submitter: Иван Иванов\nlanguage: Russian\n";

fn card(id: &str, yaml: &str) -> Card {
    Card {
        id: id.to_string(),
        yaml: yaml.to_string(),
    }
}

fn diagnostic(card: Option<&str>, field: Option<&str>, reason: &str) -> Diagnostic {
    Diagnostic {
        card: card.map(String::from),
        field: field.map(String::from),
        reason: reason.to_string(),
    }
}

const VALID_CARD: &str = "name: Иван\nsurname: Петров\nsex: M\n";

#[test]
fn non_slug_ids_are_reported_for_every_bad_card() {
    let cards = [
        card("иван-петров", VALID_CARD),
        card("Ivan_Petrov", VALID_CARD),
    ];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    let reason = "id must be a slug of lowercase latin letters, digits and hyphens";
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(Some("иван-петров"), None, reason),
            diagnostic(Some("Ivan_Petrov"), None, reason),
        ]
    );
}

#[test]
fn duplicate_id_is_reported() {
    let cards = [
        card("ivan-petrov", VALID_CARD),
        card("ivan-petrov", VALID_CARD),
    ];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(Some("ivan-petrov"), None, "duplicate id")]
    );
}

/// A broken id must not swallow the rest of that card's problems —
/// otherwise the card gets fixed one diagnostic per run.
#[test]
fn bad_id_does_not_hide_the_rest_of_the_card() {
    let cards = [card(
        "иван",
        "name: Иван\nsurname: Петров\nsex: X\nage: 44\n",
    )];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(
                Some("иван"),
                None,
                "id must be a slug of lowercase latin letters, digits and hyphens"
            ),
            diagnostic(Some("иван"), Some("sex"), "expected M or F"),
            diagnostic(Some("иван"), Some("age"), "unknown key"),
        ]
    );
}

#[test]
fn duplicate_id_does_not_hide_the_rest_of_the_card() {
    let cards = [
        card("ivan-petrov", VALID_CARD),
        card("ivan-petrov", "name: Иван\nsurname: Петров\nsex: X\n"),
    ];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(Some("ivan-petrov"), None, "duplicate id"),
            diagnostic(Some("ivan-petrov"), Some("sex"), "expected M or F"),
        ]
    );
}

#[test]
fn invalid_sex_value_is_reported() {
    let cards = [card(
        "ivan-petrov",
        "name: Иван\nsurname: Петров\nsex: муж\n",
    )];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("ivan-petrov"),
            Some("sex"),
            "expected M or F"
        )]
    );
}

#[test]
fn missing_required_card_fields_are_reported() {
    let cards = [card("ivan-petrov", "name: Иван\n")];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(
                Some("ivan-petrov"),
                Some("surname"),
                "required field is missing"
            ),
            diagnostic(
                Some("ivan-petrov"),
                Some("sex"),
                "required field is missing"
            ),
        ]
    );
}

#[test]
fn broken_card_yaml_is_reported() {
    let cards = [card("ivan-petrov", "name: [unclosed\n")];
    let result = compile(CONFIG, &cards);
    let diagnostics = result.unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].card.as_deref(), Some("ivan-petrov"));
    assert_eq!(diagnostics[0].field, None);
    assert!(diagnostics[0].reason.starts_with("invalid YAML:"));
}

#[test]
fn unknown_config_key_is_reported() {
    let config = "submitter: Тест\nlanguage: Russian\nlang: ru\n";
    let cards = [card("ivan-petrov", VALID_CARD)];
    let diagnostics = compile(config, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(None, Some("lang"), "unknown key")]
    );
}

#[test]
fn missing_config_key_is_reported() {
    let config = "submitter: Тест\n";
    let cards = [card("ivan-petrov", VALID_CARD)];
    let diagnostics = compile(config, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            None,
            Some("language"),
            "required field is missing"
        )]
    );
}

#[test]
fn config_and_card_problems_accumulate_in_one_run() {
    let config = "submitter: Тест\nlanguage: Russian\nlang: ru\n";
    let cards = [
        card("ivan-petrov", "name: Иван\nsurname: Петров\nsex: муж\n"),
        card("иван", VALID_CARD),
    ];
    let diagnostics = compile(config, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(None, Some("lang"), "unknown key"),
            diagnostic(Some("ivan-petrov"), Some("sex"), "expected M or F"),
            diagnostic(
                Some("иван"),
                None,
                "id must be a slug of lowercase latin letters, digits and hyphens"
            ),
        ]
    );
}

#[test]
fn unknown_card_key_is_reported() {
    let cards = [card(
        "ivan-petrov",
        "name: Иван\nsurname: Петров\nsex: M\nage: 44\n",
    )];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(Some("ivan-petrov"), Some("age"), "unknown key")]
    );
}
