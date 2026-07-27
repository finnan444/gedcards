//! The stub card and the command that writes it. The template is a seam like
//! the others — text in, text out — but refusing to overwrite a card and making
//! `people/` are the CLI's, and have no seam to drive: these are the tests that
//! run the binary, in a directory of their own.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use gedcards::{Card, Diagnostic, compile, new_card};

const CONFIG: &str = "submitter: Иван Иванов\nlanguage: Russian\n";

/// The template as the README documents it; `gedc new` must write this.
const TEMPLATE: &str = "\
name:
surname:
sex:
# patronymic:
# married_surname:
# birth:
#   date:
#   place:
# death:
#   date:
#   place:
# father:
# mother:
# marriage:
#   spouse:
#   date:
#   place:
";

fn diagnostic(card: Option<&str>, field: Option<&str>, reason: &str) -> Diagnostic {
    Diagnostic {
        card: card.map(String::from),
        field: field.map(String::from),
        reason: reason.to_string(),
    }
}

/// The template with its three required fields filled in: the card a person has
/// a minute after `gedc new`, once the editor has been in it.
fn filled() -> String {
    new_card("ivanov-ivan")
        .unwrap()
        .replacen("name:", "name: Иван", 1)
        .replacen("surname:", "surname: Иванов", 1)
        .replacen("sex:", "sex: M", 1)
}

#[test]
fn the_template_is_the_documented_one() {
    assert_eq!(new_card("ivanov-ivan").unwrap(), TEMPLATE);
}

/// The stub is a draft, and the compiler is what says what is left to do: the
/// three empty fields and nothing else about the template is a problem.
#[test]
fn the_template_reports_exactly_its_three_empty_fields() {
    let cards = [Card {
        id: "ivanov-ivan".to_string(),
        yaml: new_card("ivanov-ivan").unwrap(),
    }];
    let reason = "required field is missing";
    assert_eq!(
        compile(CONFIG, &cards).unwrap_err(),
        vec![
            diagnostic(Some("ivanov-ivan"), Some("name"), reason),
            diagnostic(Some("ivanov-ivan"), Some("surname"), reason),
            diagnostic(Some("ivanov-ivan"), Some("sex"), reason),
        ]
    );
}

#[test]
fn the_template_compiles_once_the_three_are_filled() {
    let cards = [Card {
        id: "ivanov-ivan".to_string(),
        yaml: filled(),
    }];
    compile(CONFIG, &cards).expect("a filled-in template should compile");
}

#[test]
fn a_non_slug_id_is_refused_with_the_compilers_wording() {
    let reason = "id must be a slug of lowercase latin letters, digits and hyphens";
    for id in ["иван-иванов", "Ivanov_Ivan", "-ivanov", "ivanov--ivan", ""] {
        assert_eq!(
            new_card(id).unwrap_err(),
            diagnostic(Some(id), None, reason)
        );
    }
}

/// A fresh empty directory to run the binary in: one per test, so they can run
/// in parallel, and one per run, so two of them can too.
fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gedc-new-{}-{test}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs `gedc new <id>` in `dir` and returns its exit code, stdout and stderr.
fn gedc_new(dir: &Path, id: &str) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_gedc"))
        .args(["new", id])
        .current_dir(dir)
        .output()
        .expect("gedc should run");
    (
        output.status.code().expect("gedc should exit normally"),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

/// The first card of a tree needs no `mkdir`, and the path is printed so it can
/// be handed straight to an editor.
#[test]
fn new_writes_the_card_and_prints_its_path() {
    let dir = temp_dir("writes-the-card");
    let (code, stdout, stderr) = gedc_new(&dir, "ivanov-ivan");
    assert_eq!(
        (code, stdout.as_str()),
        (0, "people/ivanov-ivan.yaml\n"),
        "{stderr}"
    );
    assert_eq!(
        fs::read_to_string(dir.join("people/ivanov-ivan.yaml")).unwrap(),
        TEMPLATE
    );
}

#[test]
fn new_never_overwrites_an_existing_card() {
    let dir = temp_dir("never-overwrites");
    fs::create_dir(dir.join("people")).unwrap();
    let card = dir.join("people/ivanov-ivan.yaml");
    fs::write(&card, "name: Иван\nsurname: Иванов\nsex: M\n").unwrap();

    let (code, _, stderr) = gedc_new(&dir, "ivanov-ivan");
    assert_ne!(code, 0);
    assert!(stderr.contains("people/ivanov-ivan.yaml"), "{stderr}");
    assert_eq!(
        fs::read_to_string(&card).unwrap(),
        "name: Иван\nsurname: Иванов\nsex: M\n"
    );
}

#[test]
fn new_writes_nothing_when_the_id_is_not_a_slug() {
    let dir = temp_dir("bad-id");
    let (code, _, stderr) = gedc_new(&dir, "Ivanov_Ivan");
    assert_ne!(code, 0);
    assert_eq!(
        stderr,
        "error: Ivanov_Ivan: id must be a slug of lowercase latin letters, digits and hyphens\n"
    );
    assert!(!dir.join("people").exists());
}
