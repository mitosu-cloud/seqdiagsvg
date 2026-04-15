use pest::Parser;
use pest_derive::Parser;

use crate::ast::*;
use crate::SeqDiagramError;

#[derive(Parser)]
#[grammar = "grammar.pest"]
struct SeqDiagramParser;

/// Unescape `\n` sequences in text to actual newlines.
fn unescape_text(s: &str) -> String {
    s.replace("\\n", "\n")
}

/// Parse a sequence diagram input string into a Document AST.
pub fn parse_document(input: &str) -> Result<Document, SeqDiagramError> {
    let pairs = SeqDiagramParser::parse(Rule::document, input)
        .map_err(|e| SeqDiagramError::Parse(e.to_string()))?;

    let mut statements = Vec::new();

    let document_pair = pairs.into_iter().next().unwrap();
    for pair in document_pair.into_inner() {
        match pair.as_rule() {
            Rule::title_stmt => {
                let text = pair
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::message_text)
                    .map(|p| unescape_text(p.as_str().trim()))
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
                let (actor, alias) = if let Some(pos) = rest.to_lowercase().find(" as ") {
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
                    .map(|p| unescape_text(p.as_str().trim()))
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

                // Scan remaining pairs for optional delay, activation, target actor, and text
                let mut delay = None;
                let mut activation = None;
                let mut to = String::new();
                let mut text = String::new();
                for p in inner {
                    match p.as_rule() {
                        Rule::delay_factor => {
                            if let Some(val_pair) = p.into_inner().find(|c| c.as_rule() == Rule::delay_value) {
                                delay = val_pair.as_str().parse::<u8>().ok();
                            }
                        }
                        Rule::activation_modifier => {
                            activation = match p.as_str() {
                                "+" => Some(ActivationModifier::Activate),
                                "-" => Some(ActivationModifier::Deactivate),
                                _ => None,
                            };
                        }
                        Rule::actor_name => {
                            to = p.as_str().trim().to_string();
                        }
                        Rule::message_text => {
                            text = unescape_text(p.as_str().trim());
                        }
                        _ => {}
                    }
                }
                statements.push(Statement::Message {
                    from,
                    to,
                    arrow,
                    text,
                    activation,
                    delay,
                });
            }
            Rule::activate_stmt => {
                let actor = pair
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::actor_name)
                    .expect("activate must have actor")
                    .as_str()
                    .trim()
                    .to_string();
                statements.push(Statement::Activate(actor));
            }
            Rule::deactivate_stmt => {
                let actor = pair
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::actor_name)
                    .expect("deactivate must have actor")
                    .as_str()
                    .trim()
                    .to_string();
                statements.push(Statement::Deactivate(actor));
            }
            Rule::destroy_stmt => {
                let actor = pair
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::actor_name)
                    .expect("destroy must have actor")
                    .as_str()
                    .trim()
                    .to_string();
                statements.push(Statement::Destroy(actor));
            }
            Rule::frame_open_stmt => {
                let mut inner = pair.into_inner();
                let keyword = inner
                    .next()
                    .expect("frame must have keyword")
                    .as_str();
                let kind = parse_frame_kind(keyword);
                let label = inner
                    .find(|p| p.as_rule() == Rule::frame_label)
                    .map(|p| p.as_str().trim().to_string())
                    .unwrap_or_default();
                statements.push(Statement::FrameOpen { kind, label });
            }
            Rule::frame_else_stmt => {
                let label = pair
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::frame_label)
                    .map(|p| p.as_str().trim().to_string())
                    .unwrap_or_default();
                statements.push(Statement::FrameElse { label });
            }
            Rule::end_stmt => {
                statements.push(Statement::FrameEnd);
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

fn parse_frame_kind(s: &str) -> FrameKind {
    match s.to_lowercase().as_str() {
        "alt" => FrameKind::Alt,
        "opt" => FrameKind::Opt,
        "loop" => FrameKind::Loop,
        "par" => FrameKind::Par,
        "critical" => FrameKind::Critical,
        "break" => FrameKind::Break,
        _ => FrameKind::Opt,
    }
}

/// Extract the ordered list of unique actors from a document.
/// Returns (reference_name, display_name) pairs.
pub fn resolve_actors(doc: &Document) -> Vec<(String, String)> {
    let mut actors: Vec<(String, String)> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // First pass: participant declarations define explicit ordering
    for stmt in &doc.statements {
        if let Statement::Participant { actor, alias } = stmt {
            let (ref_name, display) = match alias {
                Some(a) => (a.clone(), actor.clone()),
                None => (actor.clone(), actor.clone()),
            };
            if seen.insert(ref_name.clone()) {
                if alias.is_some() {
                    seen.insert(actor.clone());
                }
                actors.push((ref_name, display));
            }
        }
    }

    // Second pass: remaining actors from messages + activate/deactivate/destroy
    for stmt in &doc.statements {
        match stmt {
            Statement::Message { from, to, .. } => {
                if seen.insert(from.clone()) {
                    actors.push((from.clone(), from.clone()));
                }
                if seen.insert(to.clone()) {
                    actors.push((to.clone(), to.clone()));
                }
            }
            Statement::Activate(actor)
            | Statement::Deactivate(actor)
            | Statement::Destroy(actor) => {
                if seen.insert(actor.clone()) {
                    actors.push((actor.clone(), actor.clone()));
                }
            }
            _ => {}
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
                from, to, arrow, text, activation, delay,
            } => {
                assert_eq!(from, "Alice");
                assert_eq!(to, "Bob");
                assert_eq!(arrow.line_style, LineStyle::Solid);
                assert_eq!(arrow.head_style, HeadStyle::Open);
                assert_eq!(text, "Hello");
                assert!(activation.is_none());
                assert!(delay.is_none());
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

        assert_eq!(arrows[0], Arrow { line_style: LineStyle::Solid, head_style: HeadStyle::Open });
        assert_eq!(arrows[1], Arrow { line_style: LineStyle::Solid, head_style: HeadStyle::Closed });
        assert_eq!(arrows[2], Arrow { line_style: LineStyle::Dashed, head_style: HeadStyle::Open });
        assert_eq!(arrows[3], Arrow { line_style: LineStyle::Dashed, head_style: HeadStyle::Closed });
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
        assert_eq!(actors[0].0, "Bob");
        assert_eq!(actors[1].0, "Alice");
        assert_eq!(actors[2].0, "Charlie");
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

    #[test]
    fn test_resolve_actors_alias_is_reference() {
        let input = "participant User as U\nparticipant Server as S\nU->S: hello";
        let doc = parse_document(input).unwrap();
        let actors = resolve_actors(&doc);
        assert_eq!(actors.len(), 2);
        assert_eq!(actors[0].0, "U");
        assert_eq!(actors[0].1, "User");
        assert_eq!(actors[1].0, "S");
        assert_eq!(actors[1].1, "Server");
    }

    // --- New feature tests ---

    #[test]
    fn test_activation_modifier_plus() {
        let doc = parse_document("A->+B: activate").unwrap();
        match &doc.statements[0] {
            Statement::Message { from, to, activation, .. } => {
                assert_eq!(from, "A");
                assert_eq!(to, "B");
                assert_eq!(*activation, Some(ActivationModifier::Activate));
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn test_activation_modifier_minus() {
        let doc = parse_document("B-->-A: deactivate").unwrap();
        match &doc.statements[0] {
            Statement::Message { from, to, activation, arrow, .. } => {
                assert_eq!(from, "B");
                assert_eq!(to, "A");
                assert_eq!(*activation, Some(ActivationModifier::Deactivate));
                assert_eq!(arrow.line_style, LineStyle::Dashed);
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn test_standalone_activate() {
        let doc = parse_document("activate Bob").unwrap();
        match &doc.statements[0] {
            Statement::Activate(actor) => assert_eq!(actor, "Bob"),
            _ => panic!("expected Activate"),
        }
    }

    #[test]
    fn test_standalone_deactivate() {
        let doc = parse_document("deactivate Bob").unwrap();
        match &doc.statements[0] {
            Statement::Deactivate(actor) => assert_eq!(actor, "Bob"),
            _ => panic!("expected Deactivate"),
        }
    }

    #[test]
    fn test_destroy() {
        let doc = parse_document("destroy C").unwrap();
        match &doc.statements[0] {
            Statement::Destroy(actor) => assert_eq!(actor, "C"),
            _ => panic!("expected Destroy"),
        }
    }

    #[test]
    fn test_frame_alt() {
        let doc = parse_document("alt successful case").unwrap();
        match &doc.statements[0] {
            Statement::FrameOpen { kind, label } => {
                assert_eq!(*kind, FrameKind::Alt);
                assert_eq!(label, "successful case");
            }
            _ => panic!("expected FrameOpen"),
        }
    }

    #[test]
    fn test_frame_loop() {
        let doc = parse_document("loop 1000 times").unwrap();
        match &doc.statements[0] {
            Statement::FrameOpen { kind, label } => {
                assert_eq!(*kind, FrameKind::Loop);
                assert_eq!(label, "1000 times");
            }
            _ => panic!("expected FrameOpen"),
        }
    }

    #[test]
    fn test_frame_opt_no_label() {
        let doc = parse_document("opt").unwrap();
        match &doc.statements[0] {
            Statement::FrameOpen { kind, label } => {
                assert_eq!(*kind, FrameKind::Opt);
                assert_eq!(label, "");
            }
            _ => panic!("expected FrameOpen"),
        }
    }

    #[test]
    fn test_frame_else() {
        let doc = parse_document("else some failure").unwrap();
        match &doc.statements[0] {
            Statement::FrameElse { label } => {
                assert_eq!(label, "some failure");
            }
            _ => panic!("expected FrameElse"),
        }
    }

    #[test]
    fn test_frame_end() {
        let doc = parse_document("end").unwrap();
        assert!(matches!(&doc.statements[0], Statement::FrameEnd));
    }

    #[test]
    fn test_frame_end_with_keyword() {
        let doc = parse_document("end alt").unwrap();
        assert!(matches!(&doc.statements[0], Statement::FrameEnd));
    }

    #[test]
    fn test_slanted_arrow() {
        let doc = parse_document("A->(3)B: delayed").unwrap();
        match &doc.statements[0] {
            Statement::Message { from, to, delay, text, .. } => {
                assert_eq!(from, "A");
                assert_eq!(to, "B");
                assert_eq!(*delay, Some(3));
                assert_eq!(text, "delayed");
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn test_slanted_with_activation() {
        let doc = parse_document("A->(2)+B: both").unwrap();
        match &doc.statements[0] {
            Statement::Message { delay, activation, .. } => {
                assert_eq!(*delay, Some(2));
                assert_eq!(*activation, Some(ActivationModifier::Activate));
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn test_full_wsd_example() {
        let input = "\
title WebSequenceDiagrams Client
Caller->+Client: Generate Diagram
Client->+WebSequenceDiagrams: Create Diagram
alt no api key and using paid features
    WebSequenceDiagrams-->Client: 402
    Client-->Caller: Error
end alt
WebSequenceDiagrams-->-Client: 200
alt has errors
    Client-->Caller: Invalid
end alt
Client->+WebSequenceDiagrams: Get image
WebSequenceDiagrams-->-Client: Image
Client-->-Caller: Image";
        let doc = parse_document(input).unwrap();
        // Count specific statement types
        let msgs = doc.statements.iter().filter(|s| matches!(s, Statement::Message { .. })).count();
        let frames = doc.statements.iter().filter(|s| matches!(s, Statement::FrameOpen { .. })).count();
        let ends = doc.statements.iter().filter(|s| matches!(s, Statement::FrameEnd)).count();
        assert_eq!(msgs, 9);
        assert_eq!(frames, 2);
        assert_eq!(ends, 2);
    }

    #[test]
    fn test_nested_frames() {
        let input = "\
alt outer
    Alice->Bob: hi
    opt inner
        Bob->Alice: hey
    end
end";
        let doc = parse_document(input).unwrap();
        let stmts: Vec<_> = doc.statements.iter().collect();
        assert!(matches!(stmts[0], Statement::FrameOpen { kind: FrameKind::Alt, .. }));
        assert!(matches!(stmts[1], Statement::Message { .. }));
        assert!(matches!(stmts[2], Statement::FrameOpen { kind: FrameKind::Opt, .. }));
        assert!(matches!(stmts[3], Statement::Message { .. }));
        assert!(matches!(stmts[4], Statement::FrameEnd));
        assert!(matches!(stmts[5], Statement::FrameEnd));
    }

    #[test]
    fn test_resolve_actors_from_activate() {
        let input = "Alice->Bob: hi\nactivate Charlie";
        let doc = parse_document(input).unwrap();
        let actors = resolve_actors(&doc);
        assert_eq!(actors.len(), 3);
        assert_eq!(actors[2].0, "Charlie");
    }
}
