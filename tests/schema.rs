use std::fs;
use std::path::Path;

use boon::{Compiler, SchemaIndex, Schemas};
use gedcards::{Card, schema};

/// Every fixture directory, all of which compile — see tests/golden.rs.
const FIXTURES: [&str; 7] = [
    "basic",
    "burial",
    "dates",
    "full-names",
    "relationships",
    "remarriage",
    "three-people",
];

const VALID_CARD: &str = "name: Иван\nsurname: Петров\nsex: M\n";

fn card(id: &str, yaml: &str) -> Card {
    Card {
        id: id.to_string(),
        yaml: yaml.to_string(),
    }
}

/// The cards of a fixture directory, in file-name order.
fn load_cards(fixture: &str) -> Vec<Card> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture)
        .join("people");
    let mut cards: Vec<Card> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            card(
                path.file_stem().unwrap().to_str().unwrap(),
                &fs::read_to_string(&path).unwrap(),
            )
        })
        .collect();
    cards.sort_by(|a, b| a.id.cmp(&b.id));
    cards
}

/// The generated schema as a draft-07 implementation reads it, which is the
/// question that matters: not whether the JSON looks right, but whether the
/// validator an editor runs accepts the same cards the compiler does.
struct Schema {
    schemas: Schemas,
    index: SchemaIndex,
}

impl Schema {
    fn of(cards: &[Card]) -> Self {
        let json = serde_json::from_str(&schema(cards)).expect("schema is valid JSON");
        let mut compiler = Compiler::new();
        compiler.add_resource("card.json", json).unwrap();
        let mut schemas = Schemas::new();
        let index = compiler
            .compile("card.json", &mut schemas)
            .expect("schema is a valid draft-07 schema");
        Schema { schemas, index }
    }

    /// Whether a card body validates. The YAML is turned into JSON first,
    /// the way the editor's language server does it.
    fn accepts(&self, yaml: &str) -> bool {
        let value: serde_norway::Value = serde_norway::from_str(yaml).unwrap();
        let json = serde_json::to_value(value).unwrap();
        self.schemas.validate(&json, self.index).is_ok()
    }
}

/// The tree the negative cases are written against: one man, one woman.
fn couple() -> Vec<Card> {
    vec![
        card("ivan-petrov", VALID_CARD),
        card("anna-petrova", "name: Анна\nsurname: Петрова\nsex: F\n"),
    ]
}

#[test]
fn every_fixture_card_validates_against_its_own_schema() {
    for fixture in FIXTURES {
        let cards = load_cards(fixture);
        let schema = Schema::of(&cards);
        for card in &cards {
            assert!(
                schema.accepts(&card.yaml),
                "{fixture}: {} should validate",
                card.id
            );
        }
    }
}

#[test]
fn schema_matches_the_golden_file() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/relationships/expected.schema.json");
    let cards = load_cards("relationships");
    assert_eq!(schema(&cards), fs::read_to_string(path).unwrap());
}

#[test]
fn schema_does_not_depend_on_card_order() {
    let mut cards = load_cards("remarriage");
    let forwards = schema(&cards);
    cards.reverse();
    assert_eq!(schema(&cards), forwards);
}

#[test]
fn unknown_keys_are_rejected() {
    let schema = Schema::of(&couple());
    assert!(!schema.accepts("name: Иван\nsurname: Петров\nsex: M\nage: 44\n"));
    assert!(!schema.accepts("name: Иван\nsurname: Петров\nsex: M\nbirth:\n  year: 1947\n"));
    assert!(!schema.accepts(
        "name: Иван\nsurname: Петров\nsex: M\nmarriage:\n  spouse: anna-petrova\n  town: Тверь\n"
    ));
}

#[test]
fn the_required_fields_are_required() {
    let schema = Schema::of(&couple());
    assert!(!schema.accepts("surname: Петров\nsex: M\n"));
    assert!(!schema.accepts("name: Иван\nsex: M\n"));
    assert!(!schema.accepts("name: Иван\nsurname: Петров\n"));
    // A key with no value is as absent as no key, to the compiler and here.
    assert!(!schema.accepts("name:\nsurname: Петров\nsex: M\n"));
}

