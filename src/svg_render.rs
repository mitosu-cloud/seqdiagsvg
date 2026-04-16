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
            fmt(cx0), fmt(cy0), fmt(cx1), fmt(cy1), fmt(x), fmt(y)
        )
        .unwrap();
    }
    fn close(&mut self) {
        self.d.push('Z');
    }
}

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
    if pen.d.is_empty() { None } else { Some(pen.d) }
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

fn render_text_uses(
    out: &mut String,
    font: &DiagramFont,
    pt: &PositionedText,
    defs: &HashMap<GlyphKey, String>,
) {
    // Glyph paths are filled shapes — suppress inherited stroke so text has no outline
    out.push_str(r#"<g stroke="none">"#);
    render_text_uses_inner(out, font, pt, defs);
    out.push_str("</g>");
}

fn render_text_uses_inner(
    out: &mut String,
    font: &DiagramFont,
    pt: &PositionedText,
    defs: &HashMap<GlyphKey, String>,
) {
    let metrics = font.metrics(pt.font_size_px);
    let line_height = metrics.ascent - metrics.descent;

    for (line_idx, line) in pt.text.split('\n').enumerate() {
        let line_width = font.text_width(line, pt.font_size_px);
        let start_x = match pt.anchor {
            TextAnchor::Middle => pt.x - line_width / 2.0,
            TextAnchor::Start => pt.x,
        };
        let y = pt.y + line_idx as f32 * line_height;

        let mut cx = start_x;
        for c in line.chars() {
            if let Some((gid, source)) = font.resolve_glyph(c) {
                let key = (gid.to_u32() as u16, pt.font_size_px.to_bits(), source);
                if let Some(def_id) = defs.get(&key) {
                    write!(
                        out,
                        "<use href=\"#{}\" transform=\"matrix(1 0 0 -1 {} {})\"/>",
                        def_id, fmt(cx), fmt(y)
                    )
                    .unwrap();
                }
                let gm = font.glyph_metrics_for(pt.font_size_px, source);
                cx += gm.advance_width(gid).unwrap_or(0.0);
            }
        }
    }
}

fn render_text_uses_colored(
    out: &mut String,
    font: &DiagramFont,
    pt: &PositionedText,
    defs: &HashMap<GlyphKey, String>,
    color: &str,
) {
    write!(out, r#"<g fill="{}" stroke="none">"#, color).unwrap();
    render_text_uses_inner(out, font, pt, defs);
    out.push_str("</g>");
}

pub fn render_to_svg_string(
    font: &DiagramFont,
    layout: &DiagramLayout,
    opts: &RenderOptions,
) -> String {
    let fg = color_to_svg(opts.fg_color);
    let bg = color_to_svg(opts.bg_color);
    let actor_fill_color = color_to_svg(opts.actor_fill);
    let actor_text = color_to_svg(opts.actor_text_color);
    let note_text = color_to_svg(opts.note_text_color);
    let activation_fill = color_to_svg(opts.activation_fill);
    // frame_fill resolved per-frame via opts.frame_fill_for_depth()
    let style = &opts.style;
    let ms = style.marker_size;

    // Collect all text elements (including frame labels)
    let mut all_texts: Vec<&PositionedText> = Vec::new();
    if let Some(ref t) = layout.title { all_texts.push(t); }
    for a in &layout.actors {
        all_texts.push(&a.top_label);
        all_texts.push(&a.bottom_label);
    }
    for m in &layout.messages { all_texts.push(&m.label); }
    for n in &layout.notes { all_texts.push(&n.text); }
    for f in &layout.frames {
        all_texts.push(&f.tab_label);
        if let Some(ref c) = f.condition_label { all_texts.push(c); }
        for d in &f.else_dividers {
            if let Some(ref l) = d.label { all_texts.push(l); }
        }
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
    let natural_w = layout.width;
    let natural_h = layout.height;
    let scale_factor = match (opts.max_width, opts.max_height) {
        (Some(mw), Some(mh)) => (mw / natural_w).min(mh / natural_h).min(1.0),
        (Some(mw), None) => (mw / natural_w).min(1.0),
        (None, Some(mh)) => (mh / natural_h).min(1.0),
        (None, None) => 1.0,
    };
    let display_w = natural_w * scale_factor;
    let display_h = natural_h * scale_factor;

    let mut svg = String::new();
    write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">"#,
        fmt(natural_w), fmt(natural_h), fmt(display_w), fmt(display_h)
    ).unwrap();

    // Defs: glyph paths + arrowhead markers
    svg.push_str("<defs>");
    svg.push_str(&defs_svg);

    let half_ms = ms / 2.0;
    write!(
        svg,
        r#"<marker id="arrow-open" markerWidth="{ms}" markerHeight="{ms}" refX="{ms}" refY="{half}" orient="auto-start-reverse" markerUnits="userSpaceOnUse"><path d="M0,0 L{ms},{half} L0,{ms}" fill="none" stroke="{fg}" stroke-width="{sw}"/></marker>"#,
        ms = fmt(ms), half = fmt(half_ms), fg = fg, sw = fmt(style.arrow_stroke_width)
    ).unwrap();
    write!(
        svg,
        r#"<marker id="arrow-closed" markerWidth="{ms}" markerHeight="{ms}" refX="{ms}" refY="{half}" orient="auto-start-reverse" markerUnits="userSpaceOnUse"><path d="M0,0 L{ms},{half} L0,{ms} Z" fill="{fg}"/></marker>"#,
        ms = fmt(ms), half = fmt(half_ms), fg = fg
    ).unwrap();

    // Drop shadow filters (using feGaussianBlur + feOffset + feMerge for max compatibility)
    if let Some(ref shadow) = opts.actor_shadow {
        write_shadow_filter(&mut svg, "actor-shadow", shadow);
    }
    if let Some(ref shadow) = opts.note_shadow {
        write_shadow_filter(&mut svg, "note-shadow", shadow);
    }

    svg.push_str("</defs>");

    // Background
    if opts.bg_color[3] > 0 {
        write!(svg, r#"<rect width="100%" height="100%" fill="{}"/>"#, bg).unwrap();
    }

    // Content group
    write!(svg, r#"<g fill="{}" stroke="{}">"#, fg, fg).unwrap();

    // Title
    if let Some(ref t) = layout.title {
        render_text_uses(&mut svg, font, t, &defs_map);
    }

    // Lifelines
    for ll in &layout.lifelines {
        write!(
            svg,
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke-dasharray="{},{}" stroke-width="{}" fill="none"/>"#,
            fmt(ll.x), fmt(ll.y_start), fmt(ll.x), fmt(ll.y_end),
            fmt(style.lifeline_dash[0]), fmt(style.lifeline_dash[1]),
            fmt(style.lifeline_stroke_width)
        ).unwrap();
    }

    // Frames (behind messages/notes, in front of lifelines)
    for f in &layout.frames {
        let ff = color_to_svg(opts.frame_fill_for_depth(f.nesting_depth));

        // Outer frame rectangle
        write!(
            svg,
            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="{}" rx="{}"/>"#,
            fmt(f.outer_rect.x), fmt(f.outer_rect.y),
            fmt(f.outer_rect.width), fmt(f.outer_rect.height),
            ff, fg, fmt(style.frame_stroke_width), fmt(style.frame_corner_radius)
        ).unwrap();

        // Tab pentagon
        let tx = f.tab_rect.x;
        let ty = f.tab_rect.y;
        let tw = f.tab_rect.width;
        let th = f.tab_rect.height;
        let fold = FRAME_TAB_FOLD.min(tw * 0.3);
        write!(
            svg,
            r#"<path d="M{} {} L{} {} L{} {} L{} {} L{} {} Z" fill="{}" stroke="{}" stroke-width="{}"/>"#,
            fmt(tx), fmt(ty),
            fmt(tx + tw), fmt(ty),
            fmt(tx + tw), fmt(ty + th - fold),
            fmt(tx + tw - fold), fmt(ty + th),
            fmt(tx), fmt(ty + th),
            ff, fg, fmt(style.frame_stroke_width)
        ).unwrap();

        // Tab label
        render_text_uses(&mut svg, font, &f.tab_label, &defs_map);

        // Condition label
        if let Some(ref cond) = f.condition_label {
            render_text_uses(&mut svg, font, cond, &defs_map);
        }

        // Else dividers
        for div in &f.else_dividers {
            write!(
                svg,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-dasharray="{},{}" stroke-width="{}" fill="none"/>"#,
                fmt(div.x_start), fmt(div.y), fmt(div.x_end), fmt(div.y),
                fg, fmt(style.frame_else_dash[0]), fmt(style.frame_else_dash[1]),
                fmt(style.frame_stroke_width)
            ).unwrap();
            if let Some(ref label) = div.label {
                render_text_uses(&mut svg, font, label, &defs_map);
            }
        }
    }

    // Activation boxes
    for ab in &layout.activation_boxes {
        write!(
            svg,
            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
            fmt(ab.rect.x), fmt(ab.rect.y), fmt(ab.rect.width), fmt(ab.rect.height),
            activation_fill, fg, fmt(style.activation_stroke_width)
        ).unwrap();
    }

    // Actor boxes
    let actor_filter = if opts.actor_shadow.is_some() { r#" filter="url(#actor-shadow)""# } else { "" };
    for a in &layout.actors {
        if a.top_box.height > 0.0 {
            render_box(&mut svg, &a.top_box, &actor_fill_color, &fg, style.actor_box_stroke_width, style.actor_box_corner_radius, actor_filter);
            render_text_uses_colored(&mut svg, font, &a.top_label, &defs_map, &actor_text);
        }
        if a.bottom_box.height > 0.0 {
            render_box(&mut svg, &a.bottom_box, &actor_fill_color, &fg, style.actor_box_stroke_width, style.actor_box_corner_radius, actor_filter);
            render_text_uses_colored(&mut svg, font, &a.bottom_label, &defs_map, &actor_text);
        }
    }

    // Messages
    for m in &layout.messages {
        let marker = match m.arrow.head_style {
            HeadStyle::Open => "url(#arrow-open)",
            HeadStyle::Closed => "url(#arrow-closed)",
        };
        let dash = match m.arrow.line_style {
            LineStyle::Solid => String::new(),
            LineStyle::Dashed => format!(r#" stroke-dasharray="{},{}""#, fmt(style.message_dash[0]), fmt(style.message_dash[1])),
        };

        if m.is_self {
            let x = m.from_x;
            let jog_x = x + 40.0;
            let y1 = m.from_y;
            let y2 = m.from_y + 30.0;
            write!(
                svg,
                r#"<path d="M{} {} L{} {} L{} {} L{} {}" fill="none" stroke-width="{}"{} marker-end="{}"/>"#,
                fmt(x), fmt(y1), fmt(jog_x), fmt(y1), fmt(jog_x), fmt(y2), fmt(x), fmt(y2),
                fmt(style.arrow_stroke_width), dash, marker
            ).unwrap();
        } else {
            write!(
                svg,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke-width="{}" fill="none"{} marker-end="{}"/>"#,
                fmt(m.from_x), fmt(m.from_y), fmt(m.to_x), fmt(m.to_y),
                fmt(style.arrow_stroke_width), dash, marker
            ).unwrap();
        }

        render_text_uses(&mut svg, font, &m.label, &defs_map);
    }

    // Notes
    let note_bg = color_to_svg(opts.note_color);
    let note_filter = if opts.note_shadow.is_some() { r#" filter="url(#note-shadow)""# } else { "" };
    for n in &layout.notes {
        write!(
            svg,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\" rx=\"{}\"{}/>",
            fmt(n.rect.x), fmt(n.rect.y), fmt(n.rect.width), fmt(n.rect.height),
            note_bg, fg, fmt(style.note_stroke_width), fmt(style.note_corner_radius), note_filter
        ).unwrap();
        render_text_uses_colored(&mut svg, font, &n.text, &defs_map, &note_text);
    }

    // Destroy markers (X crosses)
    for dm in &layout.destroy_markers {
        let s = dm.size;
        write!(
            svg,
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" fill="none"/>"#,
            fmt(dm.center_x - s), fmt(dm.center_y - s), fmt(dm.center_x + s), fmt(dm.center_y + s),
            fg, fmt(style.destroy_stroke_width)
        ).unwrap();
        write!(
            svg,
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" fill="none"/>"#,
            fmt(dm.center_x + s), fmt(dm.center_y - s), fmt(dm.center_x - s), fmt(dm.center_y + s),
            fg, fmt(style.destroy_stroke_width)
        ).unwrap();
    }

    svg.push_str("</g></svg>");
    svg
}

fn render_box(out: &mut String, rect: &Rect, fill_color: &str, stroke_color: &str, stroke_width: f32, corner_radius: f32, filter: &str) {
    write!(
        out,
        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="{}" rx="{}"{}/>"#,
        fmt(rect.x), fmt(rect.y), fmt(rect.width), fmt(rect.height),
        fill_color, stroke_color, fmt(stroke_width), fmt(corner_radius), filter
    ).unwrap();
}

/// Emit an SVG filter definition for a drop shadow using the universal
/// feGaussianBlur + feOffset + feMerge approach (max compatibility).
fn write_shadow_filter(out: &mut String, id: &str, shadow: &crate::DropShadow) {
    let r = shadow.color[0];
    let g = shadow.color[1];
    let b = shadow.color[2];
    let a = shadow.color[3] as f32 / 255.0;
    // Expand the filter region so the shadow isn't clipped
    write!(
        out,
        r#"<filter id="{}" x="-20%" y="-20%" width="150%" height="150%" color-interpolation-filters="sRGB">"#,
        id
    ).unwrap();
    write!(
        out,
        r#"<feGaussianBlur in="SourceAlpha" stdDeviation="{}"/>"#,
        fmt(shadow.std_deviation)
    ).unwrap();
    write!(
        out,
        r#"<feOffset dx="{}" dy="{}" result="offsetblur"/>"#,
        fmt(shadow.dx), fmt(shadow.dy)
    ).unwrap();
    write!(
        out,
        r#"<feFlood flood-color="rgb({},{},{})" flood-opacity="{:.2}"/>"#,
        r, g, b, a
    ).unwrap();
    out.push_str(r#"<feComposite in2="offsetblur" operator="in"/>"#);
    out.push_str(r#"<feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge>"#);
    out.push_str("</filter>");
}

// Re-export constant for layout reference
const FRAME_TAB_FOLD: f32 = 10.0;
