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

/// The optional name fields may be left out, but a value that is not
/// a string is still a mistake worth reporting.
#[test]
fn non_string_optional_name_fields_are_reported() {
    let cards = [card(
        "ivan-petrov",
        "name: Иван\npatronymic: [Петрович]\nsurname: Петров\nmarried_surname: 1917\nsex: M\n",
    )];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(Some("ivan-petrov"), Some("patronymic"), "expected a string"),
            diagnostic(
                Some("ivan-petrov"),
                Some("married_surname"),
                "expected a string"
            ),
        ]
    );
}

/// An empty optional field would otherwise reach the emitter and produce
/// a stray space in GIVN or a valueless `2 _MARNM` line.
#[test]
fn blank_optional_name_fields_are_reported() {
    let cards = [card(
        "ivan-petrov",
        "name: Иван\npatronymic: ''\nsurname: Петров\nmarried_surname: '   '\nsex: M\n",
    )];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(Some("ivan-petrov"), Some("patronymic"), "must not be empty"),
            diagnostic(
                Some("ivan-petrov"),
                Some("married_surname"),
                "must not be empty"
            ),
        ]
    );
}

/// The same rule holds for required fields: a blank one is as unusable
/// as a missing one, and silently emits `1 NAME  //`.
#[test]
fn blank_required_card_fields_are_reported() {
    let cards = [card("ivan-petrov", "name: ''\nsurname: '  '\nsex: M\n")];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(Some("ivan-petrov"), Some("name"), "must not be empty"),
            diagnostic(Some("ivan-petrov"), Some("surname"), "must not be empty"),
        ]
    );
}

/// `patronymic:` is how someone writes "no patronymic", so the diagnostic
/// names the fix instead of complaining about the type. All three YAML
/// spellings of null read the same.
#[test]
fn valueless_optional_key_is_reported() {
    for yaml in [
        "name: Иван\npatronymic:\nsurname: Петров\nsex: M\n",
        "name: Иван\npatronymic: null\nsurname: Петров\nsex: M\n",
        "name: Иван\npatronymic: ~\nsurname: Петров\nsex: M\n",
    ] {
        let cards = [card("ivan-petrov", yaml)];
        let diagnostics = compile(CONFIG, &cards).unwrap_err();
        assert_eq!(
            diagnostics,
            vec![diagnostic(
                Some("ivan-petrov"),
                Some("patronymic"),
                "remove the key instead of leaving it empty"
            )],
            "for {yaml:?}"
        );
    }
}

/// A required key with no value is as absent as no key at all, and says so.
#[test]
fn valueless_required_key_reads_as_missing() {
    let cards = [card("ivan-petrov", "name:\nsurname: Петров\nsex: M\n")];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("ivan-petrov"),
            Some("name"),
            "required field is missing"
        )]
    );
}

/// Padding is refused rather than trimmed: silently rewriting the value
/// would leave the card and the emitted GEDCOM saying different things.
#[test]
fn padded_values_are_reported() {
    let cards = [card(
        "ivan-petrov",
        "name: ' Иван'\npatronymic: 'Петрович '\nsurname: Петров\nsex: M\n",
    )];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    let reason = "must not have leading or trailing whitespace";
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(Some("ivan-petrov"), Some("name"), reason),
            diagnostic(Some("ivan-petrov"), Some("patronymic"), reason),
        ]
    );
}

