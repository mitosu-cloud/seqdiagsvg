use seqdiagsvg::{render_to_png, render_to_svg, RenderOptions, StyleConfig};

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

// --- New feature tests ---

#[test]
fn test_custom_note_color() {
    let opts = RenderOptions {
        note_color: [0xCC, 0xEE, 0xFF, 0xFF],
        ..RenderOptions::default()
    };
    let input = "A->B: hi\nnote right of B: Custom color";
    let svg = render_to_svg(input, Some(opts)).unwrap();
    assert!(svg.contains("#cceeff"), "SVG should contain custom note color");
    assert!(!svg.contains("#ffffcc"), "SVG should not contain default yellow");
}

#[test]
fn test_custom_style_config() {
    let opts = RenderOptions {
        style: StyleConfig {
            arrow_stroke_width: 3.0,
            actor_box_corner_radius: 10.0,
            note_corner_radius: 5.0,
            ..StyleConfig::default()
        },
        ..RenderOptions::default()
    };
    let input = "A->B: hi\nnote right of B: Styled";
    let svg = render_to_svg(input, Some(opts)).unwrap();
    assert!(svg.contains("stroke-width=\"3\""), "SVG should contain custom arrow stroke width");
    assert!(svg.contains("rx=\"10\""), "SVG should contain custom actor box corner radius");
    assert!(svg.contains("rx=\"5\""), "SVG should contain custom note corner radius");
}

#[test]
fn test_max_width_svg() {
    // First render without constraints to get natural size
    let input = "A->B: hello\nB->C: world\nC->A: done";
    let svg_natural = render_to_svg(input, None).unwrap();

    // Now render with a small max_width
    let opts = RenderOptions {
        max_width: Some(200.0),
        ..RenderOptions::default()
    };
    let svg_constrained = render_to_svg(input, Some(opts)).unwrap();

    // The constrained SVG should have a smaller width attribute but same viewBox
    assert!(svg_constrained.contains("width=\"200\"") || svg_constrained.contains("width=\"200."),
        "constrained SVG width should be at or below 200");
    // viewBox should still be the natural (larger) size
    assert_ne!(svg_natural, svg_constrained);
}

#[test]
fn test_max_width_png() {
    let input = "A->B: hello\nB->C: world";

    let png_natural = render_to_png(input, None).unwrap();

    let opts = RenderOptions {
        max_width: Some(100.0),
        ..RenderOptions::default()
    };
    let png_constrained = render_to_png(input, Some(opts)).unwrap();

    // Constrained PNG should be smaller
    assert!(png_constrained.len() < png_natural.len(),
        "constrained PNG should be smaller than natural");
}

#[test]
fn test_multiline_note() {
    let input = r"A->B: hi
note right of B: Line one\nLine two";
    let svg = render_to_svg(input, None).unwrap();
    assert!(svg.starts_with("<svg"));
    // The note rect should be taller than a single-line note
    // Just verify it renders without error
    assert!(svg.contains("<rect"));
}

#[test]
fn test_multiline_message() {
    let input = r"A->B: First line\nSecond line";
    let svg = render_to_svg(input, None).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
}

#[test]
fn test_multiline_note_png() {
    let input = r"A->B: hi
note right of B: Line one\nLine two\nLine three";
    let png = render_to_png(input, None).unwrap();
    assert_eq!(&png[0..4], &[0x89, 0x50, 0x4E, 0x47]);
}

#[test]
fn test_max_constraint_no_upscale() {
    // Max constraints should never scale UP
    let input = "A->B: hi";
    let opts_huge = RenderOptions {
        max_width: Some(10000.0),
        max_height: Some(10000.0),
        ..RenderOptions::default()
    };
    let svg_natural = render_to_svg(input, None).unwrap();
    let svg_huge = render_to_svg(input, Some(opts_huge)).unwrap();
    // With huge max, width/height attrs should match natural size
    // (glyph def ordering is non-deterministic, so compare dimensions not full string)
    let extract_dims = |svg: &str| -> (String, String) {
        let w = svg.split("width=\"").nth(1).unwrap().split('"').next().unwrap().to_string();
        let h = svg.split("height=\"").nth(1).unwrap().split('"').next().unwrap().to_string();
        (w, h)
    };
    assert_eq!(extract_dims(&svg_natural), extract_dims(&svg_huge));
}

const WSD_CLIENT_DIAGRAM: &str = "\
title WebSequenceDiagrams Client

Caller->+Client: Generate Diagram
Client->+WebSequenceDiagrams: Create Diagram

alt no api key and using paid features
    WebSequenceDiagrams-->Client: 402 - You need to pay
    Client-->Caller: <YouNeedToPayWSDSome$$$Error>
end alt

WebSequenceDiagrams-->-Client: 200 - <Errors, Link to Diagram>

alt has errors
    Client-->Caller: <InvalidSequenceDiagramError>
end alt

Client->+WebSequenceDiagrams: Get image
WebSequenceDiagrams-->-Client: <Image>
Client-->-Caller: <Image>
";

#[test]
fn test_wsd_client_svg() {
    let svg = render_to_svg(WSD_CLIENT_DIAGRAM, None).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
    // Should have alt frames
    assert!(svg.contains("<rect"), "SVG should contain rectangles for frames");
}

#[test]
fn test_wsd_client_png() {
    let png = render_to_png(WSD_CLIENT_DIAGRAM, None).unwrap();
    assert_eq!(&png[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    assert!(png.len() > 1000, "PNG should be reasonably large for a complex diagram");
}

#[test]
fn test_wsd_client_dollar_signs_rendered() {
    // Verify that $$$ characters produce glyph references in SVG
    let svg = render_to_svg(WSD_CLIENT_DIAGRAM, None).unwrap();
    // Count <use> elements — a complex diagram should have many glyphs
    let use_count = svg.matches("<use ").count();
    assert!(use_count > 100, "expected >100 glyph uses, got {}", use_count);
}

#[test]
fn test_wsd_client_dark_theme() {
    let opts = RenderOptions {
        fg_color: [0xD4, 0xD4, 0xD4, 0xFF],
        bg_color: [0x1E, 0x1E, 0x1E, 0xFF],
        note_color: [0x33, 0x33, 0x33, 0xFF],
        actor_fill: [0xE0, 0xE0, 0xE0, 0xFF],
        actor_text_color: [0x1A, 0x1A, 0x1A, 0xFF],
        note_text_color: [0x1A, 0x1A, 0x1A, 0xFF],
        ..RenderOptions::default()
    };
    let svg = render_to_svg(WSD_CLIENT_DIAGRAM, Some(opts)).unwrap();
    assert!(svg.contains("#1e1e1e"), "SVG should contain dark background");
    assert!(svg.contains("#e0e0e0"), "SVG should contain light actor fill");
    assert!(svg.contains("#1a1a1a"), "SVG should contain dark actor text");
}
