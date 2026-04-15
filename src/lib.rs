pub mod ast;
mod font;
mod layout;
mod parse;
mod png_render;
mod svg_render;

pub use ast::{
    ActivationModifier, Arrow, Document, FrameKind, HeadStyle, LineStyle, NotePosition, Statement,
};
pub use parse::{parse_document, resolve_actors};

/// Style configuration for stroke widths, corner radii, and dash patterns.
pub struct StyleConfig {
    /// Stroke width for message arrows (default: 1.5)
    pub arrow_stroke_width: f32,
    /// Stroke width for actor box outlines (default: 1.5)
    pub actor_box_stroke_width: f32,
    /// Stroke width for lifelines (default: 1.0)
    pub lifeline_stroke_width: f32,
    /// Stroke width for note outlines (default: 1.0)
    pub note_stroke_width: f32,
    /// Corner radius for actor boxes (default: 3.0)
    pub actor_box_corner_radius: f32,
    /// Corner radius for note boxes (default: 2.0)
    pub note_corner_radius: f32,
    /// Dash pattern for lifelines [dash, gap] (default: [6.0, 4.0])
    pub lifeline_dash: [f32; 2],
    /// Dash pattern for dashed messages [dash, gap] (default: [8.0, 4.0])
    pub message_dash: [f32; 2],
    /// Stroke width for activation box outlines (default: 1.0)
    pub activation_stroke_width: f32,
    /// Stroke width for frame outlines (default: 1.0)
    pub frame_stroke_width: f32,
    /// Corner radius for frame rectangles (default: 3.0)
    pub frame_corner_radius: f32,
    /// Dash pattern for frame else dividers [dash, gap] (default: [6.0, 4.0])
    pub frame_else_dash: [f32; 2],
    /// Stroke width for destroy X markers (default: 2.0)
    pub destroy_stroke_width: f32,
    /// Arrowhead marker size (default: 12.0)
    pub marker_size: f32,
}

impl Default for StyleConfig {
    fn default() -> Self {
        Self {
            arrow_stroke_width: 1.5,
            actor_box_stroke_width: 1.5,
            lifeline_stroke_width: 1.0,
            note_stroke_width: 1.0,
            actor_box_corner_radius: 3.0,
            note_corner_radius: 2.0,
            lifeline_dash: [6.0, 4.0],
            message_dash: [8.0, 4.0],
            activation_stroke_width: 1.0,
            frame_stroke_width: 1.0,
            frame_corner_radius: 3.0,
            frame_else_dash: [6.0, 4.0],
            destroy_stroke_width: 2.0,
            marker_size: 12.0,
        }
    }
}

/// Render options for sequence diagrams.
pub struct RenderOptions {
    /// Font size in points (default: 14.0)
    pub font_size_pt: f32,
    /// Pixels per point / scale factor (default: 2.0 for retina)
    pub scale: f32,
    /// RGBA foreground color (default: dark gray [0x33, 0x33, 0x33, 0xFF])
    pub fg_color: [u8; 4],
    /// RGBA background color (default: white [0xFF, 0xFF, 0xFF, 0xFF])
    pub bg_color: [u8; 4],
    /// RGBA note background color (default: yellow [0xFF, 0xFF, 0xCC, 0xFF])
    pub note_color: [u8; 4],
    /// RGBA actor box fill color (default: white [0xFF, 0xFF, 0xFF, 0xFF])
    pub actor_fill: [u8; 4],
    /// RGBA actor box text color — used for labels inside actor boxes.
    /// If different from fg_color, ensures readability against actor_fill.
    /// (default: dark gray [0x33, 0x33, 0x33, 0xFF])
    pub actor_text_color: [u8; 4],
    /// RGBA note text color — used for text inside note boxes.
    /// (default: dark gray [0x33, 0x33, 0x33, 0xFF])
    pub note_text_color: [u8; 4],
    /// Padding in pixels around the diagram
    pub padding: u32,
    /// Optional path to an additional system font (OTF/TTF).
    /// Tried after the primary font (Inter) but before the embedded fallback.
    pub system_font: Option<String>,
    /// Style configuration for strokes, radii, and dash patterns.
    pub style: StyleConfig,
    /// Maximum width in pixels. If the diagram exceeds this, it scales down uniformly.
    pub max_width: Option<f32>,
    /// Maximum height in pixels. If the diagram exceeds this, it scales down uniformly.
    pub max_height: Option<f32>,
    /// RGBA activation box fill color (default: white)
    pub activation_fill: [u8; 4],
    /// RGBA frame background fill color (default: nearly transparent)
    pub frame_fill: [u8; 4],
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            font_size_pt: 14.0,
            scale: 2.0,
            fg_color: [0x33, 0x33, 0x33, 0xFF],
            bg_color: [0xFF, 0xFF, 0xFF, 0xFF],
            note_color: [0xFF, 0xFF, 0xCC, 0xFF],
            actor_fill: [0xFF, 0xFF, 0xFF, 0xFF],
            actor_text_color: [0x33, 0x33, 0x33, 0xFF],
            note_text_color: [0x33, 0x33, 0x33, 0xFF],
            padding: 16,
            system_font: None,
            style: StyleConfig::default(),
            max_width: None,
            max_height: None,
            activation_fill: [0xFF, 0xFF, 0xFF, 0xFF],
            frame_fill: [0xF8, 0xF8, 0xF8, 0xFF],
        }
    }
}

