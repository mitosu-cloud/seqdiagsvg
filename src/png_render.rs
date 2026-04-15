use skrifa::outline::OutlinePen;
use skrifa::MetadataProvider;
use tiny_skia::{
    FillRule, Paint, Path, PathBuilder, Pixmap, Stroke, StrokeDash, Transform,
};

use crate::ast::{HeadStyle, LineStyle};
use crate::font::DiagramFont;
use crate::layout::*;
use crate::RenderOptions;

/// Pen that converts glyph outlines to tiny-skia path commands.
struct SkiaPen {
    pb: PathBuilder,
}

impl SkiaPen {
    fn new() -> Self {
        Self {
            pb: PathBuilder::new(),
        }
    }
    fn finish(self) -> Option<Path> {
        self.pb.finish()
    }
}

impl OutlinePen for SkiaPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(x, y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.pb.quad_to(cx0, cy0, x, y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.pb.cubic_to(cx0, cy0, cx1, cy1, x, y);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

fn make_paint(rgba: [u8; 4]) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]);
    paint.anti_alias = true;
    paint
}

fn make_stroke(width: f32, dash: Option<&[f32]>) -> Stroke {
    let mut stroke = Stroke {
        width,
        ..Stroke::default()
    };
    if let Some(intervals) = dash {
        stroke.dash = StrokeDash::new(intervals.to_vec(), 0.0);
    }
    stroke
}

/// Render text at the given position using glyph outlines, with multi-line support.
fn render_text(
    font: &DiagramFont,
    pixmap: &mut Pixmap,
    pt: &PositionedText,
    fg: [u8; 4],
    global_transform: Transform,
) {
    let paint = make_paint(fg);
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
                let font_ref = font.font_for(source);
                let outlines = font_ref.outline_glyphs();
                if let Some(outline) = outlines.get(gid) {
                    let mut pen = SkiaPen::new();
                    let settings = draw_settings(pt.font_size_px);
                    if outline.draw(settings, &mut pen).is_ok() {
                        if let Some(path) = pen.finish() {
                            let glyph_transform =
                                Transform::from_row(1.0, 0.0, 0.0, -1.0, cx, y);
                            let transform = glyph_transform.post_concat(global_transform);
                            pixmap.fill_path(
                                &path,
                                &paint,
                                FillRule::Winding,
                                transform,
                                None,
                            );
                        }
                    }
                }
                let gm = font.glyph_metrics_for(pt.font_size_px, source);
                cx += gm.advance_width(gid).unwrap_or(0.0);
            }
        }
    }
}

/// Draw a filled rectangle.
fn fill_rect(pixmap: &mut Pixmap, rect: &Rect, rgba: [u8; 4], transform: Transform) {
    let paint = make_paint(rgba);
    if let Some(r) = tiny_skia::Rect::from_xywh(rect.x, rect.y, rect.width, rect.height) {
        pixmap.fill_rect(r, &paint, transform, None);
    }
}

/// Shorthand to create DrawSettings.
fn draw_settings(font_size_px: f32) -> skrifa::outline::DrawSettings<'static> {
    let size = skrifa::instance::Size::new(font_size_px);
    skrifa::outline::DrawSettings::unhinted(size, skrifa::instance::LocationRef::default())
}

/// Draw a stroked rectangle with rounded corners.
fn stroke_rect(pixmap: &mut Pixmap, rect: &Rect, rgba: [u8; 4], width: f32, corner_radius: f32, transform: Transform) {
    let paint = make_paint(rgba);
    let stroke = make_stroke(width, None);
    let r = corner_radius;
    let mut pb = PathBuilder::new();
    // Rounded rect path
    pb.move_to(rect.x + r, rect.y);
    pb.line_to(rect.x + rect.width - r, rect.y);
    pb.quad_to(rect.x + rect.width, rect.y, rect.x + rect.width, rect.y + r);
    pb.line_to(rect.x + rect.width, rect.y + rect.height - r);
    pb.quad_to(
        rect.x + rect.width,
        rect.y + rect.height,
        rect.x + rect.width - r,
        rect.y + rect.height,
    );
    pb.line_to(rect.x + r, rect.y + rect.height);
    pb.quad_to(rect.x, rect.y + rect.height, rect.x, rect.y + rect.height - r);
    pb.line_to(rect.x, rect.y + r);
    pb.quad_to(rect.x, rect.y, rect.x + r, rect.y);
    pb.close();
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, &paint, &stroke, transform, None);
    }
}

