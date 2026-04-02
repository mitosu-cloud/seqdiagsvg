use skrifa::instance::LocationRef;
use skrifa::metrics::GlyphMetrics;
use skrifa::{FontRef, GlyphId, MetadataProvider};

use crate::SeqDiagramError;

const PRIMARY_FONT: &[u8] = include_bytes!("../fonts/Inter-Regular.otf");
const FALLBACK_FONT: &[u8] = include_bytes!("../fonts/WenQuanYiMicroHei-Regular.ttf");

pub struct DiagramFont {
    primary: &'static [u8],
    fallback: &'static [u8],
}

/// Which font a glyph was resolved from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontSource {
    Primary,
    Fallback,
}

impl DiagramFont {
    pub fn load() -> Result<Self, SeqDiagramError> {
        // Validate both fonts load
        FontRef::new(PRIMARY_FONT)
            .map_err(|e| SeqDiagramError::Font(format!("Failed to load primary font: {e}")))?;
        FontRef::new(FALLBACK_FONT)
            .map_err(|e| SeqDiagramError::Font(format!("Failed to load fallback font: {e}")))?;
        Ok(Self {
            primary: PRIMARY_FONT,
            fallback: FALLBACK_FONT,
        })
    }

    pub fn primary_ref(&self) -> FontRef<'_> {
        FontRef::new(self.primary).unwrap()
    }

    pub fn fallback_ref(&self) -> FontRef<'_> {
        FontRef::new(self.fallback).unwrap()
    }

    /// Resolve a character to a glyph ID, trying primary font first then fallback.
    pub fn resolve_glyph(&self, c: char) -> Option<(GlyphId, FontSource)> {
        let primary = self.primary_ref();
        if let Some(gid) = primary.charmap().map(c) {
            return Some((gid, FontSource::Primary));
        }
        let fallback = self.fallback_ref();
        if let Some(gid) = fallback.charmap().map(c) {
            return Some((gid, FontSource::Fallback));
        }
        None
    }

    /// Get a FontRef for the given source.
    pub fn font_for(&self, source: FontSource) -> FontRef<'_> {
        match source {
            FontSource::Primary => self.primary_ref(),
            FontSource::Fallback => self.fallback_ref(),
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
