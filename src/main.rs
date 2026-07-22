use std::fs;
use std::path::Path;
use std::process::ExitCode;

use gedcards::Card;

const USAGE: &str = "usage: gedc <build|schema>\n\nbuild   reads people/*.yaml and tree.yaml from the current directory\n        and writes family.ged (GEDCOM 5.5.1, UTF-8).\nschema  prints a JSON Schema for the cards in people/, so an editor can\n        complete and check the ids a card names:\n        gedc schema > .vscode/people.schema.json";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command] if command == "build" => build(),
        [command] if command == "schema" => schema(),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn build() -> ExitCode {
    let config_yaml = match fs::read_to_string("tree.yaml") {
        Ok(text) => text,
        Err(err) => {
            eprintln!("error: cannot read tree.yaml: {err}");
            return ExitCode::FAILURE;
        }
    };
    let cards = match read_cards(Path::new("people")) {
        Ok(cards) => cards,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    };

    match gedcards::compile(&config_yaml, &cards) {
        Ok(ged) => {
            if let Err(err) = fs::write("family.ged", &ged) {
                eprintln!("error: cannot write family.ged: {err}");
                return ExitCode::FAILURE;
            }
            println!("family.ged written ({} people)", cards.len());
            ExitCode::SUCCESS
        }
        Err(diagnostics) => {
            for diagnostic in &diagnostics {
                eprintln!("error: {diagnostic}");
            }
            eprintln!(
                "{} problem(s) found, family.ged not written",
                diagnostics.len()
            );
            ExitCode::FAILURE
        }
    }
}

/// Writes the schema to stdout rather than to a file: where it belongs is the
/// caller's business, since it is the editor's `yaml.schemas` that has to find
/// it. No tree.yaml is read — the cards are all a schema is made of.
fn schema() -> ExitCode {
    let cards = match read_cards(Path::new("people")) {
        Ok(cards) => cards,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    };
    print!("{}", gedcards::schema(&cards));
    ExitCode::SUCCESS
}

/// Reads every .yaml file in `dir` as a card, sorted by file name
/// so diagnostics come out in a stable order.
fn read_cards(dir: &Path) -> Result<Vec<Card>, String> {
    let entries = fs::read_dir(dir).map_err(|err| format!("cannot read people/: {err}"))?;
    let mut cards = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|err| format!("cannot read people/: {err}"))?
            .path();
        let is_yaml = path.extension().and_then(|ext| ext.to_str()) == Some("yaml");
        if !path.is_file() || !is_yaml {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("card file name is not valid UTF-8: {}", path.display()))?
            .to_string();
        let yaml = fs::read_to_string(&path)
            .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        cards.push(Card { id, yaml });
    }
    cards.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(cards)
}