#[test]
fn sex_is_m_or_f() {
    let schema = Schema::of(&couple());
    assert!(schema.accepts("name: Иван\nsurname: Петров\nsex: M\n"));
    assert!(schema.accepts("name: Анна\nsurname: Петрова\nsex: F\n"));
    assert!(!schema.accepts("name: Иван\nsurname: Петров\nsex: X\n"));
    assert!(!schema.accepts("name: Иван\nsurname: Петров\nsex: m\n"));
    assert!(!schema.accepts("name: Иван\nsurname: Петров\nsex: male\n"));
}

#[test]
fn dates_follow_the_card_grammar() {
    let schema = Schema::of(&couple());
    let with_date =
        |date: &str| format!("name: Иван\nsurname: Петров\nsex: M\nbirth:\n  date: {date}\n");
    for date in [
        "1995-07-25",
        "1995-07",
        "1995",
        "~1910",
        "<1910",
        "'>1910'",
        // A leading zero keeps YAML from reading the year as an integer, so
        // this arrives as the string the pattern is written for.
        "0001",
    ] {
        assert!(schema.accepts(&with_date(date)), "{date} should validate");
    }
    for date in [
        "1995-7-25",
        "1995-13",
        "1995-00",
        "1995-07-32",
        "1995-07-00",
        "0000",
        "199",
        "12.03.1947",
        "'1995 '",
        "1995-07-25-01",
        "~~1910",
    ] {
        assert!(
            !schema.accepts(&with_date(date)),
            "{date} should not validate"
        );
    }
}

#[test]
fn an_event_needs_a_date_or_a_place() {
    let schema = Schema::of(&couple());
    assert!(schema.accepts("name: Иван\nsurname: Петров\nsex: M\nbirth:\n  place: Тверь\n"));
    assert!(!schema.accepts("name: Иван\nsurname: Петров\nsex: M\nbirth: {}\n"));
    assert!(!schema.accepts("name: Иван\nsurname: Петров\nsex: M\nbirth:\n"));
    assert!(!schema.accepts("name: Иван\nsurname: Петров\nsex: M\nbirth: 1947\n"));
}

/// `coords` follow the pair shape the compiler parses; a `note` is any string.
/// The degree bounds are the compiler's to check, so the schema takes the shape
/// alone — an editor still catches the common slips.
#[test]
fn coords_and_note_follow_the_event_grammar() {
    let schema = Schema::of(&couple());
    let event = |body: &str| format!("name: Иван\nsurname: Петров\nsex: M\nburial:\n{body}");
    assert!(schema.accepts(&event("  place: Тверь\n  coords: 55.7314, 37.9256\n")));
    assert!(schema.accepts(&event("  place: Тверь\n  coords: -55.7314, -37.9256\n")));
    assert!(schema.accepts(&event("  place: Тверь\n  note: у главной аллеи слева\n")));
    // Not a pair, or not numbers at all.
    assert!(!schema.accepts(&event("  place: Тверь\n  coords: 55.7314\n")));
    assert!(!schema.accepts(&event("  place: Тверь\n  coords: north, east\n")));
}

#[test]
fn a_marriage_needs_a_spouse() {
    let schema = Schema::of(&couple());
    assert!(
        schema.accepts("name: Иван\nsurname: Петров\nsex: M\nmarriage:\n  spouse: anna-petrova\n")
    );
    assert!(!schema.accepts("name: Иван\nsurname: Петров\nsex: M\nmarriage:\n  place: Тверь\n"));
    assert!(!schema.accepts("name: Иван\nsurname: Петров\nsex: M\nmarriage:\n"));
}

#[test]
fn a_parent_is_named_from_the_cards_of_that_sex() {
    let schema = Schema::of(&couple());
    assert!(schema.accepts(
        "name: Ольга\nsurname: Петрова\nsex: F\nfather: ivan-petrov\nmother: anna-petrova\n"
    ));
    // The compiler's role check, answered before the build: a woman named as
    // the father is not one of the ids `father` may take.
    assert!(!schema.accepts("name: Ольга\nsurname: Петрова\nsex: F\nfather: anna-petrova\n"));
    assert!(!schema.accepts("name: Ольга\nsurname: Петрова\nsex: F\nmother: ivan-petrov\n"));
    assert!(!schema.accepts("name: Ольга\nsurname: Петрова\nsex: F\nfather: pyotr-ivanov\n"));
}

