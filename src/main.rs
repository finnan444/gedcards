use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use gedcards::Card;

const USAGE: &str = "usage: gedc <build|new|schema|import>\n\nbuild        reads people/*.yaml and tree.yaml from the current directory\n             and writes family.ged (GEDCOM 5.5.1, UTF-8).\nnew ID       writes people/ID.yaml with the card's fields in it, empty, and\n             prints the path. An existing card is never overwritten.\nschema       prints a JSON Schema for the cards in people/, so an editor can\n             complete and check the ids a card names:\n             gedc schema > .vscode/people.schema.json\nimport [--submitter NAME] FILE\n             reads a GEDCOM 5.5.1 file and writes tree.yaml and one card per\n             person into the current directory. --submitter sets tree.yaml's\n             submitter, for a file whose SUBM an export left out.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command] if command == "build" => build(),
        [command] if command == "schema" => schema(),
        [command, id] if command == "new" => new(id),
        [command, rest @ ..] if command == "import" => import(rest),
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

/// Writes a stub card for `id` into people/, and prints the path — just the
/// path, so it can be handed straight to an editor.
///
/// The id is checked before anything is written, so a bad one is an error
/// rather than a file to rename. `create_new` is what refuses an existing
/// card: asking whether the file is there and then writing it are two
/// answers to one question, and only the write's is current.
fn new(id: &str) -> ExitCode {
    let template = match gedcards::new_card(id) {
        Ok(template) => template,
        Err(diagnostic) => {
            eprintln!("error: {diagnostic}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(err) = fs::create_dir_all("people") {
        eprintln!("error: cannot create people/: {err}");
        return ExitCode::FAILURE;
    }
    let path = Path::new("people").join(format!("{id}.yaml"));
    let written =
        fs::File::create_new(&path).and_then(|mut file| file.write_all(template.as_bytes()));
    match written {
        Ok(()) => {
            println!("{}", path.display());
            ExitCode::SUCCESS
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            eprintln!("error: {} already exists", path.display());
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("error: cannot write {}: {err}", path.display());
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

/// Reads a GEDCOM file and writes a tree: tree.yaml plus one card per person
/// in people/. Import bootstraps a new tree, so it refuses to run where a
/// tree.yaml already sits rather than write over cards it did not author.
///
/// The arguments are the file to read and an optional `--submitter NAME` that
/// sets tree.yaml's submitter — all the parsing the import command does, kept
/// here at the entry point rather than in the library.
fn import(args: &[String]) -> ExitCode {
    let mut file: Option<&str> = None;
    let mut submitter: Option<&str> = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--submitter" => match rest.next() {
                Some(name) => submitter = Some(name),
                None => {
                    eprintln!("{USAGE}");
                    return ExitCode::from(2);
                }
            },
            path if file.is_none() && !path.starts_with("--") => file = Some(path),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(file) = file else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let path = Path::new(file);

    let ged = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("error: cannot read {}: {err}", path.display());
            return ExitCode::FAILURE;
        }
    };
    if Path::new("tree.yaml").exists() {
        eprintln!("error: tree.yaml already exists here, refusing to overwrite it");
        return ExitCode::FAILURE;
    }

    let (tree_yaml, cards) = match gedcards::import(&ged, submitter) {
        Ok(imported) => imported,
        Err(diagnostics) => {
            for diagnostic in &diagnostics {
                eprintln!("error: {diagnostic}");
            }
            eprintln!("{} problem(s) found, nothing written", diagnostics.len());
            return ExitCode::FAILURE;
        }
    };

    if let Err(err) = fs::create_dir_all("people") {
        eprintln!("error: cannot create people/: {err}");
        return ExitCode::FAILURE;
    }
    for card in &cards {
        let path = Path::new("people").join(format!("{}.yaml", card.id));
        if let Err(err) = fs::write(&path, &card.yaml) {
            eprintln!("error: cannot write {}: {err}", path.display());
            return ExitCode::FAILURE;
        }
    }
    if let Err(err) = fs::write("tree.yaml", &tree_yaml) {
        eprintln!("error: cannot write tree.yaml: {err}");
        return ExitCode::FAILURE;
    }
    println!("tree.yaml and {} card(s) written", cards.len());
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
