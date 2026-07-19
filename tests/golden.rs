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

#[test]
fn output_does_not_depend_on_card_order() {
    let (config_yaml, mut cards) = load_fixture("three-people");
    cards.reverse();
    let ged = compile(&config_yaml, &cards).expect("compile should succeed");
    assert_eq!(ged, expected("three-people"));
}
