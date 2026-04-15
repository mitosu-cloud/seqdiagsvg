use std::collections::HashMap;

use crate::ast::*;
use crate::font::DiagramFont;
use crate::parse;
use crate::SeqDiagramError;

// --- Layout constants ---

const ACTOR_BOX_PAD_X: f32 = 16.0;
const ACTOR_BOX_PAD_Y: f32 = 10.0;
const COLUMN_MIN_GAP: f32 = 150.0;
const MESSAGE_SPACING: f32 = 50.0;
const NOTE_PAD_X: f32 = 10.0;
const NOTE_PAD_Y: f32 = 8.0;
const SELF_MESSAGE_WIDTH: f32 = 40.0;
const SELF_MESSAGE_HEIGHT: f32 = 40.0;
const TITLE_MARGIN_BOTTOM: f32 = 16.0;

const ACTIVATION_BOX_WIDTH: f32 = 10.0;
const ACTIVATION_NESTING_OFFSET: f32 = 4.0;
const FRAME_PAD_X: f32 = 10.0;
const FRAME_PAD_Y: f32 = 8.0;
const FRAME_TAB_PAD_X: f32 = 8.0;
const FRAME_TAB_PAD_Y: f32 = 4.0;
const _FRAME_TAB_FOLD: f32 = 10.0;
const FRAME_SPACING: f32 = 10.0;
const DESTROY_MARKER_SIZE: f32 = 8.0;
const SLANT_PER_UNIT: f32 = 10.0;

// --- Layout types ---

#[derive(Debug, Clone, Copy)]
pub enum TextAnchor {
    Start,
    Middle,
}

#[derive(Debug, Clone)]
pub struct PositionedText {
    pub x: f32,
    pub y: f32,
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
    pub from_y: f32,
    pub to_y: f32,
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
pub struct ActivationBoxLayout {
    pub rect: Rect,
}

#[derive(Debug, Clone)]
pub struct FrameLayout {
    pub outer_rect: Rect,
    pub kind: FrameKind,
    pub tab_rect: Rect,
    pub tab_label: PositionedText,
    pub condition_label: Option<PositionedText>,
    pub else_dividers: Vec<FrameDivider>,
}

#[derive(Debug, Clone)]
pub struct FrameDivider {
    pub y: f32,
    pub x_start: f32,
    pub x_end: f32,
    pub label: Option<PositionedText>,
}

#[derive(Debug, Clone)]
pub struct DestroyMarkerLayout {
    pub center_x: f32,
    pub center_y: f32,
    pub size: f32,
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
    pub activation_boxes: Vec<ActivationBoxLayout>,
    pub frames: Vec<FrameLayout>,
    pub destroy_markers: Vec<DestroyMarkerLayout>,
}

// --- Tracking state for the layout walk ---

struct OpenFrame {
    kind: FrameKind,
    label: String,
    y_start: f32,
    nesting_depth: usize,
    else_dividers: Vec<(f32, String)>,
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
        return layout_empty(doc, font, font_size_px, padding);
    }

    let font_metrics = font.metrics(font_size_px);
    let ascent = font_metrics.ascent;
    let descent = font_metrics.descent;
    let line_height = ascent - descent;

    let actor_widths: Vec<f32> = actors_list
        .iter()
        .map(|(_, display)| font.text_width(display, font_size_px))
        .collect();
    let actor_box_height = line_height + 2.0 * ACTOR_BOX_PAD_Y;
    let actor_box_widths: Vec<f32> = actor_widths
        .iter()
        .map(|w| w + 2.0 * ACTOR_BOX_PAD_X)
        .collect();

    let mut actor_index: HashMap<&str, usize> = HashMap::new();
    for (i, (ref_name, display_name)) in actors_list.iter().enumerate() {
        actor_index.insert(ref_name.as_str(), i);
        if ref_name != display_name {
            actor_index.insert(display_name.as_str(), i);
        }
    }

    // Compute per-gap minimum widths
    let num_gaps = if actors_list.len() > 1 { actors_list.len() - 1 } else { 0 };
    let mut gap_widths = vec![COLUMN_MIN_GAP; num_gaps];

