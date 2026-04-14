use skrifa::instance::LocationRef;
use skrifa::metrics::GlyphMetrics;
use skrifa::{FontRef, GlyphId, MetadataProvider};

use crate::SeqDiagramError;

const PRIMARY_FONT: &[u8] = include_bytes!("../fonts/Inter-Regular.otf");

#[cfg(feature = "unicode-fonts")]
const FALLBACK_FONT: &[u8] = include_bytes!("../fonts/GoNotoKurrent-Regular.ttf");

/// Which font a glyph was resolved from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontSource {
    Primary,
    Fallback,
    System,
}

pub struct DiagramFont {
    primary: &'static [u8],
    #[cfg(feature = "unicode-fonts")]
    fallback: &'static [u8],
    /// Optional system font loaded at runtime.
    system_font: Option<Vec<u8>>,
}

impl DiagramFont {
    pub fn load() -> Result<Self, SeqDiagramError> {
        FontRef::new(PRIMARY_FONT)
            .map_err(|e| SeqDiagramError::Font(format!("Failed to load primary font: {e}")))?;

        #[cfg(feature = "unicode-fonts")]
        FontRef::new(FALLBACK_FONT)
            .map_err(|e| SeqDiagramError::Font(format!("Failed to load fallback font: {e}")))?;

        Ok(Self {
            primary: PRIMARY_FONT,
            #[cfg(feature = "unicode-fonts")]
            fallback: FALLBACK_FONT,
            system_font: None,
        })
    }

    /// Load with an additional system font from a file path.
    pub fn load_with_system_font(path: &str) -> Result<Self, SeqDiagramError> {
        let mut font = Self::load()?;
        let data = std::fs::read(path)
            .map_err(|e| SeqDiagramError::Font(format!("Failed to read font {path}: {e}")))?;
        // Validate
        FontRef::new(&data)
            .map_err(|e| SeqDiagramError::Font(format!("Invalid font {path}: {e}")))?;
        font.system_font = Some(data);
        Ok(font)
    }

    pub fn primary_ref(&self) -> FontRef<'_> {
        FontRef::new(self.primary).unwrap()
    }

    #[cfg(feature = "unicode-fonts")]
    pub fn fallback_ref(&self) -> FontRef<'_> {
        FontRef::new(self.fallback).unwrap()
    }

    fn system_ref(&self) -> Option<FontRef<'_>> {
        self.system_font.as_ref().and_then(|d| FontRef::new(d).ok())
    }

    /// Resolve a character to a glyph ID, trying primary -> system -> fallback.
    pub fn resolve_glyph(&self, c: char) -> Option<(GlyphId, FontSource)> {
        // 1. Primary (Inter)
        if let Some(gid) = self.primary_ref().charmap().map(c) {
            return Some((gid, FontSource::Primary));
        }
        // 2. System font (if loaded)
        if let Some(font_ref) = self.system_ref() {
            if let Some(gid) = font_ref.charmap().map(c) {
                return Some((gid, FontSource::System));
            }
        }
        // 3. Embedded fallback (GoNotoKurrent)
        #[cfg(feature = "unicode-fonts")]
        if let Some(gid) = self.fallback_ref().charmap().map(c) {
            return Some((gid, FontSource::Fallback));
        }
        None
    }

    /// Get a FontRef for the given source.
    pub fn font_for(&self, source: FontSource) -> FontRef<'_> {
        match source {
            FontSource::Primary => self.primary_ref(),
            #[cfg(feature = "unicode-fonts")]
            FontSource::Fallback => self.fallback_ref(),
            FontSource::System => self.system_ref().expect("system font not loaded"),
            #[cfg(not(feature = "unicode-fonts"))]
            FontSource::Fallback => panic!("unicode-fonts feature not enabled"),
        }
    }

    /// Get glyph metrics at a given size for a specific font source.
    pub fn glyph_metrics_for(&self, font_size_px: f32, source: FontSource) -> GlyphMetrics<'_> {
        let font = self.font_for(source);
        let size = skrifa::instance::Size::new(font_size_px);
        font.glyph_metrics(size, LocationRef::default())
    }

    /// Get font-level metrics (ascent, descent) at a given size.
    /// Uses the primary font for consistent baseline positioning.
    pub fn metrics(&self, font_size_px: f32) -> skrifa::metrics::Metrics {
        let font = self.primary_ref();
        let size = skrifa::instance::Size::new(font_size_px);
        font.metrics(size, LocationRef::default())
    }

    /// Measure the width of a text string at the given font size in pixels.
    pub fn text_width(&self, text: &str, font_size_px: f32) -> f32 {
        text.chars()
            .filter_map(|c| {
                let (gid, source) = self.resolve_glyph(c)?;
                let gm = self.glyph_metrics_for(font_size_px, source);
                Some(gm.advance_width(gid).unwrap_or(0.0))
            })
            .sum()
    }
}
