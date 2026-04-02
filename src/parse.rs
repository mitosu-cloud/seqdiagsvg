use pest::Parser;
use pest_derive::Parser;

use crate::ast::*;
use crate::SeqDiagramError;

#[derive(Parser)]
#[grammar = "grammar.pest"]
struct SeqDiagramParser;

/// Parse a sequence diagram input string into a Document AST.
pub fn parse_document(input: &str) -> Result<Document, SeqDiagramError> {
    let pairs = SeqDiagramParser::parse(Rule::document, input)
        .map_err(|e| SeqDiagramError::Parse(e.to_string()))?;

    let mut statements = Vec::new();

    // The top-level parse result contains a single `document` pair
    // which in turn contains the actual statement pairs.
    let document_pair = pairs.into_iter().next().unwrap();
    for pair in document_pair.into_inner() {
        match pair.as_rule() {
            Rule::title_stmt => {
                let text = pair
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::message_text)
                    .map(|p| p.as_str().trim().to_string())
                    .unwrap_or_default();
                statements.push(Statement::Title(text));
            }
            Rule::participant_stmt => {
                let rest = pair
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::participant_rest)
                    .expect("participant must have rest")
                    .as_str()
                    .trim();
                // Split on " as " (case-insensitive) to get actor and optional alias
                let (actor, alias) = if let Some(pos) = rest
                    .to_lowercase()
                    .find(" as ")
                {
                    let actor = rest[..pos].trim().to_string();
                    let alias = rest[pos + 4..].trim().to_string();
                    (actor, Some(alias))
                } else {
                    (rest.to_string(), None)
                };
                statements.push(Statement::Participant { actor, alias });
            }
            Rule::note_stmt => {
                let mut inner = pair.into_inner();
                let pos_pair = inner.next().expect("note must have position");
                let position = parse_note_position(pos_pair);
                let text = inner
                    .find(|p| p.as_rule() == Rule::message_text)
                    .map(|p| p.as_str().trim().to_string())
                    .unwrap_or_default();
                statements.push(Statement::Note { position, text });
            }
            Rule::message_stmt => {
                let mut inner = pair.into_inner();
                let from = inner
                    .next()
                    .expect("message must have from actor")
                    .as_str()
                    .trim()
                    .to_string();
                let arrow_pair = inner.next().expect("message must have arrow");
                let arrow = parse_arrow(arrow_pair.as_str());
                let to = inner
                    .next()
                    .expect("message must have to actor")
                    .as_str()
                    .trim()
                    .to_string();
                let text = inner
                    .find(|p| p.as_rule() == Rule::message_text)
                    .map(|p| p.as_str().trim().to_string())
                    .unwrap_or_default();
                statements.push(Statement::Message {
                    from,
                    to,
                    arrow,
                    text,
                });
            }
            Rule::EOI => {}
            _ => {}
        }
    }

    Ok(Document { statements })
}

fn parse_note_position(pair: pest::iterators::Pair<Rule>) -> NotePosition {
    match pair.as_rule() {
        Rule::note_left_of => {
            let actor = pair
                .into_inner()
                .next()
                .expect("left of must have actor")
                .as_str()
                .trim()
                .to_string();
            NotePosition::LeftOf(actor)
        }
        Rule::note_right_of => {
            let actor = pair
                .into_inner()
                .next()
                .expect("right of must have actor")
                .as_str()
                .trim()
                .to_string();
            NotePosition::RightOf(actor)
        }
        Rule::note_over => {
            let mut inner = pair.into_inner();
            let first = inner
                .next()
                .expect("over must have actor")
                .as_str()
                .trim()
                .to_string();
            match inner.next() {
                Some(second) => {
                    NotePosition::OverBetween(first, second.as_str().trim().to_string())
                }
                None => NotePosition::Over(first),
            }
        }
        _ => unreachable!("unexpected note position rule: {:?}", pair.as_rule()),
    }
}

