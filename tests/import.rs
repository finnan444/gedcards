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
/// patronymic fused into `GIVN` — read into the cards checked in beside it.
#[test]
fn myheritage_export_imports_to_the_expected_cards() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/import/myheritage");
    let ged = fs::read_to_string(dir.join("family.ged")).unwrap();

    let (tree_yaml, cards) = import(&ged, None).expect("import should succeed");

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
/// sort the same and no id appears in the output, so the bytes match. Every
/// fixture that compiles round-trips: names, dates and events, the burial with
/// its age and cause, and the families with their marriages and divorces.
#[test]
fn build_import_build_is_byte_identical() {
    for fixture in [
        "full-names",
        "three-people",
        "dates",
        "burial",
        "relationships",
        "religion",
        "remarriage",
    ] {
        let (config, cards) = load_fixture(fixture);
        let first = compile(&config, &cards).expect("first build should succeed");

        let (tree_yaml, imported) = import(&first, None).expect("import should succeed");
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
    let (_, cards) = import(&ged, None).expect("import should succeed");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].id, "ivanov-ivan");
    assert_eq!(cards[0].yaml, "name: Иван\nsurname: Иванов\nsex: M\n");
}

/// A tag with no card field is still named rather than dropped, even now that
/// events and families import: a name piece the card cannot hold, a standard
/// tag with no field, an event detail with none, and a date in a form the card
/// grammar has no room for. Every one is reported in the same run.
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
1 OCCU Blacksmith\n\
1 BIRT\n\
2 DATE BET 1900 AND 1910\n\
2 TIME 09:00\n\
{SUBMITTER}"
    );
    let diagnostics = import(&ged, None).err().unwrap();
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(Some("@I1@"), None, "NAME piece NPFX is not imported yet"),
            diagnostic(Some("@I1@"), None, "OCCU is not imported yet"),
            diagnostic(
                Some("@I1@"),
                None,
                "BIRT date \"BET 1900 AND 1910\" is not a form a card can hold"
            ),
            diagnostic(Some("@I1@"), None, "BIRT detail TIME is not imported yet"),
        ]
    );
}

/// Events and a family read back into the card fields that hold them: the birth,
/// death (with its age and cause) and burial blocks, and the parents and the
/// marriage the FAM record carries. The marriage lands on one spouse's card.
#[test]
fn events_and_relationships_import() {
    let ged = format!(
        "{HEADER}\
0 @I1@ INDI\n\
1 NAME Пётр /Иванов/\n\
2 GIVN Пётр\n\
2 SURN Иванов\n\
1 SEX M\n\
1 BIRT\n\
2 DATE 12 MAR 1947\n\
2 PLAC Тверь\n\
1 DEAT Y\n\
2 DATE 2020\n\
2 AGE 73\n\
2 CAUS Stroke\n\
1 BURI\n\
2 PLAC Москва\n\
0 @I2@ INDI\n\
1 NAME Анна /Петрова/\n\
2 GIVN Анна\n\
2 SURN Петрова\n\
1 SEX F\n\
0 @I3@ INDI\n\
1 NAME Ольга /Иванова/\n\
2 GIVN Ольга\n\
2 SURN Иванова\n\
1 SEX F\n\
0 @F1@ FAM\n\
1 MARR\n\
2 DATE 1970\n\
1 DIV Y\n\
1 HUSB @I1@\n\
1 WIFE @I2@\n\
1 CHIL @I3@\n\
{SUBMITTER}"
    );
    let (_, cards) = import(&ged, None).expect("import should succeed");
    let by_id: HashMap<&str, &str> = cards
        .iter()
        .map(|card| (card.id.as_str(), card.yaml.as_str()))
        .collect();
    assert_eq!(
        by_id["ivanov-pyotr"],
        "name: Пётр\nsurname: Иванов\nsex: M\n\
birth:\n  date: 1947-03-12\n  place: Тверь\n\
death:\n  date: 2020\n  age: 73\n  cause: Stroke\n\
burial:\n  place: Москва\n\
marriage:\n  spouse: petrova-anna\n  date: 1970\n  divorce:\n"
    );
    assert_eq!(
        by_id["ivanova-olga"],
        "name: Ольга\nsurname: Иванова\nsex: F\nfather: ivanov-pyotr\nmother: petrova-anna\n"
    );
}

