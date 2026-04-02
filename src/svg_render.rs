use std::collections::HashMap;
use std::fmt::Write;

use skrifa::instance::LocationRef;
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::MetadataProvider;

use crate::ast::{HeadStyle, LineStyle};
use crate::font::{DiagramFont, FontSource};
use crate::layout::*;
use crate::RenderOptions;

/// Pen that converts glyph outlines to SVG path `d` attribute commands.
struct SvgPen {
    d: String,
}

impl SvgPen {
    fn new() -> Self {
        Self { d: String::new() }
    }
}

fn fmt(v: f32) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0');
    let s = s.trim_end_matches('.');
    s.to_string()
}

impl OutlinePen for SvgPen {
    fn move_to(&mut self, x: f32, y: f32) {
        write!(self.d, "M{} {}", fmt(x), fmt(y)).unwrap();
    }
    fn line_to(&mut self, x: f32, y: f32) {
        write!(self.d, "L{} {}", fmt(x), fmt(y)).unwrap();
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        write!(self.d, "Q{} {} {} {}", fmt(cx0), fmt(cy0), fmt(x), fmt(y)).unwrap();
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        write!(
            self.d,
            "C{} {} {} {} {} {}",
            fmt(cx0),
            fmt(cy0),
            fmt(cx1),
            fmt(cy1),
            fmt(x),
            fmt(y)
        )
        .unwrap();
    }
    fn close(&mut self) {
        self.d.push('Z');
    }
}

/// Glyph def key: (glyph_id, font_size_bits, font_source)
type GlyphKey = (u16, u32, FontSource);

fn glyph_path_data(
    font: &DiagramFont,
    glyph_id: u16,
    font_size_px: f32,
    source: FontSource,
) -> Option<String> {
    let gid = skrifa::GlyphId::new(glyph_id as u32);
    let font_ref = font.font_for(source);
    let outlines = font_ref.outline_glyphs();
    let outline = outlines.get(gid)?;
    let size = skrifa::instance::Size::new(font_size_px);
    let settings = DrawSettings::unhinted(size, LocationRef::default());
    let mut pen = SvgPen::new();
    outline.draw(settings, &mut pen).ok()?;
    if pen.d.is_empty() {
        None
    } else {
        Some(pen.d)
    }
}

fn color_to_svg(rgba: [u8; 4]) -> String {
    if rgba[3] == 255 {
        format!("#{:02x}{:02x}{:02x}", rgba[0], rgba[1], rgba[2])
    } else {
        let alpha = rgba[3] as f32 / 255.0;
        format!(
            "#{:02x}{:02x}{:02x}\" fill-opacity=\"{:.2}",
            rgba[0], rgba[1], rgba[2], alpha
        )
    }
}

/// Collect all unique (glyph_id, font_size_bits, font_source) from all text.
fn collect_text_glyphs(
    font: &DiagramFont,
    texts: &[&PositionedText],
    glyphs: &mut HashMap<GlyphKey, ()>,
) {
    for pt in texts {
        for c in pt.text.chars() {
            if let Some((gid, source)) = font.resolve_glyph(c) {
                glyphs.insert((gid.to_u32() as u16, pt.font_size_px.to_bits(), source), ());
            }
        }
    }
}

/// Render positioned text as a series of <use> references.
fn render_text_uses(
    out: &mut String,
    font: &DiagramFont,
    pt: &PositionedText,
    defs: &HashMap<GlyphKey, String>,
) {
    let total_width = font.text_width(&pt.text, pt.font_size_px);
    let start_x = match pt.anchor {
        TextAnchor::Middle => pt.x - total_width / 2.0,
        TextAnchor::Start => pt.x,
    };

    let mut cx = start_x;
    for c in pt.text.chars() {
        if let Some((gid, source)) = font.resolve_glyph(c) {
            let key = (gid.to_u32() as u16, pt.font_size_px.to_bits(), source);
            if let Some(def_id) = defs.get(&key) {
                write!(
                    out,
                    "<use href=\"#{}\" transform=\"matrix(1 0 0 -1 {} {})\"/>",
                    def_id,
                    fmt(cx),
                    fmt(pt.y)
                )
                .unwrap();
            }
            let gm = font.glyph_metrics_for(pt.font_size_px, source);
            cx += gm.advance_width(gid).unwrap_or(0.0);
        }
    }
}

