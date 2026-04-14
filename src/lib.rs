pub mod ast;
mod font;
mod layout;
mod parse;
mod png_render;
mod svg_render;

pub use ast::{Arrow, Document, HeadStyle, LineStyle, NotePosition, Statement};
pub use parse::{parse_document, resolve_actors};

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
    /// Padding in pixels around the diagram
    pub padding: u32,
    /// Optional path to an additional system font (OTF/TTF).
    /// Tried after the primary font (Inter) but before the embedded fallback.
    pub system_font: Option<String>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            font_size_pt: 14.0,
            scale: 2.0,
            fg_color: [0x33, 0x33, 0x33, 0xFF],
            bg_color: [0xFF, 0xFF, 0xFF, 0xFF],
            padding: 16,
            system_font: None,
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

    let img_width = layout.width.ceil() as u32;
    let img_height = layout.height.ceil() as u32;
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

    png_render::render_diagram(&diagram_font, &mut pixmap, &layout, opts.fg_color);

    Ok(pixmap)
}