    for stmt in &doc.statements {
        if let Statement::Message { from, to, text, .. } = stmt {
            if let (Some(&fi), Some(&ti)) = (actor_index.get(from.as_str()), actor_index.get(to.as_str())) {
                if fi != ti {
                    let left = fi.min(ti);
                    let right = fi.max(ti);
                    let label_w = font.text_block_width(text, font_size_px) + 20.0;
                    let span = right - left;
                    let per_gap = label_w / span as f32;
                    for g in left..right {
                        gap_widths[g] = gap_widths[g].max(per_gap);
                    }
                }
            }
        }
    }

    for (i, gap) in gap_widths.iter_mut().enumerate() {
        let left_half = actor_box_widths[i] / 2.0;
        let right_half = actor_box_widths[i + 1] / 2.0;
        let min_for_boxes = left_half + right_half + 20.0;
        *gap = gap.max(min_for_boxes);
    }

    let mut centers: Vec<f32> = Vec::with_capacity(actors_list.len());
    let first_center = padding + actor_box_widths[0] / 2.0;
    centers.push(first_center);
    for (i, gap) in gap_widths.iter().enumerate() {
        centers.push(centers[i] + gap);
    }

    let mut y_cursor = padding;

    // Title
    let mut title_layout = {
        let mut title = None;
        for stmt in &doc.statements {
            if let Statement::Title(text) = stmt {
                let title_font_size = font_size_px * 1.2;
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
            let top_box = Rect { x: cx - bw / 2.0, y: top_box_y, width: bw, height: actor_box_height };
            let label_y = top_box_y + ACTOR_BOX_PAD_Y + ascent;
            ActorLayout {
                name: name.clone(),
                display_name: display.clone(),
                center_x: cx,
                top_box,
                bottom_box: top_box,
                top_label: PositionedText { x: cx, y: label_y, text: display.clone(), font_size_px, anchor: TextAnchor::Middle },
                bottom_label: PositionedText { x: cx, y: label_y, text: display.clone(), font_size_px, anchor: TextAnchor::Middle },
            }
        })
        .collect();

    y_cursor = top_box_y + actor_box_height + MESSAGE_SPACING;

    // --- Walk statements ---
    let mut messages = Vec::new();
    let mut notes = Vec::new();
    let mut activation_boxes = Vec::new();
    let mut frames = Vec::new();
    let mut destroy_markers = Vec::new();

    // Activation tracking: actor_index -> stack of y_start values
    let mut active_stack: HashMap<usize, Vec<f32>> = HashMap::new();
    let mut destroyed: std::collections::HashSet<usize> = std::collections::HashSet::new();

    // Frame tracking
    let mut frame_stack: Vec<OpenFrame> = Vec::new();

    // Compute frame x extents
    let frame_left = centers[0] - actor_box_widths[0] / 2.0 - FRAME_PAD_X;
    let frame_right = *centers.last().unwrap() + actor_box_widths.last().unwrap() / 2.0 + FRAME_PAD_X;

    // Small font for frame labels
    let frame_font_size = font_size_px * 0.85;
    let frame_metrics = font.metrics(frame_font_size);
    let frame_line_height = frame_metrics.ascent - frame_metrics.descent;

    for stmt in &doc.statements {
        match stmt {
            Statement::Message { from, to, arrow, text, activation, delay } => {
                let fi = actor_index.get(from.as_str()).copied();
                let ti = actor_index.get(to.as_str()).copied();
                if let (Some(fi), Some(ti)) = (fi, ti) {
                    let is_self = fi == ti;
                    let from_x = centers[fi];
                    let to_x = centers[ti];
                    let num_lines = text.split('\n').count().max(1);
                    let extra_lines = (num_lines - 1) as f32;
                    let delay_drop = delay.map(|d| d as f32 * SLANT_PER_UNIT).unwrap_or(0.0);

                    if is_self {
                        let label_x = from_x + SELF_MESSAGE_WIDTH + 8.0;
                        let from_y = y_cursor;
                        let to_y = from_y; // self-messages don't slant
                        messages.push(MessageLayout {
                            from_x, to_x, from_y, to_y,
                            arrow: *arrow,
                            label: PositionedText { x: label_x, y: y_cursor + ascent / 2.0, text: text.clone(), font_size_px, anchor: TextAnchor::Start },
                            is_self: true,
                        });
                        let self_height = SELF_MESSAGE_HEIGHT.max((num_lines as f32) * line_height);
                        y_cursor += self_height + MESSAGE_SPACING;
                    } else {
                        let extra_height = extra_lines * line_height;
                        y_cursor += extra_height;
                        let from_y = y_cursor;
                        let to_y = from_y + delay_drop;
                        let mid_x = (from_x + to_x) / 2.0;
                        // For slanted arrows, place the label above the higher endpoint
                        // so text doesn't cross the diagonal line.
                        let label_ref_y = from_y.min(to_y);
                        let label_y = label_ref_y - descent.abs() - 4.0 - extra_height;
                        messages.push(MessageLayout {
                            from_x, to_x, from_y, to_y,
                            arrow: *arrow,
                            label: PositionedText { x: mid_x, y: label_y, text: text.clone(), font_size_px, anchor: TextAnchor::Middle },
                            is_self: false,
                        });
                        y_cursor = to_y.max(from_y) + MESSAGE_SPACING;
                    }

                    // Handle activation modifier on target actor
                    if let Some(act_mod) = activation {
                        match act_mod {
                            ActivationModifier::Activate => {
                                active_stack.entry(ti).or_default().push(y_cursor - MESSAGE_SPACING);
                            }
                            ActivationModifier::Deactivate => {
                                if let Some(stack) = active_stack.get_mut(&ti) {
                                    if let Some(y_start) = stack.pop() {
                                        let nesting = stack.len() as f32;
                                        let cx = centers[ti];
                                        activation_boxes.push(ActivationBoxLayout {
                                            rect: Rect {
                                                x: cx - ACTIVATION_BOX_WIDTH / 2.0 + nesting * ACTIVATION_NESTING_OFFSET,
                                                y: y_start,
                                                width: ACTIVATION_BOX_WIDTH,
                                                height: (y_cursor - MESSAGE_SPACING) - y_start,
                                            },
                                        });
                                    }
                                }
                            }
                        }
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

                notes.push(NoteLayout {
                    rect: Rect { x: note_x, y: y_cursor, width: final_w, height: note_h },
                    text: PositionedText { x: note_x + NOTE_PAD_X, y: y_cursor + NOTE_PAD_Y + ascent, text: text.clone(), font_size_px, anchor: TextAnchor::Start },
                });
                y_cursor += note_h + MESSAGE_SPACING;
            }
            Statement::Activate(actor) => {
                if let Some(&idx) = actor_index.get(actor.as_str()) {
                    active_stack.entry(idx).or_default().push(y_cursor);
                }
            }
            Statement::Deactivate(actor) => {
                if let Some(&idx) = actor_index.get(actor.as_str()) {
                    if let Some(stack) = active_stack.get_mut(&idx) {
                        if let Some(y_start) = stack.pop() {
                            let nesting = stack.len() as f32;
                            let cx = centers[idx];
                            activation_boxes.push(ActivationBoxLayout {
                                rect: Rect {
                                    x: cx - ACTIVATION_BOX_WIDTH / 2.0 + nesting * ACTIVATION_NESTING_OFFSET,
                                    y: y_start,
                                    width: ACTIVATION_BOX_WIDTH,
                                    height: y_cursor - y_start,
                                },
                            });
                        }
                    }
                }
            }
            Statement::Destroy(actor) => {
                if let Some(&idx) = actor_index.get(actor.as_str()) {
                    // Close all activations for this actor
                    if let Some(stack) = active_stack.remove(&idx) {
                        for (nesting, y_start) in stack.iter().enumerate().rev() {
                            let cx = centers[idx];
                            activation_boxes.push(ActivationBoxLayout {
                                rect: Rect {
                                    x: cx - ACTIVATION_BOX_WIDTH / 2.0 + nesting as f32 * ACTIVATION_NESTING_OFFSET,
                                    y: *y_start,
                                    width: ACTIVATION_BOX_WIDTH,
                                    height: y_cursor - y_start,
                                },
                            });
                        }
                    }
                    destroy_markers.push(DestroyMarkerLayout {
                        center_x: centers[idx],
                        center_y: y_cursor,
                        size: DESTROY_MARKER_SIZE,
                    });
                    destroyed.insert(idx);
                    y_cursor += MESSAGE_SPACING;
                }
            }
            Statement::FrameOpen { kind, label } => {
                let nesting_depth = frame_stack.len();
                let inset = nesting_depth as f32 * FRAME_PAD_X;
                let fx = frame_left + inset;
                let fw = (frame_right - inset) - fx;

                // Leave space for the tab
                let tab_text = kind.to_string();
                let tab_text_w = font.text_width(&tab_text, frame_font_size);
                let tab_w = tab_text_w + 2.0 * FRAME_TAB_PAD_X;
                let tab_h = frame_line_height + 2.0 * FRAME_TAB_PAD_Y;

                // We'll record the tab dimensions for later; actual rect computed at FrameEnd
                frame_stack.push(OpenFrame {
                    kind: *kind,
                    label: label.clone(),
                    y_start: y_cursor,
                    nesting_depth,
                    else_dividers: Vec::new(),
                });

                // Advance past the tab. Message labels float above the message line
                // (by roughly line_height), so we need enough space that the label top
                // clears the tab bottom.
                y_cursor += tab_h + line_height + FRAME_PAD_Y;
                // Ignore tab_w/fw here; they're used at FrameEnd
                let _ = (tab_w, fw);
            }
            Statement::FrameElse { label } => {
                if let Some(open_frame) = frame_stack.last_mut() {
                    open_frame.else_dividers.push((y_cursor, label.clone()));
                    // Advance enough so the next message label clears the divider label
                    let else_advance = if label.is_empty() {
                        FRAME_SPACING
                    } else {
                        frame_line_height + line_height
                    };
                    y_cursor += else_advance;
                }
            }
            Statement::FrameEnd => {
                if let Some(open_frame) = frame_stack.pop() {
                    y_cursor += FRAME_PAD_Y;
                    let inset = open_frame.nesting_depth as f32 * FRAME_PAD_X;
                    let fx = frame_left + inset;
                    let fw = (frame_right - inset) - fx;

                    let tab_text = open_frame.kind.to_string();
                    let tab_text_w = font.text_width(&tab_text, frame_font_size);
                    let tab_w = tab_text_w + 2.0 * FRAME_TAB_PAD_X;
                    let tab_h = frame_line_height + 2.0 * FRAME_TAB_PAD_Y;

                    let outer_rect = Rect { x: fx, y: open_frame.y_start, width: fw, height: y_cursor - open_frame.y_start };
                    let tab_rect = Rect { x: fx, y: open_frame.y_start, width: tab_w, height: tab_h };

                    let tab_label = PositionedText {
                        x: fx + FRAME_TAB_PAD_X,
                        y: open_frame.y_start + FRAME_TAB_PAD_Y + frame_metrics.ascent,
                        text: tab_text,
                        font_size_px: frame_font_size,
                        anchor: TextAnchor::Start,
                    };

                    let condition_label = if !open_frame.label.is_empty() {
                        Some(PositionedText {
                            x: fx + tab_w + FRAME_TAB_PAD_X,
                            y: open_frame.y_start + FRAME_TAB_PAD_Y + frame_metrics.ascent,
                            text: open_frame.label.clone(),
                            font_size_px: frame_font_size,
                            anchor: TextAnchor::Start,
                        })
                    } else {
                        None
                    };

                    let else_dividers = open_frame.else_dividers.iter().map(|(div_y, div_label)| {
                        let label = if !div_label.is_empty() {
                            Some(PositionedText {
                                x: fx + FRAME_TAB_PAD_X,
                                y: *div_y + frame_metrics.ascent,
                                text: format!("[{}]", div_label),
                                font_size_px: frame_font_size,
                                anchor: TextAnchor::Start,
                            })
                        } else {
                            None
                        };
                        FrameDivider { y: *div_y, x_start: fx, x_end: fx + fw, label }
                    }).collect();

                    frames.push(FrameLayout {
                        outer_rect, kind: open_frame.kind, tab_rect, tab_label, condition_label, else_dividers,
                    });
                    y_cursor += FRAME_SPACING;
                }
            }
            Statement::Title(_) | Statement::Participant { .. } => {}
        }
    }

    // Force-close any unclosed activations at diagram bottom
    let force_close_y = y_cursor;
    for (idx, stack) in active_stack.drain() {
        for (nesting, y_start) in stack.iter().enumerate() {
            let cx = centers[idx];
            activation_boxes.push(ActivationBoxLayout {
                rect: Rect {
                    x: cx - ACTIVATION_BOX_WIDTH / 2.0 + nesting as f32 * ACTIVATION_NESTING_OFFSET,
                    y: *y_start,
                    width: ACTIVATION_BOX_WIDTH,
                    height: force_close_y - y_start,
                },
            });
        }
    }

    // Bottom actor boxes
    y_cursor += MESSAGE_SPACING / 2.0;
    let bottom_box_y = y_cursor;

    let mut actor_layouts = actor_layouts;
    for (i, al) in actor_layouts.iter_mut().enumerate() {
        if destroyed.contains(&i) {
            // Destroyed actors don't get a bottom box — hide it
            al.bottom_box = Rect { x: al.top_box.x, y: bottom_box_y, width: al.top_box.width, height: 0.0 };
            al.bottom_label = PositionedText { x: al.center_x, y: bottom_box_y, text: String::new(), font_size_px, anchor: TextAnchor::Middle };
        } else {
            al.bottom_box = Rect { x: al.top_box.x, y: bottom_box_y, width: al.top_box.width, height: actor_box_height };
            al.bottom_label = PositionedText { x: al.center_x, y: bottom_box_y + ACTOR_BOX_PAD_Y + ascent, text: al.display_name.clone(), font_size_px, anchor: TextAnchor::Middle };
        }
    }

    // --- Bounds check ---
    let min_left = notes
        .iter()
        .map(|n| n.rect.x)
        .chain(messages.iter().map(|m| {
            if m.is_self { m.label.x } else {
                let half_w = font.text_block_width(&m.label.text, m.label.font_size_px) / 2.0;
                m.label.x - half_w
            }
        }))
        .chain(frames.iter().map(|f| f.outer_rect.x))
        .fold(f32::MAX, f32::min);

    let shift = if min_left < padding { padding - min_left } else { 0.0 };

    if shift > 0.0 {
        if let Some(ref mut t) = title_layout { t.x += shift; }
        for al in actor_layouts.iter_mut() {
            al.center_x += shift;
            al.top_box.x += shift;
            al.bottom_box.x += shift;
            al.top_label.x += shift;
            al.bottom_label.x += shift;
        }
        for m in messages.iter_mut() { m.from_x += shift; m.to_x += shift; m.label.x += shift; }
        for n in notes.iter_mut() { n.rect.x += shift; n.text.x += shift; }
        for ab in activation_boxes.iter_mut() { ab.rect.x += shift; }
        for f in frames.iter_mut() {
            f.outer_rect.x += shift;
            f.tab_rect.x += shift;
            f.tab_label.x += shift;
            if let Some(ref mut c) = f.condition_label { c.x += shift; }
            for d in f.else_dividers.iter_mut() {
                d.x_start += shift;
                d.x_end += shift;
                if let Some(ref mut l) = d.label { l.x += shift; }
            }
        }
        for dm in destroy_markers.iter_mut() { dm.center_x += shift; }
    }

    // Recompute lifeline x after shift
    let lifelines: Vec<LifelineLayout> = actor_layouts
        .iter()
        .enumerate()
        .map(|(i, al)| {
            let y_end = if destroyed.contains(&i) {
                destroy_markers.iter()
                    .find(|dm| (dm.center_x - al.center_x).abs() < 1.0)
                    .map(|dm| dm.center_y)
                    .unwrap_or(al.bottom_box.y)
            } else {
                al.bottom_box.y
            };
            LifelineLayout { x: al.center_x, y_start: al.top_box.y + al.top_box.height, y_end }
        })
        .collect();

    // Final dimensions
    let base_width = *centers.last().unwrap() + actor_box_widths.last().unwrap() / 2.0 + padding + shift;
    let max_bottom = actor_layouts.iter().map(|al| al.bottom_box.y + al.bottom_box.height).fold(0.0f32, f32::max);
    let total_height = max_bottom + padding;

    let max_right = notes
        .iter()
        .map(|n| n.rect.x + n.rect.width)
        .chain(messages.iter().map(|m| {
            if m.is_self { m.label.x + font.text_block_width(&m.label.text, m.label.font_size_px) }
            else { let half_w = font.text_block_width(&m.label.text, m.label.font_size_px) / 2.0; m.label.x + half_w }
        }))
        .chain(frames.iter().map(|f| f.outer_rect.x + f.outer_rect.width))
        .chain(std::iter::once(base_width))
        .fold(0.0f32, f32::max);
    let total_width = base_width.max(max_right + padding);

    // Reverse frames so outer frames render first (bottom), inner frames on top
    frames.reverse();

    Ok(DiagramLayout {
        width: total_width,
        height: total_height,
        title: title_layout,
        actors: actor_layouts,
        messages,
        notes,
        lifelines,
        activation_boxes,
        frames,
        destroy_markers,
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
        activation_boxes: vec![],
        frames: vec![],
        destroy_markers: vec![],
    })
}