#[test]
fn blank_config_value_is_reported() {
    let config = "submitter: ''\nlanguage: Russian\n";
    let cards = [card("ivan-petrov", VALID_CARD)];
    let diagnostics = compile(config, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(None, Some("submitter"), "must not be empty")]
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

/// The date grammar is a closed set of six forms. Anything else is refused
/// rather than guessed at, including forms a reader might think obvious:
/// a two-digit year, a day-first date, and a month out of range.
#[test]
fn unrecognized_dates_are_reported() {
    for date in [
        "25.07.1995",
        "25 JUL 1995",
        "95-07-25",
        "1995-7-25",
        "1995-13",
        "1995-07-32",
        "1995-00",
        "about 1910",
        "1995-07-25-01",
        "0000",
    ] {
        let yaml = format!("name: Иван\nsurname: Петров\nsex: M\nbirth:\n  date: '{date}'\n");
        let cards = [card("ivan-petrov", &yaml)];
        let diagnostics = compile(CONFIG, &cards).unwrap_err();
        assert_eq!(
            diagnostics,
            vec![diagnostic(
                Some("ivan-petrov"),
                Some("birth.date"),
                "expected a date like 1995-07-25, 1995-07 or 1995, optionally prefixed with ~, < or >"
            )],
            "for {date:?}"
        );
    }
}

/// Every date form, including the ones YAML hands over as something other
/// than a string, reaches the compiler intact.
#[test]
fn every_date_form_is_accepted() {
    for date in ["1995-07-25", "1995-07", "1995", "~1910", "<1910", "'>1910'"] {
        let yaml = format!("name: Иван\nsurname: Петров\nsex: M\ndeath:\n  date: {date}\n");
        let cards = [card("ivan-petrov", &yaml)];
        assert!(compile(CONFIG, &cards).is_ok(), "for {date:?}");
    }
}

#[test]
fn non_mapping_event_is_reported() {
    let cards = [card(
        "ivan-petrov",
        "name: Иван\nsurname: Петров\nsex: M\nbirth: 1995-07-25\n",
    )];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("ivan-petrov"),
            Some("birth"),
            "expected a block with a date, a place, an age and/or a cause"
        )]
    );
}

/// An event with neither part says nothing, and would emit a bare `1 BIRT`.
#[test]
fn empty_event_block_is_reported() {
    let cards = [card(
        "ivan-petrov",
        "name: Иван\nsurname: Петров\nsex: M\nbirth: {}\n",
    )];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("ivan-petrov"),
            Some("birth"),
            "needs a date, a place, an age or a cause"
        )]
    );
}

#[test]
fn valueless_event_key_is_reported() {
    let cards = [card(
        "ivan-petrov",
        "name: Иван\nsurname: Петров\nsex: M\ndeath:\n",
    )];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("ivan-petrov"),
            Some("death"),
            "remove the key instead of leaving it empty"
        )]
    );
}

/// Keys inside an event block are checked like any others, and the
/// diagnostic names the full path so it is clear which block they sit in.
#[test]
fn unknown_key_inside_an_event_is_reported() {
    let cards = [card(
        "ivan-petrov",
        "name: Иван\nsurname: Петров\nsex: M\nbirth:\n  date: 1995\n  city: Москва\n",
    )];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("ivan-petrov"),
            Some("birth.city"),
            "unknown key"
        )]
    );
}

/// The rules that hold for top-level values hold inside an event block too.
#[test]
fn blank_and_padded_event_values_are_reported() {
    let cards = [card(
        "ivan-petrov",
        "name: Иван\nsurname: Петров\nsex: M\nbirth:\n  date: ' 1995'\n  place: ''\n",
    )];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(
                Some("ivan-petrov"),
                Some("birth.date"),
                "must not have leading or trailing whitespace"
            ),
            diagnostic(
                Some("ivan-petrov"),
                Some("birth.place"),
                "must not be empty"
            ),
        ]
    );
}

/// A mistyped id is the common way a reference goes wrong, so the diagnostic
/// names the id that was probably meant instead of only saying "unknown".
#[test]
fn unknown_parent_id_names_the_closest_one() {
    let cards = [
        card("ivan-petrov", VALID_CARD),
        card(
            "anna-petrova",
            "name: Анна\nsurname: Петрова\nsex: F\nfather: ivan-petroff\n",
        ),
    ];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("anna-petrova"),
            Some("father"),
            "no card with id ivan-petroff, did you mean ivan-petrov?"
        )]
    );
}

