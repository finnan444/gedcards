use std::fs;
use std::path::Path;

use gedcards::{Card, compile};

/// Loads a fixture directory (tree.yaml + people/*.yaml) and returns
/// the compile inputs. Cards come back in file-name order.
fn load_fixture(name: &str) -> (String, Vec<Card>) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let config_yaml = fs::read_to_string(dir.join("tree.yaml")).unwrap();
    let mut cards: Vec<Card> = fs::read_dir(dir.join("people"))
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            Card {
                id: path.file_stem().unwrap().to_str().unwrap().to_string(),
                yaml: fs::read_to_string(&path).unwrap(),
            }
        })
        .collect();
    cards.sort_by(|a, b| a.id.cmp(&b.id));
    (config_yaml, cards)
}

fn expected(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .join("expected.ged");
    fs::read_to_string(path).unwrap()
}

#[test]
fn basic_fixture_compiles_to_expected_ged() {
    let (config_yaml, cards) = load_fixture("basic");
    let ged = compile(&config_yaml, &cards).expect("compile should succeed");
    assert_eq!(ged, expected("basic"));
}

#[test]
fn three_people_fixture_compiles_to_expected_ged() {
    let (config_yaml, cards) = load_fixture("three-people");
    let ged = compile(&config_yaml, &cards).expect("compile should succeed");
    assert_eq!(ged, expected("three-people"));
}

/// Covers the four combinations of the optional name fields: neither,
/// married surname only, both, and patronymic only.
#[test]
fn full_names_fixture_compiles_to_expected_ged() {
    let (config_yaml, cards) = load_fixture("full-names");
    let ged = compile(&config_yaml, &cards).expect("compile should succeed");
    assert_eq!(ged, expected("full-names"));
}

/// Covers all six date forms, both events, and the combinations of date
/// and place: both, date only, and place only.
#[test]
fn dates_fixture_compiles_to_expected_ged() {
    let (config_yaml, cards) = load_fixture("dates");
    let ged = compile(&config_yaml, &cards).expect("compile should succeed");
    assert_eq!(ged, expected("dates"));
}

/// One couple: the marriage declared on the husband's card, four children
/// naming the pair, and the child order that follows from their birth dates
/// rather than from their ids.
#[test]
fn relationships_fixture_compiles_to_expected_ged() {
    let (config_yaml, cards) = load_fixture("relationships");
    let ged = compile(&config_yaml, &cards).expect("compile should succeed");
    assert_eq!(ged, expected("relationships"));
}

/// Covers the families that need no declaring at all: a second pairing with
/// its own children, a child with only a mother, and a childless couple whose
/// only reason to exist is the marriage — declared here from the wife's side.
/// Both shapes a divorce comes in ride along: one with a date and a place,
/// one bare.
#[test]
fn remarriage_fixture_compiles_to_expected_ged() {
    let (config_yaml, cards) = load_fixture("remarriage");
    let ged = compile(&config_yaml, &cards).expect("compile should succeed");
    assert_eq!(ged, expected("remarriage"));
}

#[test]
fn output_does_not_depend_on_card_order() {
    let (config_yaml, mut cards) = load_fixture("three-people");
    cards.reverse();
    let ged = compile(&config_yaml, &cards).expect("compile should succeed");
    assert_eq!(ged, expected("three-people"));
}

/// Families are collected as the cards are walked, so their numbering needs
/// pinning against card order of its own.
#[test]
fn family_output_does_not_depend_on_card_order() {
    let (config_yaml, mut cards) = load_fixture("remarriage");
    cards.reverse();
    let ged = compile(&config_yaml, &cards).expect("compile should succeed");
    assert_eq!(ged, expected("remarriage"));
}
