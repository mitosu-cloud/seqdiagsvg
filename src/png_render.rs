use skrifa::outline::OutlinePen;
use skrifa::MetadataProvider;
use tiny_skia::{
    FillRule, Paint, Path, PathBuilder, Pixmap, Stroke, StrokeDash, Transform,
};

use crate::ast::{HeadStyle, LineStyle};
use crate::font::DiagramFont;
use crate::layout::*;

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

/// Render text at the given position using glyph outlines.
fn render_text(
    font: &DiagramFont,
    pixmap: &mut Pixmap,
    pt: &PositionedText,
    fg: [u8; 4],
) {
    let paint = make_paint(fg);

    let total_width = font.text_width(&pt.text, pt.font_size_px);
    let start_x = match pt.anchor {
        TextAnchor::Middle => pt.x - total_width / 2.0,
        TextAnchor::Start => pt.x,
    };

    let mut cx = start_x;

    for c in pt.text.chars() {
        if let Some((gid, source)) = font.resolve_glyph(c) {
            let font_ref = font.font_for(source);
            let outlines = font_ref.outline_glyphs();
            if let Some(outline) = outlines.get(gid) {
                let mut pen = SkiaPen::new();
                let settings = draw_settings(pt.font_size_px);
                if outline.draw(settings, &mut pen).is_ok() {
                    if let Some(path) = pen.finish() {
                        let transform =
                            Transform::from_row(1.0, 0.0, 0.0, -1.0, cx, pt.y);
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

/// Draw a filled rectangle.
fn fill_rect(pixmap: &mut Pixmap, rect: &Rect, rgba: [u8; 4]) {
    let paint = make_paint(rgba);
    if let Some(r) = tiny_skia::Rect::from_xywh(rect.x, rect.y, rect.width, rect.height) {
        pixmap.fill_rect(r, &paint, Transform::identity(), None);
    }
}

/// Shorthand to create DrawSettings.
fn draw_settings(font_size_px: f32) -> skrifa::outline::DrawSettings<'static> {
    let size = skrifa::instance::Size::new(font_size_px);
    skrifa::outline::DrawSettings::unhinted(size, skrifa::instance::LocationRef::default())
}

/// Draw a stroked rectangle with rounded corners.
fn stroke_rect(pixmap: &mut Pixmap, rect: &Rect, rgba: [u8; 4], width: f32) {
    let paint = make_paint(rgba);
    let stroke = make_stroke(width, None);
    let r = 3.0f32; // corner radius
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
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
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
) {
    let paint = make_paint(rgba);
    let stroke = make_stroke(width, dash);
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
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
                pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
            }
            HeadStyle::Open => {
                // Draw just the two lines of the V
                let mut pb2 = PathBuilder::new();
                pb2.move_to(tip_x + dx, tip_y - dy);
                pb2.line_to(tip_x, tip_y);
                pb2.line_to(tip_x + dx, tip_y + dy);
                if let Some(path2) = pb2.finish() {
                    let stroke = make_stroke(1.5, None);
                    pixmap.stroke_path(&path2, &paint, &stroke, Transform::identity(), None);
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
    fg: [u8; 4],
) {
    let white = [255u8, 255, 255, 255];
    let note_bg = [255u8, 255, 204, 255]; // #ffffcc

    // Title
    if let Some(ref t) = layout.title {
        render_text(font, pixmap, t, fg);
    }

    // Lifelines
    for ll in &layout.lifelines {
        draw_line(
            pixmap,
            ll.x,
            ll.y_start,
            ll.x,
            ll.y_end,
            fg,
            1.0,
            Some(&[6.0, 4.0]),
        );
    }

    // Actor boxes
    for a in &layout.actors {
        // Top box
        fill_rect(pixmap, &a.top_box, white);
        stroke_rect(pixmap, &a.top_box, fg, 1.5);
        render_text(font, pixmap, &a.top_label, fg);
        // Bottom box
        fill_rect(pixmap, &a.bottom_box, white);
        stroke_rect(pixmap, &a.bottom_box, fg, 1.5);
        render_text(font, pixmap, &a.bottom_label, fg);
    }

    // Messages
    for m in &layout.messages {
        let dash: Option<&[f32]> = match m.arrow.line_style {
            LineStyle::Solid => None,
            LineStyle::Dashed => Some(&[8.0, 4.0]),
        };

        if m.is_self {
            let x = m.from_x;
            let jog_x = x + 40.0;
            let y1 = m.y;
            let y2 = m.y + 30.0;
            // Draw the three segments of the self-loop
            draw_line(pixmap, x, y1, jog_x, y1, fg, 1.5, dash);
            draw_line(pixmap, jog_x, y1, jog_x, y2, fg, 1.5, dash);
            draw_line(pixmap, jog_x, y2, x, y2, fg, 1.5, dash);
            // Arrowhead pointing left at (x, y2)
            draw_arrowhead(pixmap, x, y2, false, m.arrow.head_style, fg);
        } else {
            let pointing_right = m.to_x > m.from_x;
            draw_line(pixmap, m.from_x, m.y, m.to_x, m.y, fg, 1.5, dash);
            draw_arrowhead(pixmap, m.to_x, m.y, pointing_right, m.arrow.head_style, fg);
        }

        // Message label
        render_text(font, pixmap, &m.label, fg);
    }

    // Notes
    for n in &layout.notes {
        fill_rect(pixmap, &n.rect, note_bg);
        stroke_rect(pixmap, &n.rect, fg, 1.0);
        render_text(font, pixmap, &n.text, fg);
    }
}