/// The religious events and the `RELI` affiliation read back into the card the
/// same way the birth block does: `CHR`/`BAPM`/`CONF`/`FCOM` into their event
/// blocks, in the field order the compiler emits them, and `RELI` into a bare
/// `religion` line after the events.
#[test]
fn religious_events_and_affiliation_import() {
    let ged = format!(
        "{HEADER}\
0 @I1@ INDI\n\
1 NAME Борис /Орлов/\n\
2 GIVN Борис\n\
2 SURN Орлов\n\
1 SEX M\n\
1 BIRT\n\
2 DATE 2 MAY 1899\n\
1 CHR\n\
2 DATE 10 MAY 1899\n\
2 PLAC Москва\n\
1 BAPM\n\
2 DATE 10 MAY 1899\n\
1 CONF\n\
2 DATE 1913\n\
1 FCOM\n\
2 DATE 1911\n\
1 RELI Православие\n\
{SUBMITTER}"
    );
    let (_, cards) = import(&ged, None).expect("import should succeed");
    assert_eq!(cards.len(), 1);
    assert_eq!(
        cards[0].yaml,
        "name: Борис\nsurname: Орлов\nsex: M\n\
birth:\n  date: 1899-05-02\n\
christening:\n  date: 1899-05-10\n  place: Москва\n\
baptism:\n  date: 1899-05-10\n\
confirmation:\n  date: 1913\n\
first_communion:\n  date: 1911\n\
religion: Православие\n"
    );
}

/// A burial's coordinates and note read back into the card: `MAP` nested under
/// `PLAC` becomes the `coords` pair, the hemisphere letters turning into signs
/// (S and W negative), and `NOTE` becomes the free line.
#[test]
fn burial_coordinates_and_note_import() {
    let ged = format!(
        "{HEADER}\
0 @I1@ INDI\n\
1 NAME Роальд /Амундсен/\n\
2 GIVN Роальд\n\
2 SURN Амундсен\n\
1 SEX M\n\
1 BURI\n\
2 PLAC Ушуайя\n\
3 MAP\n\
4 LATI S54.8\n\
4 LONG W68.3\n\
2 NOTE у флагштока\n\
{SUBMITTER}"
    );
    let (_, cards) = import(&ged, None).expect("import should succeed");
    assert_eq!(cards.len(), 1);
    assert_eq!(
        cards[0].yaml,
        "name: Роальд\nsurname: Амундсен\nsex: M\n\
burial:\n  place: Ушуайя\n  coords: -54.8, -68.3\n  note: у флагштока\n"
    );
}

/// A `NOTE` on the `INDI` itself — not on an event — reads back into the card's
/// own `note` field, written last, after the events and any relationships.
#[test]
fn a_note_on_the_person_imports() {
    let ged = format!(
        "{HEADER}\
0 @I1@ INDI\n\
1 NAME Александра /Волкова/\n\
2 GIVN Александра\n\
2 SURN Волкова\n\
1 SEX F\n\
1 BIRT\n\
2 DATE 1918\n\
1 NOTE любимая бабушка, всегда звали бабушкой Шурой\n\
{SUBMITTER}"
    );
    let (_, cards) = import(&ged, None).expect("import should succeed");
    assert_eq!(cards.len(), 1);
    assert_eq!(
        cards[0].yaml,
        "name: Александра\nsurname: Волкова\nsex: F\n\
birth:\n  date: 1918\n\
note: любимая бабушка, всегда звали бабушкой Шурой\n"
    );
}

/// GEDCOM requires both a LATI and a LONG under a MAP; a half-written pin cannot
/// become a card's coords, so it is named rather than dropped.
#[test]
fn a_map_missing_a_coordinate_is_reported() {
    let ged = format!(
        "{HEADER}\
0 @I1@ INDI\n\
1 NAME Иван /Иванов/\n\
2 GIVN Иван\n\
2 SURN Иванов\n\
1 SEX M\n\
1 BURI\n\
2 PLAC Тверь\n\
3 MAP\n\
4 LATI N56.86\n\
{SUBMITTER}"
    );
    let diagnostics = import(&ged, None).err().unwrap();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("@I1@"),
            None,
            "BURI MAP needs both a LATI and a LONG to import"
        )]
    );
}