pub fn render_to_svg_string(
    font: &DiagramFont,
    layout: &DiagramLayout,
    opts: &RenderOptions,
) -> String {
    let fg = color_to_svg(opts.fg_color);
    let bg = color_to_svg(opts.bg_color);

    // Collect all text elements
    let mut all_texts: Vec<&PositionedText> = Vec::new();
    if let Some(ref t) = layout.title {
        all_texts.push(t);
    }
    for a in &layout.actors {
        all_texts.push(&a.top_label);
        all_texts.push(&a.bottom_label);
    }
    for m in &layout.messages {
        all_texts.push(&m.label);
    }
    for n in &layout.notes {
        all_texts.push(&n.text);
    }

    // Build glyph defs
    let mut glyph_set = HashMap::new();
    collect_text_glyphs(font, &all_texts, &mut glyph_set);

    let mut defs_map: HashMap<GlyphKey, String> = HashMap::new();
    let mut defs_svg = String::new();
    let mut glyph_idx = 0;

    for &(glyph_id, size_bits, source) in glyph_set.keys() {
        let font_size_px = f32::from_bits(size_bits);
        if let Some(path_d) = glyph_path_data(font, glyph_id, font_size_px, source) {
            let def_id = format!("g{}", glyph_idx);
            write!(defs_svg, r#"<path id="{}" d="{}"/>"#, def_id, path_d).unwrap();
            defs_map.insert((glyph_id, size_bits, source), def_id);
            glyph_idx += 1;
        }
    }

    // SVG header
    let mut svg = String::new();
    write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">"#,
        fmt(layout.width),
        fmt(layout.height),
        fmt(layout.width),
        fmt(layout.height)
    )
    .unwrap();

    // Defs: glyph paths + arrowhead markers
    svg.push_str("<defs>");
    svg.push_str(&defs_svg);

    // Open arrowhead marker (two lines forming a V)
    write!(
        svg,
        r#"<marker id="arrow-open" markerWidth="10" markerHeight="10" refX="10" refY="5" orient="auto-start-reverse" markerUnits="userSpaceOnUse"><path d="M0,0 L10,5 L0,10" fill="none" stroke="{}" stroke-width="1.5"/></marker>"#,
        fg
    )
    .unwrap();

    // Closed arrowhead marker (filled triangle)
    write!(
        svg,
        r#"<marker id="arrow-closed" markerWidth="10" markerHeight="10" refX="10" refY="5" orient="auto-start-reverse" markerUnits="userSpaceOnUse"><path d="M0,0 L10,5 L0,10 Z" fill="{}"/></marker>"#,
        fg
    )
    .unwrap();

    svg.push_str("</defs>");

    // Background
    if opts.bg_color[3] > 0 {
        write!(
            svg,
            r#"<rect width="100%" height="100%" fill="{}"/>"#,
            bg
        )
        .unwrap();
    }

    // Content group
    write!(svg, r#"<g fill="{}" stroke="{}">"#, fg, fg).unwrap();

    // Title
    if let Some(ref t) = layout.title {
        render_text_uses(&mut svg, font, t, &defs_map);
    }

    // Lifelines (dashed vertical lines, drawn behind everything else)
    for ll in &layout.lifelines {
        write!(
            svg,
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke-dasharray="6,4" stroke-width="1" fill="none"/>"#,
            fmt(ll.x),
            fmt(ll.y_start),
            fmt(ll.x),
            fmt(ll.y_end)
        )
        .unwrap();
    }

    // Actor boxes
    for a in &layout.actors {
        render_box(&mut svg, &a.top_box, &fg);
        render_text_uses(&mut svg, font, &a.top_label, &defs_map);
        render_box(&mut svg, &a.bottom_box, &fg);
        render_text_uses(&mut svg, font, &a.bottom_label, &defs_map);
    }

    // Messages
    for m in &layout.messages {
        let marker = match m.arrow.head_style {
            HeadStyle::Open => "url(#arrow-open)",
            HeadStyle::Closed => "url(#arrow-closed)",
        };
        let dash = match m.arrow.line_style {
            LineStyle::Solid => "",
            LineStyle::Dashed => r#" stroke-dasharray="8,4""#,
        };

        if m.is_self {
            let x = m.from_x;
            let jog_x = x + 40.0;
            let y1 = m.y;
            let y2 = m.y + 30.0;
            write!(
                svg,
                r#"<path d="M{} {} L{} {} L{} {} L{} {}" fill="none" stroke-width="1.5"{} marker-end="{}"/>"#,
                fmt(x), fmt(y1),
                fmt(jog_x), fmt(y1),
                fmt(jog_x), fmt(y2),
                fmt(x), fmt(y2),
                dash, marker
            )
            .unwrap();
        } else {
            write!(
                svg,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke-width="1.5" fill="none"{} marker-end="{}"/>"#,
                fmt(m.from_x),
                fmt(m.y),
                fmt(m.to_x),
                fmt(m.y),
                dash,
                marker
            )
            .unwrap();
        }

        render_text_uses(&mut svg, font, &m.label, &defs_map);
    }

    // Notes
    for n in &layout.notes {
        let note_bg = "#ffffcc";
        write!(
            svg,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1\" rx=\"2\"/>",
            fmt(n.rect.x),
            fmt(n.rect.y),
            fmt(n.rect.width),
            fmt(n.rect.height),
            note_bg,
            fg
        )
        .unwrap();
        render_text_uses(&mut svg, font, &n.text, &defs_map);
    }

    svg.push_str("</g></svg>");
    svg
}

fn render_box(out: &mut String, rect: &Rect, stroke_color: &str) {
    write!(
        out,
        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="white" stroke="{}" stroke-width="1.5" rx="3"/>"#,
        fmt(rect.x),
        fmt(rect.y),
        fmt(rect.width),
        fmt(rect.height),
        stroke_color
    )
    .unwrap();
}