/// An id nothing resembles is a card that was never written rather than a
/// typo, and naming the nearest stranger would only mislead.
#[test]
fn unknown_id_far_from_every_card_is_reported_on_its_own() {
    let cards = [
        card("ivan-petrov", VALID_CARD),
        card(
            "anna-petrova",
            "name: Анна\nsurname: Петрова\nsex: F\nmother: zinaida-kuznetsova\n",
        ),
    ];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("anna-petrova"),
            Some("mother"),
            "no card with id zinaida-kuznetsova"
        )]
    );
}

#[test]
fn reference_to_the_card_itself_is_reported() {
    let cards = [card(
        "ivan-petrov",
        "name: Иван\nsurname: Петров\nsex: M\nfather: ivan-petrov\n",
    )];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("ivan-petrov"),
            Some("father"),
            "must not be the card's own id"
        )]
    );
}

/// The compiler has to know which spouse is the husband to emit HUSB and
/// WIFE, and a card naming a woman as `father` is a mix-up rather than a
/// family whose shape the emitter should invent.
#[test]
fn parent_of_the_wrong_sex_is_reported() {
    let cards = [
        card("anna-petrova", "name: Анна\nsurname: Петрова\nsex: F\n"),
        card("ivan-petrov", VALID_CARD),
        card(
            "olga-petrova",
            "name: Ольга\nsurname: Петрова\nsex: F\nfather: anna-petrova\nmother: ivan-petrov\n",
        ),
    ];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(
                Some("olga-petrova"),
                Some("father"),
                "anna-petrova has sex F, expected M"
            ),
            diagnostic(
                Some("olga-petrova"),
                Some("mother"),
                "ivan-petrov has sex M, expected F"
            ),
        ]
    );
}

#[test]
fn spouse_of_the_same_sex_is_reported() {
    let cards = [
        card("ivan-petrov", VALID_CARD),
        card(
            "pyotr-petrov",
            "name: Пётр\nsurname: Петров\nsex: M\nmarriage:\n  spouse: ivan-petrov\n",
        ),
    ];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("pyotr-petrov"),
            Some("marriage.spouse"),
            "ivan-petrov has sex M, expected F"
        )]
    );
}

/// A marriage belongs to one card. Declared on both, it is one fact kept in
/// two places, which is what the card format exists to avoid.
#[test]
fn marriage_declared_on_both_cards_is_reported() {
    let cards = [
        card(
            "anna-petrova",
            "name: Анна\nsurname: Петрова\nsex: F\nmarriage:\n  spouse: ivan-petrov\n",
        ),
        card(
            "ivan-petrov",
            "name: Иван\nsurname: Петров\nsex: M\nmarriage:\n  spouse: anna-petrova\n  date: 1946\n",
        ),
    ];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("ivan-petrov"),
            Some("marriage"),
            "also declared on anna-petrova's card, keep only one"
        )]
    );
}

/// Two people marrying the same third person is not a double declaration:
/// those are two marriages, and each is declared once.
#[test]
fn two_marriages_to_the_same_person_are_accepted() {
    let cards = [
        card("ivan-petrov", VALID_CARD),
        card(
            "anna-petrova",
            "name: Анна\nsurname: Петрова\nsex: F\nmarriage:\n  spouse: ivan-petrov\n",
        ),
        card(
            "olga-petrova",
            "name: Ольга\nsurname: Петрова\nsex: F\nmarriage:\n  spouse: ivan-petrov\n",
        ),
    ];
    assert!(compile(CONFIG, &cards).is_ok());
}

/// The spouse is what makes the family, so a marriage block without one has
/// nothing to synthesize from.
#[test]
fn marriage_without_a_spouse_is_reported() {
    let cards = [card(
        "ivan-petrov",
        "name: Иван\nsurname: Петров\nsex: M\nmarriage:\n  date: 1946\n",
    )];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("ivan-petrov"),
            Some("marriage.spouse"),
            "required field is missing"
        )]
    );
}

