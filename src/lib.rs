pub mod ast;
mod font;
mod layout;
mod parse;
mod svg_render;

pub use ast::{
    ActivationModifier, Arrow, Document, FrameKind, HeadStyle, LineStyle, NotePosition, Statement,
};
pub use parse::{parse_document, resolve_actors};

/// Drop shadow configuration.
/// Maps to an SVG filter using feGaussianBlur + feOffset + feMerge for maximum compatibility.
#[derive(Debug, Clone, Copy)]
pub struct DropShadow {
    /// Horizontal offset (default: 2.0)
    pub dx: f32,
    /// Vertical offset (default: 2.0)
    pub dy: f32,
    /// Blur radius (default: 3.0)
    pub std_deviation: f32,
    /// Shadow color as RGBA (default: black at 50% opacity)
    pub color: [u8; 4],
}

impl Default for DropShadow {
    fn default() -> Self {
        Self {
            dx: 2.0,
            dy: 2.0,
            std_deviation: 3.0,
            color: [0x00, 0x00, 0x00, 0x80], // black, 50% opacity
        }
    }
}

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
    /// RGBA frame fill colors per nesting depth (up to 5 levels).
    /// `None` entries fall back to the previous level, or to the built-in default.
    /// Built-in defaults: level 0 = fully transparent, levels 1+ = light blue ~10% opacity.
    pub frame_fills: [Option<[u8; 4]>; 5],
    /// Drop shadow for actor boxes. None = no shadow.
    pub actor_shadow: Option<DropShadow>,
    /// Drop shadow for note boxes. None = no shadow.
    pub note_shadow: Option<DropShadow>,
}

/// Built-in frame fill defaults per nesting depth.
const DEFAULT_FRAME_FILLS: [[u8; 4]; 5] = [
    [0xFF, 0xFF, 0xFF, 0x00], // depth 0: fully transparent (like websequencediagrams)
    [0xBB, 0xCC, 0xDD, 0x1A], // depth 1: light blue, ~10% opacity
    [0xBB, 0xCC, 0xDD, 0x26], // depth 2: light blue, ~15% opacity
    [0xBB, 0xCC, 0xDD, 0x33], // depth 3: light blue, ~20% opacity
    [0xBB, 0xCC, 0xDD, 0x40], // depth 4: light blue, ~25% opacity
];

impl RenderOptions {
    /// Resolve the frame fill color for a given nesting depth.
    /// Checks user overrides first, falling back through previous levels,
    /// then to the built-in defaults.
    pub fn frame_fill_for_depth(&self, depth: usize) -> [u8; 4] {
        let clamped = depth.min(4);
        // Check this level and fall back through previous levels
        for d in (0..=clamped).rev() {
            if let Some(color) = self.frame_fills[d] {
                return color;
            }
        }
        // No user override at any level — use built-in default
        DEFAULT_FRAME_FILLS[clamped]
    }
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
            frame_fills: [None; 5], // all None = use built-in defaults
            actor_shadow: None,
            note_shadow: None,
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
///
/// Internally renders to SVG first, then rasterizes via resvg.
/// This guarantees pixel-perfect parity between SVG and PNG output.
pub fn render_to_png(
    input: &str,
    options: Option<RenderOptions>,
) -> Result<Vec<u8>, SeqDiagramError> {
    let svg = render_to_svg(input, options)?;

    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_str(&svg, &opt)
        .map_err(|e| SeqDiagramError::Render(format!("SVG parse error: {e}")))?;

    let size = tree.size();
    let width = size.width().ceil() as u32;
    let height = size.height().ceil() as u32;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width.max(1), height.max(1))
        .ok_or_else(|| SeqDiagramError::Render("Failed to create pixmap".into()))?;

    resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pixmap.as_mut());

    pixmap
        .encode_png()
        .map_err(|e| SeqDiagramError::Render(e.to_string()))
}