fn parse_arrow(s: &str) -> Arrow {
    match s {
        "->>" => Arrow {
            line_style: LineStyle::Solid,
            head_style: HeadStyle::Closed,
        },
        "->" => Arrow {
            line_style: LineStyle::Solid,
            head_style: HeadStyle::Open,
        },
        "-->>" => Arrow {
            line_style: LineStyle::Dashed,
            head_style: HeadStyle::Closed,
        },
        "-->" => Arrow {
            line_style: LineStyle::Dashed,
            head_style: HeadStyle::Open,
        },
        _ => Arrow {
            line_style: LineStyle::Solid,
            head_style: HeadStyle::Open,
        },
    }
}

/// Extract the ordered list of unique actors from a document.
/// Participant declarations come first (in order), then any remaining actors
/// from messages in order of first appearance.
pub fn resolve_actors(doc: &Document) -> Vec<(String, String)> {
    let mut actors: Vec<(String, String)> = Vec::new(); // (name, display_name)
    let mut seen = std::collections::HashSet::new();

    // First pass: participant declarations define explicit ordering
    for stmt in &doc.statements {
        if let Statement::Participant { actor, alias } = stmt {
            if seen.insert(actor.clone()) {
                let display = alias.as_deref().unwrap_or(actor).to_string();
                actors.push((actor.clone(), display));
            }
        }
    }

    // Second pass: remaining actors from messages in order of first appearance
    for stmt in &doc.statements {
        if let Statement::Message { from, to, .. } = stmt {
            if seen.insert(from.clone()) {
                actors.push((from.clone(), from.clone()));
            }
            if seen.insert(to.clone()) {
                actors.push((to.clone(), to.clone()));
            }
        }
    }

    actors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_message() {
        let doc = parse_document("Alice->Bob: Hello").unwrap();
        assert_eq!(doc.statements.len(), 1);
        match &doc.statements[0] {
            Statement::Message {
                from,
                to,
                arrow,
                text,
            } => {
                assert_eq!(from, "Alice");
                assert_eq!(to, "Bob");
                assert_eq!(arrow.line_style, LineStyle::Solid);
                assert_eq!(arrow.head_style, HeadStyle::Open);
                assert_eq!(text, "Hello");
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn test_all_arrow_types() {
        let input = "A->B: solid open\nA->>B: solid closed\nA-->B: dashed open\nA-->>B: dashed closed";
        let doc = parse_document(input).unwrap();
        assert_eq!(doc.statements.len(), 4);

        let arrows: Vec<Arrow> = doc
            .statements
            .iter()
            .map(|s| match s {
                Statement::Message { arrow, .. } => *arrow,
                _ => panic!("expected Message"),
            })
            .collect();

        assert_eq!(
            arrows[0],
            Arrow {
                line_style: LineStyle::Solid,
                head_style: HeadStyle::Open
            }
        );
        assert_eq!(
            arrows[1],
            Arrow {
                line_style: LineStyle::Solid,
                head_style: HeadStyle::Closed
            }
        );
        assert_eq!(
            arrows[2],
            Arrow {
                line_style: LineStyle::Dashed,
                head_style: HeadStyle::Open
            }
        );
        assert_eq!(
            arrows[3],
            Arrow {
                line_style: LineStyle::Dashed,
                head_style: HeadStyle::Closed
            }
        );
    }

    #[test]
    fn test_title() {
        let doc = parse_document("title: My Diagram").unwrap();
        assert_eq!(doc.statements.len(), 1);
        match &doc.statements[0] {
            Statement::Title(t) => assert_eq!(t, "My Diagram"),
            _ => panic!("expected Title"),
        }
    }

    #[test]
    fn test_title_without_colon() {
        let doc = parse_document("title My Diagram").unwrap();
        match &doc.statements[0] {
            Statement::Title(t) => assert_eq!(t, "My Diagram"),
            _ => panic!("expected Title"),
        }
    }

    #[test]
    fn test_participant_with_alias() {
        let doc = parse_document("participant Alice as A").unwrap();
        match &doc.statements[0] {
            Statement::Participant { actor, alias } => {
                assert_eq!(actor, "Alice");
                assert_eq!(alias.as_deref(), Some("A"));
            }
            _ => panic!("expected Participant"),
        }
    }

    #[test]
    fn test_participant_without_alias() {
        let doc = parse_document("participant Bob").unwrap();
        match &doc.statements[0] {
            Statement::Participant { actor, alias } => {
                assert_eq!(actor, "Bob");
                assert!(alias.is_none());
            }
            _ => panic!("expected Participant"),
        }
    }

    #[test]
    fn test_note_left_of() {
        let doc = parse_document("note left of Alice: Important").unwrap();
        match &doc.statements[0] {
            Statement::Note { position, text } => {
                assert_eq!(position, &NotePosition::LeftOf("Alice".into()));
                assert_eq!(text, "Important");
            }
            _ => panic!("expected Note"),
        }
    }

    #[test]
    fn test_note_right_of() {
        let doc = parse_document("note right of Bob: Check this").unwrap();
        match &doc.statements[0] {
            Statement::Note { position, text } => {
                assert_eq!(position, &NotePosition::RightOf("Bob".into()));
                assert_eq!(text, "Check this");
            }
            _ => panic!("expected Note"),
        }
    }

    #[test]
    fn test_note_over_single() {
        let doc = parse_document("note over Alice: Thinking").unwrap();
        match &doc.statements[0] {
            Statement::Note { position, text } => {
                assert_eq!(position, &NotePosition::Over("Alice".into()));
                assert_eq!(text, "Thinking");
            }
            _ => panic!("expected Note"),
        }
    }

    #[test]
    fn test_note_over_two_actors() {
        let doc = parse_document("note over Alice, Bob: Shared state").unwrap();
        match &doc.statements[0] {
            Statement::Note { position, text } => {
                assert_eq!(
                    position,
                    &NotePosition::OverBetween("Alice".into(), "Bob".into())
                );
                assert_eq!(text, "Shared state");
            }
            _ => panic!("expected Note"),
        }
    }

    #[test]
    fn test_resolve_actors_from_messages() {
        let doc = parse_document("Alice->Bob: hi\nBob->Charlie: hey").unwrap();
        let actors = resolve_actors(&doc);
        assert_eq!(actors.len(), 3);
        assert_eq!(actors[0].0, "Alice");
        assert_eq!(actors[1].0, "Bob");
        assert_eq!(actors[2].0, "Charlie");
    }

    #[test]
    fn test_resolve_actors_participant_first() {
        let input = "participant Bob\nparticipant Alice\nAlice->Bob: hi\nBob->Charlie: hey";
        let doc = parse_document(input).unwrap();
        let actors = resolve_actors(&doc);
        assert_eq!(actors.len(), 3);
        assert_eq!(actors[0].0, "Bob"); // participant declared first
        assert_eq!(actors[1].0, "Alice"); // participant declared second
        assert_eq!(actors[2].0, "Charlie"); // from messages
    }

    #[test]
    fn test_multiline_document() {
        let input = "\
title: Auth Flow
participant Client
participant Server

Client->Server: POST /login
Server-->Client: 200 OK";
        let doc = parse_document(input).unwrap();
        assert_eq!(doc.statements.len(), 5);
    }

    #[test]
    fn test_blank_lines_ignored() {
        let input = "Alice->Bob: hi\n\n\nBob->Alice: hey";
        let doc = parse_document(input).unwrap();
        assert_eq!(doc.statements.len(), 2);
    }

    #[test]
    fn test_comment_lines() {
        let input = "# This is a comment\nAlice->Bob: hi\n# Another comment";
        let doc = parse_document(input).unwrap();
        assert_eq!(doc.statements.len(), 1);
    }
}