/// Draw a line from (x1,y1) to (x2,y2).
fn draw_line(
    pixmap: &mut Pixmap,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    rgba: [u8; 4],
    width: f32,
    dash: Option<&[f32]>,
    transform: Transform,
) {
    let paint = make_paint(rgba);
    let stroke = make_stroke(width, dash);
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, &paint, &stroke, transform, None);
    }
}

/// Draw an arrowhead at (tip_x, tip_y) pointing in the given direction.
fn draw_arrowhead(
    pixmap: &mut Pixmap,
    tip_x: f32,
    tip_y: f32,
    pointing_right: bool,
    head_style: HeadStyle,
    rgba: [u8; 4],
    arrow_stroke_width: f32,
    transform: Transform,
) {
    let size = 8.0f32;
    let dx = if pointing_right { -size } else { size };
    let dy = size * 0.5;

    let mut pb = PathBuilder::new();
    pb.move_to(tip_x, tip_y);
    pb.line_to(tip_x + dx, tip_y - dy);
    pb.line_to(tip_x + dx, tip_y + dy);
    pb.close();

    if let Some(path) = pb.finish() {
        let paint = make_paint(rgba);
        match head_style {
            HeadStyle::Closed => {
                pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
            }
            HeadStyle::Open => {
                // Draw just the two lines of the V
                let mut pb2 = PathBuilder::new();
                pb2.move_to(tip_x + dx, tip_y - dy);
                pb2.line_to(tip_x, tip_y);
                pb2.line_to(tip_x + dx, tip_y + dy);
                if let Some(path2) = pb2.finish() {
                    let stroke = make_stroke(arrow_stroke_width, None);
                    pixmap.stroke_path(&path2, &paint, &stroke, transform, None);
                }
            }
        }
    }
}