/// Two people who share a name and a birth year both keep it: the first takes the
/// bare slug, the namesake the birth-year suffix the README's ids use.
#[test]
fn a_namesake_takes_the_birth_year_suffix() {
    let ged = format!(
        "{HEADER}\
0 @I1@ INDI\n\
1 NAME Иван /Иванов/\n\
2 GIVN Иван\n\
2 SURN Иванов\n\
1 SEX M\n\
1 BIRT\n\
2 DATE 1910\n\
0 @I2@ INDI\n\
1 NAME Иван /Иванов/\n\
2 GIVN Иван\n\
2 SURN Иванов\n\
1 SEX M\n\
1 BIRT\n\
2 DATE 3 AUG 1947\n\
{SUBMITTER}"
    );
    let (_, cards) = import(&ged, None).expect("import should succeed");
    let mut ids: Vec<&str> = cards.iter().map(|card| card.id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["ivanov-ivan", "ivanov-ivan-1947"]);
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
    let (tree_yaml, cards) = import(&ged, None).expect("import should succeed");
    assert_eq!(tree_yaml, "submitter: Иван Иванов\nlanguage: Russian\n");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].id, "ivanov-ivan");
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
    let (_, cards) = import(&ged, None).expect("import should succeed");
    let ids: Vec<&str> = cards.iter().map(|card| card.id.as_str()).collect();
    assert_eq!(ids, vec!["ivanov-ivan", "ivanov-ivan-2"]);
}

/// A patronymic in `GIVN` stays part of the name — it cannot be split back off —
/// and is left out of the id, which is the surname and the first given name.
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
    let (_, cards) = import(&ged, None).expect("import should succeed");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].id, "ivanov-pyotr");
    assert_eq!(
        cards[0].yaml,
        "name: Пётр Сергеевич\nsurname: Иванов\nsex: M\n"
    );
}

/// A `NAME` the file types as the married one carries the surname taken at
/// marriage, not a name at birth: its `SURN` comes back as `married_surname` and
/// the card is left without a `surname`, the shape `gedc build` writes for a
/// person whose birth surname was never learned. The id falls back to the same
/// surname. 5.5.1 spells the type in lower case and 7.0 in upper; both read.
#[test]
fn a_name_typed_married_imports_as_the_married_surname() {
    for spelling in ["married", "MARRIED"] {
        let ged = format!(
            "{HEADER}\
0 @I1@ INDI\n\
1 NAME Мария /Иванова/\n\
2 TYPE {spelling}\n\
2 GIVN Мария\n\
2 SURN Иванова\n\
1 SEX F\n\
{SUBMITTER}"
        );
        let (_, cards) = import(&ged, None).expect("import should succeed");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].id, "ivanova-mariya");
        assert_eq!(
            cards[0].yaml,
            "name: Мария\nmarried_surname: Иванова\nsex: F\n"
        );
    }
}

/// Every other name type is a distinction the card cannot hold, so the `NAME` is
/// named rather than read as a name at birth it may not be.
#[test]
fn a_name_of_another_type_is_reported() {
    let ged = format!(
        "{HEADER}\
0 @I1@ INDI\n\
1 NAME Иван /Иванов/\n\
2 TYPE immigrant\n\
2 GIVN Иван\n\
2 SURN Иванов\n\
1 SEX M\n\
{SUBMITTER}"
    );
    let diagnostics = import(&ged, None).err().unwrap();
    assert_eq!(
        diagnostics,
        vec![diagnostic(
            Some("@I1@"),
            None,
            "NAME of type immigrant is not imported yet"
        )]
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
    let diagnostics = import(ged, None).err().unwrap();
    assert_eq!(
        diagnostics,
        vec![
            diagnostic(None, Some("language"), "no LANG in the header to import"),
            diagnostic(
                None,
                Some("submitter"),
                "no SUBM record with a NAME to import, and no --submitter given"
            ),
        ]
    );
}

/// A supplied submitter stands in for one the file lacks — the common case for an
/// export — and wins over one it carries, since it names the tree's new owner.
#[test]
fn a_supplied_submitter_fills_in_and_overrides() {
    // No SUBM record at all: the flag is the only submitter there is.
    let ged = "0 HEAD\n1 CHAR UTF-8\n1 LANG Russian\n\
0 @I1@ INDI\n\
1 NAME Иван /Иванов/\n\
2 GIVN Иван\n\
2 SURN Иванов\n\
1 SEX M\n\
0 TRLR\n";
    let (tree_yaml, cards) = import(ged, Some("Пётр Рыковский")).expect("import should succeed");
    assert_eq!(tree_yaml, "submitter: Пётр Рыковский\nlanguage: Russian\n");
    assert_eq!(cards.len(), 1);

    // A file with its own SUBM: the flag still wins.
    let with_subm = format!(
        "{HEADER}\
0 @I1@ INDI\n\
1 NAME Иван /Иванов/\n\
2 GIVN Иван\n\
2 SURN Иванов\n\
1 SEX M\n\
{SUBMITTER}"
    );
    let (tree_yaml, _) = import(&with_subm, Some("Пётр Рыковский")).expect("import should succeed");
    assert_eq!(tree_yaml, "submitter: Пётр Рыковский\nlanguage: Russian\n");
}
