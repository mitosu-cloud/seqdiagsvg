use seqdiagsvg::{render_to_png, render_to_svg, RenderOptions};

const SAMPLE_DIAGRAM: &str = "\
title: Auth Flow
participant Client
participant Server
participant DB

Client->Server: POST /login
note right of Server: Validate credentials
Server->DB: SELECT user
DB-->Server: user record
Server-->Client: 200 OK + token
";

#[test]
fn test_svg_output_is_valid() {
    let svg = render_to_svg(SAMPLE_DIAGRAM, None).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("viewBox="));
    assert!(svg.ends_with("</svg>"));
    assert!(svg.contains("<defs>"));
}

#[test]
fn test_png_output_is_valid() {
    let png = render_to_png(SAMPLE_DIAGRAM, None).unwrap();
    // PNG magic bytes
    assert_eq!(&png[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    assert!(png.len() > 100);
}

#[test]
fn test_svg_contains_actors() {
    let svg = render_to_svg(SAMPLE_DIAGRAM, None).unwrap();
    // Should contain actor box rects and glyph references
    assert!(svg.contains("<rect"));
    assert!(svg.contains("<use"));
}

#[test]
fn test_svg_contains_lifelines() {
    let svg = render_to_svg(SAMPLE_DIAGRAM, None).unwrap();
    assert!(svg.contains("stroke-dasharray=\"6,4\""));
}

#[test]
fn test_svg_contains_arrows() {
    let svg = render_to_svg(SAMPLE_DIAGRAM, None).unwrap();
    // Should have arrow markers defined
    assert!(svg.contains("marker-end="));
    assert!(svg.contains("arrow-open") || svg.contains("arrow-closed"));
}

#[test]
fn test_svg_contains_notes() {
    let svg = render_to_svg(SAMPLE_DIAGRAM, None).unwrap();
    // Notes have a yellow background
    assert!(svg.contains("#ffffcc"));
}

#[test]
fn test_all_arrow_types() {
    let input = "A->B: solid open\nA->>B: solid closed\nA-->B: dashed open\nA-->>B: dashed closed";
    let svg = render_to_svg(input, None).unwrap();
    assert!(svg.contains("arrow-open"));
    assert!(svg.contains("arrow-closed"));
    assert!(svg.contains("stroke-dasharray=\"8,4\""));
}

#[test]
fn test_self_message() {
    let input = "A->A: self message";
    let svg = render_to_svg(input, None).unwrap();
    // Self-messages use a path (not a line)
    assert!(svg.contains("<path"));
}

#[test]
fn test_custom_colors() {
    let opts = RenderOptions {
        fg_color: [0xFF, 0x00, 0x00, 0xFF],
        bg_color: [0x00, 0x00, 0xFF, 0xFF],
        ..RenderOptions::default()
    };
    let svg = render_to_svg("A->B: hello", Some(opts)).unwrap();
    assert!(svg.contains("#ff0000"));
    assert!(svg.contains("#0000ff"));
}

#[test]
fn test_empty_diagram() {
    let svg = render_to_svg("", None).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
}

#[test]
fn test_title_only() {
    let svg = render_to_svg("title: Just a title", None).unwrap();
    assert!(svg.starts_with("<svg"));
}

#[test]
fn test_note_positions() {
    let input = "\
A->B: hi
note left of A: Left
note right of B: Right
note over A: Over one
note over A, B: Over both
";
    let svg = render_to_svg(input, None).unwrap();
    assert!(svg.starts_with("<svg"));
    // Should have 4 notes (4 yellow rects)
    let note_count = svg.matches("#ffffcc").count();
    assert_eq!(note_count, 4, "expected 4 notes, got {}", note_count);
}

#[test]
fn test_png_custom_scale() {
    let opts = RenderOptions {
        scale: 1.0,
        ..RenderOptions::default()
    };
    let png1 = render_to_png("A->B: hi", Some(opts)).unwrap();

    let opts2 = RenderOptions {
        scale: 3.0,
        ..RenderOptions::default()
    };
    let png2 = render_to_png("A->B: hi", Some(opts2)).unwrap();

    // Higher scale should produce a larger PNG
    assert!(png2.len() > png1.len());
}
