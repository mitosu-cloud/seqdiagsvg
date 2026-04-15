use crate::ast::*;
use crate::font::DiagramFont;
use crate::parse;
use crate::SeqDiagramError;

// --- Layout constants (proportional to font_size_px) ---

const ACTOR_BOX_PAD_X: f32 = 16.0;
const ACTOR_BOX_PAD_Y: f32 = 10.0;
const COLUMN_MIN_GAP: f32 = 150.0;
const MESSAGE_SPACING: f32 = 50.0;
const NOTE_PAD_X: f32 = 10.0;
const NOTE_PAD_Y: f32 = 8.0;
const SELF_MESSAGE_WIDTH: f32 = 40.0;
const SELF_MESSAGE_HEIGHT: f32 = 40.0;
const TITLE_MARGIN_BOTTOM: f32 = 16.0;

// --- Layout types ---

#[derive(Debug, Clone, Copy)]
pub enum TextAnchor {
    Start,
    Middle,
}

#[derive(Debug, Clone)]
pub struct PositionedText {
    pub x: f32,
    pub y: f32, // baseline y
    pub text: String,
    pub font_size_px: f32,
    pub anchor: TextAnchor,
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct ActorLayout {
    pub name: String,
    pub display_name: String,
    pub center_x: f32,
    pub top_box: Rect,
    pub bottom_box: Rect,
    pub top_label: PositionedText,
    pub bottom_label: PositionedText,
}

#[derive(Debug, Clone)]
pub struct MessageLayout {
    pub from_x: f32,
    pub to_x: f32,
    pub y: f32,
    pub arrow: Arrow,
    pub label: PositionedText,
    pub is_self: bool,
}

#[derive(Debug, Clone)]
pub struct NoteLayout {
    pub rect: Rect,
    pub text: PositionedText,
}

#[derive(Debug, Clone)]
pub struct LifelineLayout {
    pub x: f32,
    pub y_start: f32,
    pub y_end: f32,
}

#[derive(Debug, Clone)]
pub struct DiagramLayout {
    pub width: f32,
    pub height: f32,
    pub title: Option<PositionedText>,
    pub actors: Vec<ActorLayout>,
    pub messages: Vec<MessageLayout>,
    pub notes: Vec<NoteLayout>,
    pub lifelines: Vec<LifelineLayout>,
}

/// Lay out a sequence diagram document.
pub fn layout_diagram(
    font: &DiagramFont,
    doc: &Document,
    font_size_px: f32,
    padding: f32,
) -> Result<DiagramLayout, SeqDiagramError> {
    let actors_list = parse::resolve_actors(doc);
    if actors_list.is_empty() {
        // Might just be a title or empty doc
        return layout_empty(doc, font, font_size_px, padding);
    }

    let font_metrics = font.metrics(font_size_px);
    let ascent = font_metrics.ascent;
    let descent = font_metrics.descent;
    let line_height = ascent - descent;

    // Measure actor label widths
    let actor_widths: Vec<f32> = actors_list
        .iter()
        .map(|(_, display)| font.text_width(display, font_size_px))
        .collect();

    let actor_box_height = line_height + 2.0 * ACTOR_BOX_PAD_Y;

    // Compute minimum column widths based on actor labels
    let actor_box_widths: Vec<f32> = actor_widths
        .iter()
        .map(|w| w + 2.0 * ACTOR_BOX_PAD_X)
        .collect();

    // Build actor name -> index mapping (both reference name and display name)
    let mut actor_index: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (i, (ref_name, display_name)) in actors_list.iter().enumerate() {
        actor_index.insert(ref_name.as_str(), i);
        if ref_name != display_name {
            actor_index.insert(display_name.as_str(), i);
        }
    }

    // Compute per-gap minimum widths based on message labels between adjacent columns
    let num_gaps = if actors_list.len() > 1 {
        actors_list.len() - 1
    } else {
        0
    };
    let mut gap_widths = vec![COLUMN_MIN_GAP; num_gaps];

    for stmt in &doc.statements {
        if let Statement::Message { from, to, .. } = stmt {
            if let (Some(&fi), Some(&ti)) = (actor_index.get(from.as_str()), actor_index.get(to.as_str())) {
                if fi != ti {
                    let left = fi.min(ti);
                    let right = fi.max(ti);
                    let label_text = match stmt {
                        Statement::Message { text, .. } => text,
                        _ => unreachable!(),
                    };
                    let label_w = font.text_block_width(label_text, font_size_px) + 20.0; // padding
                    // Distribute across gaps between left..right
                    let span = right - left;
                    let per_gap = label_w / span as f32;
                    for g in left..right {
                        gap_widths[g] = gap_widths[g].max(per_gap);
                    }
                }
            }
        }
    }

    // Ensure gaps are wide enough for actor boxes on either side
    for (i, gap) in gap_widths.iter_mut().enumerate() {
        let left_half = actor_box_widths[i] / 2.0;
        let right_half = actor_box_widths[i + 1] / 2.0;
        let min_for_boxes = left_half + right_half + 20.0;
        *gap = gap.max(min_for_boxes);
    }

    // Compute actor center x positions
    let mut centers: Vec<f32> = Vec::with_capacity(actors_list.len());
    let first_center = padding + actor_box_widths[0] / 2.0;
    centers.push(first_center);
    for (i, gap) in gap_widths.iter().enumerate() {
        centers.push(centers[i] + gap);
    }

    // Compute y cursor
    let mut y_cursor = padding;

    // Title
    let mut title_layout = {
        let mut title = None;
        for stmt in &doc.statements {
            if let Statement::Title(text) = stmt {
                let title_font_size = font_size_px * 1.2;
                let _tw = font.text_width(text, title_font_size);
                let total_w = *centers.last().unwrap() + actor_box_widths.last().unwrap() / 2.0 + padding;
                let tx = total_w / 2.0;
                let title_ascent = font.metrics(title_font_size).ascent;
                title = Some(PositionedText {
                    x: tx,
                    y: y_cursor + title_ascent,
                    text: text.clone(),
                    font_size_px: title_font_size,
                    anchor: TextAnchor::Middle,
                });
                let title_metrics = font.metrics(title_font_size);
                let title_height = title_metrics.ascent - title_metrics.descent;
                y_cursor += title_height + TITLE_MARGIN_BOTTOM;
                break;
            }
        }
        title
    };

    // Top actor boxes
    let top_box_y = y_cursor;
    let actor_layouts: Vec<ActorLayout> = actors_list
        .iter()
        .enumerate()
        .map(|(i, (name, display))| {
            let cx = centers[i];
            let bw = actor_box_widths[i];
            let top_box = Rect {
                x: cx - bw / 2.0,
                y: top_box_y,
                width: bw,
                height: actor_box_height,
            };
            let label_y = top_box_y + ACTOR_BOX_PAD_Y + ascent;
            ActorLayout {
                name: name.clone(),
                display_name: display.clone(),
                center_x: cx,
                top_box,
                bottom_box: top_box, // placeholder, updated later
                top_label: PositionedText {
                    x: cx,
                    y: label_y,
                    text: display.clone(),
                    font_size_px,
                    anchor: TextAnchor::Middle,
                },
                bottom_label: PositionedText {
                    x: cx,
                    y: label_y, // placeholder
                    text: display.clone(),
                    font_size_px,
                    anchor: TextAnchor::Middle,
                },
            }
        })
        .collect();

    y_cursor = top_box_y + actor_box_height + MESSAGE_SPACING;

    // Walk statements and place messages/notes
    let mut messages = Vec::new();
    let mut notes = Vec::new();

    for stmt in &doc.statements {
        match stmt {
            Statement::Message {
                from,
                to,
                arrow,
                text,
            } => {
                let fi = actor_index.get(from.as_str()).copied();
                let ti = actor_index.get(to.as_str()).copied();
                if let (Some(fi), Some(ti)) = (fi, ti) {
                    let is_self = fi == ti;
                    let from_x = centers[fi];
                    let to_x = centers[ti];

                    let _label_w = font.text_block_width(text, font_size_px);
                    let num_lines = text.split('\n').count().max(1);
                    let extra_lines = (num_lines - 1) as f32;

                    if is_self {
                        // Self-message: label to the right of the jog
                        let label_x = from_x + SELF_MESSAGE_WIDTH + 8.0;
                        messages.push(MessageLayout {
                            from_x,
                            to_x,
                            y: y_cursor,
                            arrow: *arrow,
                            label: PositionedText {
                                x: label_x,
                                y: y_cursor + ascent / 2.0,
                                text: text.clone(),
                                font_size_px,
                                anchor: TextAnchor::Start,
                            },
                            is_self: true,
                        });
                        let self_height = SELF_MESSAGE_HEIGHT.max((num_lines as f32) * line_height);
                        y_cursor += self_height + MESSAGE_SPACING;
                    } else {
                        // Normal message: label centered above arrow
                        // For multi-line, shift label up so bottom line sits above arrow
                        let extra_height = extra_lines * line_height;
                        y_cursor += extra_height; // push arrow down to make room
                        let mid_x = (from_x + to_x) / 2.0;
                        let label_y = y_cursor - descent.abs() - 4.0 - extra_height;
                        messages.push(MessageLayout {
                            from_x,
                            to_x,
                            y: y_cursor,
                            arrow: *arrow,
                            label: PositionedText {
                                x: mid_x,
                                y: label_y,
                                text: text.clone(),
                                font_size_px,
                                anchor: TextAnchor::Middle,
                            },
                            is_self: false,
                        });
                        y_cursor += MESSAGE_SPACING;
                    }
                }
            }
            Statement::Note { position, text } => {
                let note_lines = text.split('\n').count().max(1);
                let note_w = font.text_block_width(text, font_size_px) + 2.0 * NOTE_PAD_X;
                let note_h = (line_height * note_lines as f32) + 2.0 * NOTE_PAD_Y;

                let (note_x, final_w) = match position {
                    NotePosition::LeftOf(actor) => {
                        let idx = actor_index.get(actor.as_str()).copied().unwrap_or(0);
                        let x = (centers[idx] - note_w - 10.0).max(padding);
                        (x, note_w)
                    }
                    NotePosition::RightOf(actor) => {
                        let idx = actor_index.get(actor.as_str()).copied().unwrap_or(0);
                        (centers[idx] + 10.0, note_w)
                    }
                    NotePosition::Over(actor) => {
                        let idx = actor_index.get(actor.as_str()).copied().unwrap_or(0);
                        (centers[idx] - note_w / 2.0, note_w)
                    }
                    NotePosition::OverBetween(a, b) => {
                        let ai = actor_index.get(a.as_str()).copied().unwrap_or(0);
                        let bi = actor_index.get(b.as_str()).copied().unwrap_or(0);
                        let mid = (centers[ai] + centers[bi]) / 2.0;
                        let span_w = (centers[ai] - centers[bi]).abs() + 20.0;
                        let note_w_actual = note_w.max(span_w);
                        (mid - note_w_actual / 2.0, note_w_actual)
                    }
                };

                let note_rect = Rect {
                    x: note_x,
                    y: y_cursor,
                    width: final_w,
                    height: note_h,
                };

                notes.push(NoteLayout {
                    rect: note_rect,
                    text: PositionedText {
                        x: note_x + NOTE_PAD_X,
                        y: y_cursor + NOTE_PAD_Y + ascent,
                        text: text.clone(),
                        font_size_px,
                        anchor: TextAnchor::Start,
                    },
                });

                y_cursor += note_h + MESSAGE_SPACING;
            }
            Statement::Title(_) | Statement::Participant { .. } => {
                // Already handled
            }
        }
    }

    // Bottom actor boxes
    y_cursor += MESSAGE_SPACING / 2.0;
    let bottom_box_y = y_cursor;

    let mut actor_layouts = actor_layouts;
    for al in actor_layouts.iter_mut() {
        al.bottom_box = Rect {
            x: al.top_box.x,
            y: bottom_box_y,
            width: al.top_box.width,
            height: actor_box_height,
        };
        al.bottom_label = PositionedText {
            x: al.center_x,
            y: bottom_box_y + ACTOR_BOX_PAD_Y + ascent,
            text: al.display_name.clone(),
            font_size_px,
            anchor: TextAnchor::Middle,
        };
    }

    // Lifelines
    let mut lifelines: Vec<LifelineLayout> = actor_layouts
        .iter()
        .map(|al| LifelineLayout {
            x: al.center_x,
            y_start: al.top_box.y + al.top_box.height,
            y_end: al.bottom_box.y,
        })
        .collect();

    // --- Bounds check: find leftmost and rightmost extents of all elements ---

    // Compute the leftmost x of any element (notes, message labels)
    let min_left = notes
        .iter()
        .map(|n| n.rect.x)
        .chain(messages.iter().map(|m| {
            if m.is_self {
                m.label.x
            } else {
                let half_w = font.text_block_width(&m.label.text, m.label.font_size_px) / 2.0;
                m.label.x - half_w
            }
        }))
        .fold(f32::MAX, f32::min);

    // If anything extends left of the padding boundary, shift everything right
    let shift = if min_left < padding {
        padding - min_left
    } else {
        0.0
    };

    if shift > 0.0 {
        // Shift all x positions
        if let Some(ref mut t) = title_layout {
            t.x += shift;
        }
        for al in actor_layouts.iter_mut() {
            al.center_x += shift;
            al.top_box.x += shift;
            al.bottom_box.x += shift;
            al.top_label.x += shift;
            al.bottom_label.x += shift;
        }
        for m in messages.iter_mut() {
            m.from_x += shift;
            m.to_x += shift;
            m.label.x += shift;
        }
        for n in notes.iter_mut() {
            n.rect.x += shift;
            n.text.x += shift;
        }
        for ll in lifelines.iter_mut() {
            ll.x += shift;
        }
    }

    // Final dimensions
    let base_width = *centers.last().unwrap() + actor_box_widths.last().unwrap() / 2.0 + padding + shift;
    let total_height = bottom_box_y + actor_box_height + padding;

    // Ensure nothing bleeds past the right edge
    let max_right = notes
        .iter()
        .map(|n| n.rect.x + n.rect.width)
        .chain(messages.iter().map(|m| {
            if m.is_self {
                m.label.x + font.text_block_width(&m.label.text, m.label.font_size_px)
            } else {
                let half_w = font.text_block_width(&m.label.text, m.label.font_size_px) / 2.0;
                m.label.x + half_w
            }
        }))
        .chain(std::iter::once(base_width))
        .fold(0.0f32, f32::max);
    let total_width = base_width.max(max_right + padding);

    Ok(DiagramLayout {
        width: total_width,
        height: total_height,
        title: title_layout,
        actors: actor_layouts,
        messages,
        notes,
        lifelines,
    })
}

fn layout_empty(
    doc: &Document,
    font: &DiagramFont,
    font_size_px: f32,
    padding: f32,
) -> Result<DiagramLayout, SeqDiagramError> {
    let mut title = None;
    let mut h = padding;
    for stmt in &doc.statements {
        if let Statement::Title(text) = stmt {
            let title_font_size = font_size_px * 1.2;
            let tw = font.text_width(text, title_font_size);
            let ascent = font.metrics(title_font_size).ascent;
            title = Some(PositionedText {
                x: padding + tw / 2.0,
                y: padding + ascent,
                text: text.clone(),
                font_size_px: title_font_size,
                anchor: TextAnchor::Middle,
            });
            let metrics = font.metrics(title_font_size);
            h += metrics.ascent - metrics.descent + TITLE_MARGIN_BOTTOM;
            break;
        }
    }
    h += padding;
    let w = title
        .as_ref()
        .map(|t| font.text_width(&t.text, t.font_size_px) + 2.0 * padding)
        .unwrap_or(2.0 * padding);

    Ok(DiagramLayout {
        width: w,
        height: h,
        title,
        actors: vec![],
        messages: vec![],
        notes: vec![],
        lifelines: vec![],
    })
}