fn load_font(opts: &RenderOptions) -> Result<font::DiagramFont, SeqDiagramError> {
    match &opts.system_font {
        Some(path) => font::DiagramFont::load_with_system_font(path),
        None => font::DiagramFont::load(),
    }
}

/// Error type for sequence diagram operations.
#[derive(Debug, thiserror::Error)]
pub enum SeqDiagramError {
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Layout error: {0}")]
    Layout(String),
    #[error("Font error: {0}")]
    Font(String),
    #[error("Render error: {0}")]
    Render(String),
}

/// Render a sequence diagram to an SVG string.
pub fn render_to_svg(input: &str, options: Option<RenderOptions>) -> Result<String, SeqDiagramError> {
    let opts = options.unwrap_or_default();
    let font_size_px = opts.font_size_pt * opts.scale;

    let doc = parse::parse_document(input)?;
    let diagram_font = load_font(&opts)?;
    let layout = layout::layout_diagram(&diagram_font, &doc, font_size_px, opts.padding as f32)?;

    Ok(svg_render::render_to_svg_string(
        &diagram_font,
        &layout,
        &opts,
    ))
}

/// Render a sequence diagram to PNG bytes.
pub fn render_to_png(
    input: &str,
    options: Option<RenderOptions>,
) -> Result<Vec<u8>, SeqDiagramError> {
    let pixmap = render_to_pixmap(input, options)?;
    pixmap
        .encode_png()
        .map_err(|e| SeqDiagramError::Render(e.to_string()))
}

/// Render a sequence diagram to a tiny_skia::Pixmap.
pub fn render_to_pixmap(
    input: &str,
    options: Option<RenderOptions>,
) -> Result<tiny_skia::Pixmap, SeqDiagramError> {
    let opts = options.unwrap_or_default();
    let font_size_px = opts.font_size_pt * opts.scale;

    let doc = parse::parse_document(input)?;
    let diagram_font = load_font(&opts)?;
    let layout = layout::layout_diagram(&diagram_font, &doc, font_size_px, opts.padding as f32)?;

    let natural_w = layout.width;
    let natural_h = layout.height;

    // Compute uniform scale factor for max constraints
    let scale_factor = match (opts.max_width, opts.max_height) {
        (Some(mw), Some(mh)) => {
            let sw = mw / natural_w;
            let sh = mh / natural_h;
            sw.min(sh).min(1.0) // never scale up
        }
        (Some(mw), None) => (mw / natural_w).min(1.0),
        (None, Some(mh)) => (mh / natural_h).min(1.0),
        (None, None) => 1.0,
    };

    let img_width = (natural_w * scale_factor).ceil() as u32;
    let img_height = (natural_h * scale_factor).ceil() as u32;
    let img_width = img_width.max(1);
    let img_height = img_height.max(1);

    let mut pixmap = tiny_skia::Pixmap::new(img_width, img_height)
        .ok_or_else(|| SeqDiagramError::Render("Failed to create pixmap".into()))?;

    let bg = tiny_skia::Color::from_rgba8(
        opts.bg_color[0],
        opts.bg_color[1],
        opts.bg_color[2],
        opts.bg_color[3],
    );
    pixmap.fill(bg);

    let transform = if scale_factor < 1.0 {
        tiny_skia::Transform::from_scale(scale_factor, scale_factor)
    } else {
        tiny_skia::Transform::identity()
    };

    png_render::render_diagram(&diagram_font, &mut pixmap, &layout, &opts, transform);

    Ok(pixmap)
}