/// Render the full diagram onto a pixmap.
pub fn render_diagram(
    font: &DiagramFont,
    pixmap: &mut Pixmap,
    layout: &DiagramLayout,
    opts: &RenderOptions,
    transform: Transform,
) {
    let fg = opts.fg_color;
    let actor_fill = opts.actor_fill;
    let actor_text = opts.actor_text_color;
    let note_text = opts.note_text_color;
    let style = &opts.style;
    let note_bg = opts.note_color;

    // Title
    if let Some(ref t) = layout.title {
        render_text(font, pixmap, t, fg, transform);
    }

    // Lifelines
    let ll_dash = [style.lifeline_dash[0], style.lifeline_dash[1]];
    for ll in &layout.lifelines {
        draw_line(
            pixmap,
            ll.x,
            ll.y_start,
            ll.x,
            ll.y_end,
            fg,
            style.lifeline_stroke_width,
            Some(&ll_dash),
            transform,
        );
    }

    // Frames (behind messages/notes, in front of lifelines)
    let frame_fill_color = opts.frame_fill;
    let frame_else_dash = [style.frame_else_dash[0], style.frame_else_dash[1]];
    for f in &layout.frames {
        fill_rect(pixmap, &f.outer_rect, frame_fill_color, transform);
        stroke_rect(pixmap, &f.outer_rect, fg, style.frame_stroke_width, style.frame_corner_radius, transform);

        // Tab pentagon
        let tx = f.tab_rect.x;
        let ty = f.tab_rect.y;
        let tw = f.tab_rect.width;
        let th = f.tab_rect.height;
        let fold = 10.0f32.min(tw * 0.3);
        let mut pb = PathBuilder::new();
        pb.move_to(tx, ty);
        pb.line_to(tx + tw, ty);
        pb.line_to(tx + tw, ty + th - fold);
        pb.line_to(tx + tw - fold, ty + th);
        pb.line_to(tx, ty + th);
        pb.close();
        if let Some(path) = pb.finish() {
            let paint = make_paint(frame_fill_color);
            pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
            let stroke_paint = make_paint(fg);
            let stroke = make_stroke(style.frame_stroke_width, None);
            pixmap.stroke_path(&path, &stroke_paint, &stroke, transform, None);
        }

        render_text(font, pixmap, &f.tab_label, fg, transform);
        if let Some(ref cond) = f.condition_label {
            render_text(font, pixmap, cond, fg, transform);
        }

        for div in &f.else_dividers {
            draw_line(pixmap, div.x_start, div.y, div.x_end, div.y, fg, style.frame_stroke_width, Some(&frame_else_dash), transform);
            if let Some(ref label) = div.label {
                render_text(font, pixmap, label, fg, transform);
            }
        }
    }

    // Activation boxes
    let activation_fill = opts.activation_fill;
    for ab in &layout.activation_boxes {
        fill_rect(pixmap, &ab.rect, activation_fill, transform);
        stroke_rect(pixmap, &ab.rect, fg, style.activation_stroke_width, 0.0, transform);
    }

    // Actor boxes
    for a in &layout.actors {
        // Top box
        fill_rect(pixmap, &a.top_box, actor_fill, transform);
        stroke_rect(pixmap, &a.top_box, fg, style.actor_box_stroke_width, style.actor_box_corner_radius, transform);
        render_text(font, pixmap, &a.top_label, actor_text, transform);
        // Bottom box
        fill_rect(pixmap, &a.bottom_box, actor_fill, transform);
        stroke_rect(pixmap, &a.bottom_box, fg, style.actor_box_stroke_width, style.actor_box_corner_radius, transform);
        render_text(font, pixmap, &a.bottom_label, actor_text, transform);
    }

    // Messages
    let msg_dash = [style.message_dash[0], style.message_dash[1]];
    for m in &layout.messages {
        let dash: Option<&[f32]> = match m.arrow.line_style {
            LineStyle::Solid => None,
            LineStyle::Dashed => Some(&msg_dash),
        };

        if m.is_self {
            let x = m.from_x;
            let jog_x = x + 40.0;
            let y1 = m.from_y;
            let y2 = m.from_y + 30.0;
            draw_line(pixmap, x, y1, jog_x, y1, fg, style.arrow_stroke_width, dash, transform);
            draw_line(pixmap, jog_x, y1, jog_x, y2, fg, style.arrow_stroke_width, dash, transform);
            draw_line(pixmap, jog_x, y2, x, y2, fg, style.arrow_stroke_width, dash, transform);
            draw_arrowhead(pixmap, x, y2, false, m.arrow.head_style, fg, style.arrow_stroke_width, transform);
        } else {
            let pointing_right = m.to_x > m.from_x;
            draw_line(pixmap, m.from_x, m.from_y, m.to_x, m.to_y, fg, style.arrow_stroke_width, dash, transform);
            draw_arrowhead(pixmap, m.to_x, m.to_y, pointing_right, m.arrow.head_style, fg, style.arrow_stroke_width, transform);
        }

        // Message label
        render_text(font, pixmap, &m.label, fg, transform);
    }

    // Notes
    for n in &layout.notes {
        fill_rect(pixmap, &n.rect, note_bg, transform);
        stroke_rect(pixmap, &n.rect, fg, style.note_stroke_width, style.note_corner_radius, transform);
        render_text(font, pixmap, &n.text, note_text, transform);
    }

    // Destroy markers
    for dm in &layout.destroy_markers {
        let s = dm.size;
        draw_line(pixmap, dm.center_x - s, dm.center_y - s, dm.center_x + s, dm.center_y + s, fg, style.destroy_stroke_width, None, transform);
        draw_line(pixmap, dm.center_x + s, dm.center_y - s, dm.center_x - s, dm.center_y + s, fg, style.destroy_stroke_width, None, transform);
    }
}
