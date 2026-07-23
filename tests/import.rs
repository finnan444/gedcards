use std::collections::HashMap;
use std::fs;
use std::path::Path;

use gedcards::{Card, Diagnostic, compile, import};

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

/// Loads a build fixture (tree.yaml + people/*.yaml), the inputs `compile`
/// takes. Cards come back in file-name order.
fn load_fixture(name: &str) -> (String, Vec<Card>) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let config_yaml = fs::read_to_string(dir.join("tree.yaml")).unwrap();
    let mut cards: Vec<Card> = fs::read_dir(dir.join("people"))
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
    (config_yaml, cards)
}

/// A golden import: a MyHeritage-shaped export — its own header, `_MARNM`, a
/// patronymic fused into `GIVN` — read into the cards checked in beside it. The
/// export is trimmed to names and sex, the fields import has a home for; dates
/// and families arrive with issues #1 and #4.
#[test]
fn myheritage_export_imports_to_the_expected_cards() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/import/myheritage");
    let ged = fs::read_to_string(dir.join("family.ged")).unwrap();

    let (tree_yaml, cards) = import(&ged).expect("import should succeed");

    assert_eq!(
        tree_yaml,
        fs::read_to_string(dir.join("tree.yaml")).unwrap()
    );
    let produced: HashMap<&str, &str> = cards
        .iter()
        .map(|card| (card.id.as_str(), card.yaml.as_str()))
        .collect();
    let expected: HashMap<String, String> = fs::read_dir(dir.join("people"))
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_stem().unwrap().to_str().unwrap().to_string(),
                fs::read_to_string(&path).unwrap(),
            )
        })
        .collect();
    assert_eq!(produced.len(), expected.len(), "same number of cards");
    for (id, yaml) in &expected {
        assert_eq!(produced.get(id.as_str()), Some(&yaml.as_str()), "card {id}");
    }
}

/// The round trip: build a fixture, import its output, build again — the two
/// `.ged` files are byte-identical. The imported ids differ from the authored
/// ones (a patronymic drops out, `Мария` transliterates to `mariya`), but they
/// sort the same and no id appears in the output, so the bytes match. Both
/// fixtures are names and sex only, the shape import round-trips today.
#[test]
fn build_import_build_is_byte_identical() {
    for fixture in ["full-names", "three-people"] {
        let (config, cards) = load_fixture(fixture);
        let first = compile(&config, &cards).expect("first build should succeed");

        let (tree_yaml, imported) = import(&first).expect("import should succeed");
        let second = compile(&tree_yaml, &imported).expect("second build should succeed");

        assert_eq!(first, second, "round trip differs for {fixture}");
    }
}

const HEADER: &str = "0 HEAD\n1 CHAR UTF-8\n1 SUBM @SUB1@\n1 LANG Russian\n";
const SUBMITTER: &str = "0 @SUB1@ SUBM\n1 NAME Иван Иванов\n0 TRLR\n";

/// A tool's bookkeeping — MyHeritage's `_UID`, `_UPD`, the standard `RIN`
/// record key — is envelope, not a person's fact, so it is dropped without a
/// diagnostic the way the header's metadata is. The name and sex still import.
#[test]
fn tool_bookkeeping_tags_are_dropped_without_a_diagnostic() {
    let ged = format!(
        "{HEADER}\
0 @I1@ INDI\n\
1 _UPD 3 SEP 2025 02:42:09 GMT -0500\n\
1 NAME Иван /Иванов/\n\
2 GIVN Иван\n\
2 SURN Иванов\n\
1 SEX M\n\
1 RIN MH:I1\n\
1 _UID 68B0683499F445A60024280905725BDC\n\
{SUBMITTER}"
    );
    let (_, cards) = import(&ged).expect("import should succeed");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].id, "ivan-ivanov");
    assert_eq!(cards[0].yaml, "name: Иван\nsurname: Иванов\nsex: M\n");
}