#[test]
fn malformed_marriage_block_is_reported() {
    let cards = [
        card(
            "ivan-petrov",
            "name: Иван\nsurname: Петров\nsex: M\nmarriage: anna-petrova\n",
        ),
        card(
            "pyotr-petrov",
            "name: Пётр\nsurname: Петров\nsex: M\nmarriage:\n",
        ),
    ];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(
                Some("ivan-petrov"),
                Some("marriage"),
                "expected a block with a spouse, and optionally a date, a place and a divorce"
            ),
            diagnostic(
                Some("pyotr-petrov"),
                Some("marriage"),
                "remove the key instead of leaving it empty"
            ),
        ]
    );
}

#[test]
fn unknown_key_inside_a_marriage_is_reported() {
    let cards = [
        card("anna-petrova", "name: Анна\nsurname: Петрова\nsex: F\n"),
        card(
            "ivan-petrov",
            "name: Иван\nsurname: Петров\nsex: M\nmarriage:\n  spouse: anna-petrova\n  town: Тверь\n",
        ),
    ];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("ivan-petrov"),
            Some("marriage.town"),
            "unknown key"
        )]
    );
}

/// A divorce says the marriage it sits in ended. Saying nothing else about it
/// is not a mistake — that is the whole fact, and `1 DIV Y` is how GEDCOM
/// asserts it. Both of YAML's spellings for an empty block say the same thing.
#[test]
fn a_divorce_with_nothing_in_it_is_accepted() {
    for divorce in ["  divorce:\n", "  divorce: {}\n"] {
        let yaml = format!(
            "name: Иван\nsurname: Петров\nsex: M\nmarriage:\n  spouse: anna-petrova\n{divorce}"
        );
        let cards = [
            card("anna-petrova", "name: Анна\nsurname: Петрова\nsex: F\n"),
            card("ivan-petrov", &yaml),
        ];
        let ged = compile(CONFIG, &cards).expect("compile should succeed");
        assert!(ged.contains("1 DIV Y\n"), "for {divorce:?}");
    }
}

#[test]
fn unrecognized_divorce_date_is_reported() {
    let cards = [
        card("anna-petrova", "name: Анна\nsurname: Петрова\nsex: F\n"),
        card(
            "ivan-petrov",
            "name: Иван\nsurname: Петров\nsex: M\nmarriage:\n  spouse: anna-petrova\n  divorce:\n    date: 12.03.1981\n",
        ),
    ];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("ivan-petrov"),
            Some("marriage.divorce.date"),
            "expected a date like 1995-07-25, 1995-07 or 1995, optionally prefixed with ~, < or >"
        )]
    );
}

#[test]
fn malformed_divorce_block_is_reported() {
    let cards = [
        card("anna-petrova", "name: Анна\nsurname: Петрова\nsex: F\n"),
        card(
            "ivan-petrov",
            "name: Иван\nsurname: Петров\nsex: M\nmarriage:\n  spouse: anna-petrova\n  divorce: 1981-04\n",
        ),
    ];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("ivan-petrov"),
            Some("marriage.divorce"),
            "expected a block with a date and/or a place, or nothing at all"
        )]
    );
}

#[test]
fn unknown_key_inside_a_divorce_is_reported() {
    let cards = [
        card("anna-petrova", "name: Анна\nsurname: Петрова\nsex: F\n"),
        card(
            "ivan-petrov",
            "name: Иван\nsurname: Петров\nsex: M\nmarriage:\n  spouse: anna-petrova\n  divorce:\n    town: Тверь\n",
        ),
    ];
    let diagnostics = compile(CONFIG, &cards).unwrap_err();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("ivan-petrov"),
            Some("marriage.divorce.town"),
            "unknown key"
        )]
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