/// A divorce is an event block like any other, except that carrying nothing
/// is allowed: that the marriage ended is the whole fact.
#[test]
fn a_divorce_may_carry_nothing() {
    let schema = Schema::of(&couple());
    let with_divorce = |divorce: &str| {
        format!("name: Иван\nsurname: Петров\nsex: M\nmarriage:\n  spouse: anna-petrova\n{divorce}")
    };
    assert!(schema.accepts(&with_divorce("  divorce:\n")));
    assert!(schema.accepts(&with_divorce("  divorce: {}\n")));
    assert!(schema.accepts(&with_divorce(
        "  divorce:\n    date: 1981-04\n    place: Тверь\n"
    )));
    assert!(!schema.accepts(&with_divorce("  divorce:\n    date: 12.03.1981\n")));
    assert!(!schema.accepts(&with_divorce("  divorce:\n    town: Тверь\n")));
    assert!(!schema.accepts(&with_divorce("  divorce: 1981-04\n")));
}

/// Narrowing `spouse` by the card's own sex is a non-goal: every id is offered.
#[test]
fn a_spouse_is_any_card() {
    let schema = Schema::of(&couple());
    let with_spouse =
        |id: &str| format!("name: Иван\nsurname: Петров\nsex: M\nmarriage:\n  spouse: {id}\n");
    assert!(schema.accepts(&with_spouse("anna-petrova")));
    assert!(schema.accepts(&with_spouse("ivan-petrov")));
    assert!(!schema.accepts(&with_spouse("pyotr-ivanov")));
}

/// The same rule `card_ids` follows in `compile`: the file exists, so the id
/// can be referenced, whatever state the body is in.
#[test]
fn a_broken_or_empty_card_still_contributes_its_id() {
    let cards = [
        card("ivan-petrov", VALID_CARD),
        card("boris-orlov", "name: [Борис\n"),
        card("pavel-sokolov", ""),
    ];
    let schema = Schema::of(&cards);
    let with_spouse =
        |id: &str| format!("name: Анна\nsurname: Петрова\nsex: F\nmarriage:\n  spouse: {id}\n");
    assert!(schema.accepts(&with_spouse("boris-orlov")));
    assert!(schema.accepts(&with_spouse("pavel-sokolov")));
    // Neither says it is a man, so neither may be a father.
    assert!(!schema.accepts("name: Анна\nsurname: Петрова\nsex: F\nfather: boris-orlov\n"));
}

/// An empty `enum` matches nothing, which would make the field unusable rather
/// than merely unchecked; the shape of an id is what is left to say.
#[test]
fn a_tree_with_nobody_of_a_sex_falls_back_to_the_id_shape() {
    let schema = Schema::of(&[card("ivan-petrov", VALID_CARD)]);
    assert!(schema.accepts("name: Ольга\nsurname: Петрова\nsex: F\nmother: anna-petrova\n"));
    assert!(!schema.accepts("name: Ольга\nsurname: Петрова\nsex: F\nmother: Анна\n"));
    assert!(!schema.accepts("name: Ольга\nsurname: Петрова\nsex: F\nmother: anna--petrova\n"));
    // The men are known, so `father` still enumerates them.
    assert!(!schema.accepts("name: Ольга\nsurname: Петрова\nsex: F\nfather: boris-orlov\n"));
}

/// An id can be anything a file name can be. The card is a compile error, but
/// the schema still has to name it without breaking the JSON around it.
#[test]
fn an_id_that_is_not_a_slug_is_still_valid_json() {
    let cards = [card("иван \"the\\quoted\"", VALID_CARD)];
    let json: serde_json::Value = serde_json::from_str(&schema(&cards)).expect("valid JSON");
    let spouse = &json["properties"]["marriage"]["properties"]["spouse"]["enum"];
    assert_eq!(spouse[0], "иван \"the\\quoted\"");
}