/// A tag with no card field is named rather than dropped: a birth date, a name
/// piece the card cannot hold, and the whole `FAM` record that would carry
/// relationships. Every one is reported in the same run.
#[test]
fn tags_with_no_card_field_are_reported() {
    let ged = format!(
        "{HEADER}\
0 @I1@ INDI\n\
1 NAME Иван /Иванов/\n\
2 GIVN Иван\n\
2 SURN Иванов\n\
2 NPFX Dr\n\
1 SEX M\n\
1 BIRT\n\
2 DATE 12 MAR 1947\n\
0 @F1@ FAM\n\
1 HUSB @I1@\n\
{SUBMITTER}"
    );
    let diagnostics = import(&ged).err().unwrap();
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(Some("@I1@"), None, "NAME piece NPFX is not imported yet"),
            diagnostic(Some("@I1@"), None, "BIRT is not imported yet"),
            diagnostic(
                Some("@F1@"),
                None,
                "relationships are not imported yet — they arrive with FAM records (#4)"
            ),
        ]
    );
}

/// A file MyHeritage exports leads with a UTF-8 BOM. It is stripped, so the
/// `0 HEAD` behind it parses and the header — `LANG` and all — reads normally
/// rather than the whole file collapsing on the first line.
#[test]
fn a_leading_byte_order_mark_is_stripped() {
    let ged = format!(
        "\u{feff}{HEADER}\
0 @I1@ INDI\n\
1 NAME Иван /Иванов/\n\
2 GIVN Иван\n\
2 SURN Иванов\n\
1 SEX M\n\
{SUBMITTER}"
    );
    let (tree_yaml, cards) = import(&ged).expect("import should succeed");
    assert_eq!(tree_yaml, "submitter: Иван Иванов\nlanguage: Russian\n");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].id, "ivan-ivanov");
}

/// Two people whose names transliterate to the same slug, with no birth year yet
/// to tell them apart, get a numeric suffix in file order — deterministic for
/// the same file.
#[test]
fn namesakes_get_a_numeric_suffix() {
    let ged = format!(
        "{HEADER}\
0 @I1@ INDI\n\
1 NAME Иван /Иванов/\n\
2 GIVN Иван\n\
2 SURN Иванов\n\
1 SEX M\n\
0 @I2@ INDI\n\
1 NAME Иван /Иванов/\n\
2 GIVN Иван\n\
2 SURN Иванов\n\
1 SEX M\n\
{SUBMITTER}"
    );
    let (_, cards) = import(&ged).expect("import should succeed");
    let ids: Vec<&str> = cards.iter().map(|card| card.id.as_str()).collect();
    assert_eq!(ids, vec!["ivan-ivanov", "ivan-ivanov-2"]);
}

/// A patronymic in `GIVN` stays part of the name — it cannot be split back off —
/// and is left out of the id, which is the first given name and the surname.
#[test]
fn a_patronymic_stays_in_the_name_and_out_of_the_id() {
    let ged = format!(
        "{HEADER}\
0 @I1@ INDI\n\
1 NAME Пётр Сергеевич /Иванов/\n\
2 GIVN Пётр Сергеевич\n\
2 SURN Иванов\n\
1 SEX M\n\
{SUBMITTER}"
    );
    let (_, cards) = import(&ged).expect("import should succeed");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].id, "pyotr-ivanov");
    assert_eq!(
        cards[0].yaml,
        "name: Пётр Сергеевич\nsurname: Иванов\nsex: M\n"
    );
}

/// tree.yaml needs a language and a submitter; a header without them is named
/// rather than compiled into a tree that will not build.
#[test]
fn a_header_without_language_or_submitter_is_reported() {
    let ged = "0 HEAD\n1 CHAR UTF-8\n\
0 @I1@ INDI\n\
1 NAME Иван /Иванов/\n\
2 GIVN Иван\n\
2 SURN Иванов\n\
1 SEX M\n\
0 TRLR\n";
    let diagnostics = import(ged).err().unwrap();
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(None, Some("language"), "no LANG in the header to import"),
            diagnostic(
                None,
                Some("submitter"),
                "no SUBM record with a NAME to import"
            ),
        ]
    );
}
