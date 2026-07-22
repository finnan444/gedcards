//! The JSON Schema for the cards of a tree. JSON Schema cannot look at the
//! card directory, so the ids are baked into it: the editor gets the answer
//! the compiler would only give at build time.

use std::collections::BTreeSet;

use crate::{Card, parse_mapping, take_string};

/// Everything up to `father`. `birth` and `death` are the same event block,
/// so both point at the one definition rather than repeating it.
const PROLOGUE: &str = r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "gedcards person card",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "name",
    "surname",
    "sex"
  ],
  "properties": {
    "name": {
      "type": "string"
    },
    "patronymic": {
      "type": "string"
    },
    "surname": {
      "type": "string"
    },
    "married_surname": {
      "type": "string"
    },
    "sex": {
      "enum": [
        "M",
        "F"
      ]
    },
    "birth": {
      "$ref": "#/definitions/event"
    },
    "death": {
      "$ref": "#/definitions/event"
    },
"##;

/// The marriage block, down to its `spouse`.
const MARRIAGE: &str = r##"    "marriage": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "spouse"
      ],
      "properties": {
"##;

/// The rest of the marriage block, and the definitions the event and date
/// fields point at.
///
/// The date pattern is `parse_date`'s grammar as a regex: a `~`, `<` or `>`
/// marker, a year, then an optional month and day. The year is spelled as an
/// alternation of "some digit is not zero" rather than four digits and a
/// negative lookahead, because lookahead is not part of the regex dialect
/// every validator implements. A bare year is accepted as an integer too,
/// since that is what YAML reads `1995` as — of four digits, because an
/// integer reaches the compiler through `to_string`, where `0001` is `1`.
///
/// An event block carries a date, a place, or both — `minProperties` says
/// "at least one of them", the two being all this block may hold.
const EPILOGUE: &str = r##"        "date": {
          "$ref": "#/definitions/date"
        },
        "place": {
          "type": "string"
        }
      }
    }
  },
  "definitions": {
    "date": {
      "anyOf": [
        {
          "type": "string",
          "pattern": "^[~<>]?([0-9]{3}[1-9]|[0-9]{2}[1-9][0-9]|[0-9][1-9][0-9]{2}|[1-9][0-9]{3})(-(0[1-9]|1[0-2])(-(0[1-9]|[12][0-9]|3[01]))?)?$"
        },
        {
          "type": "integer",
          "minimum": 1000,
          "maximum": 9999
        }
      ]
    },
    "event": {
      "type": "object",
      "additionalProperties": false,
      "minProperties": 1,
      "properties": {
        "date": {
          "$ref": "#/definitions/date"
        },
        "place": {
          "type": "string"
        }
      }
    }
  }
}
"##;

/// What an id looks like, for a tree that has no card to name — the same shape
/// `is_slug` accepts.
const ID_PATTERN: &str = "^[a-z0-9]+(-[a-z0-9]+)*$";

/// The JSON Schema for the cards of a tree, as draft-07. Output is
/// byte-for-byte deterministic for the same cards, whatever order they come
/// in: the ids are sorted.
pub fn schema(cards: &[Card]) -> String {
    // Every card counts as a spouse, even one whose body is broken — the same
    // rule `compile` follows: the file exists, so the reference is not the
    // problem. Only a card that says which sex it is can be a parent.
    let mut everyone = BTreeSet::new();
    let mut men = BTreeSet::new();
    let mut women = BTreeSet::new();
    for card in cards {
        everyone.insert(card.id.as_str());
        match sex(card).as_deref() {
            Some("M") => men.insert(card.id.as_str()),
            Some("F") => women.insert(card.id.as_str()),
            _ => false,
        };
    }

    let mut json = String::from(PROLOGUE);
    json.push_str(&reference("father", &men, 4));
    json.push_str(&reference("mother", &women, 4));
    json.push_str(MARRIAGE);
    json.push_str(&reference("spouse", &everyone, 8));
    json.push_str(EPILOGUE);
    json
}

/// A card's sex, when the card states one. It is read by the parser the
/// compiler uses, and the diagnostics are dropped: a card too broken to say
/// whether it is `M` or `F` simply cannot be named as a parent.
fn sex(card: &Card) -> Option<String> {
    let mut ignored = Vec::new();
    let mut mapping = parse_mapping(&card.yaml, Some(&card.id), &mut ignored)?;
    let sex = take_string(&mut mapping, "sex", Some(&card.id), &mut ignored)?;
    (sex == "M" || sex == "F").then_some(sex)
}

/// One of the three fields naming another card, as a property holding the ids
/// it may take. A tree with nobody of that sex gets the shape of an id
/// instead: an empty `enum` matches nothing, which would make the field
/// unusable rather than merely unchecked.
///
/// The property always ends with a comma — `father`, `mother` and
/// `marriage.spouse` are each followed by another one.
fn reference(field: &str, ids: &BTreeSet<&str>, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let mut property = format!("{pad}\"{field}\": {{\n");
    if ids.is_empty() {
        property.push_str(&format!("{pad}  \"type\": \"string\",\n"));
        property.push_str(&format!("{pad}  \"pattern\": \"{ID_PATTERN}\"\n"));
    } else {
        property.push_str(&format!("{pad}  \"enum\": [\n"));
        let ids: Vec<String> = ids
            .iter()
            .map(|id| format!("{pad}    {}", quote(id)))
            .collect();
        property.push_str(&ids.join(",\n"));
        property.push_str(&format!("\n{pad}  ]\n"));
    }
    property.push_str(&format!("{pad}}},\n"));
    property
}

/// A string as JSON spells it. An id is a file name, so it can hold anything a
/// file name can: a quote in one is a compile error, but the schema still has
/// to name the card without breaking the JSON around it.
fn quote(text: &str) -> String {
    let mut quoted = String::from('"');
    for character in text.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            control if control < ' ' => {
                quoted.push_str(&format!("\\u{:04x}", control as u32));
            }
            character => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}
