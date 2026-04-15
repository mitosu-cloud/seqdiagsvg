uniffi::setup_scaffolding!();

use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SeqDiagError {
    #[error("Parse failed: {msg}")]
    ParseFailed { msg: String },
    #[error("Render failed: {msg}")]
    RenderFailed { msg: String },
    #[error("IO failed: {msg}")]
    IoFailed { msg: String },
    #[error("Theme parse failed: {msg}")]
    ThemeParseFailed { msg: String },
}

// ── Checksum ────────────────────────────────────────────────────────────────

/// SHA256 of seqdiag source, truncated to 12 hex chars.
fn compute_checksum(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let result = hasher.finalize();
    hex_encode(&result[..6])
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Sanitise theme name for use in filenames: lowercase, spaces → hyphens.
fn sanitise_theme_name(name: &str) -> String {
    name.to_lowercase().replace(' ', "-")
}

// ── Theme color extraction ─────────────────────────────────────────────────

/// Extracted theme colors: foreground, background, and note background.
struct ThemeColors {
    fg: [u8; 4],
    bg: [u8; 4],
    note_color: [u8; 4],
}

/// Extract fg/bg/note RGBA colors from Mitosu theme JSON.
/// Lookup chains mirror the mermaid FFI pattern: seqdiag.* -> mermaid.* -> generic keys -> default.
fn extract_colors(theme_json: &str) -> Result<ThemeColors, SeqDiagError> {
    let val: serde_json::Value = serde_json::from_str(theme_json)
        .map_err(|e| SeqDiagError::ThemeParseFailed { msg: e.to_string() })?;

    let colors = val.get("colors").and_then(|v| v.as_object());
    let is_dark = val
        .get("type")
        .and_then(|v| v.as_str())
        .map(|t| t == "dark")
        .unwrap_or(true);

    let get = |key: &str| -> Option<String> {
        colors
            .and_then(|c| c.get(key))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };

    // Foreground: seqdiag.foreground -> mermaid.textColor -> markdownEditor.text -> ui.text -> default
    let fg = get("seqdiag.foreground")
        .or_else(|| get("mermaid.textColor"))
        .or_else(|| get("markdownEditor.text"))
        .or_else(|| get("ui.text"))
        .and_then(|hex| parse_hex_color(&hex))
        .unwrap_or(if is_dark {
            [0xD4, 0xD4, 0xD4, 0xFF]
        } else {
            [0x33, 0x33, 0x33, 0xFF]
        });

    // Background: seqdiag.background -> mermaid.background -> markdownEditor.background -> ui.background -> default
    let bg = get("seqdiag.background")
        .or_else(|| get("mermaid.background"))
        .or_else(|| get("markdownEditor.background"))
        .or_else(|| get("ui.background"))
        .and_then(|hex| parse_hex_color(&hex))
        .unwrap_or(if is_dark {
            [0x1E, 0x1E, 0x1E, 0xFF]
        } else {
            [0xFF, 0xFF, 0xFF, 0xFF]
        });

    // Note background: seqdiag.noteBackground -> mermaid.sequenceNoteFill -> wysiwygEditor.blockquoteBackground -> default
    let note_color = get("seqdiag.noteBackground")
        .or_else(|| get("mermaid.sequenceNoteFill"))
        .or_else(|| get("wysiwygEditor.blockquoteBackground"))
        .and_then(|hex| parse_hex_color(&hex))
        .unwrap_or(if is_dark {
            [0x33, 0x33, 0x33, 0xFF]
        } else {
            [0xFF, 0xFF, 0xCC, 0xFF]
        });

    Ok(ThemeColors { fg, bg, note_color })
}

/// Parse #RGB, #RRGGBB, or #RRGGBBAA hex color to [u8; 4] RGBA.
fn parse_hex_color(hex: &str) -> Option<[u8; 4]> {
    let hex = hex.trim_start_matches('#');
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some([r, g, b, 255])
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some([r, g, b, 255])
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some([r, g, b, a])
        }
        _ => None,
    }
}

// ── Metadata ────────────────────────────────────────────────────────────────

/// In-memory representation of metadata.json.
/// Includes all sections to avoid dropping data from other crates.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Metadata {
    #[serde(default)]
    mermaid_diagrams: HashMap<String, DiagramEntry>,
    #[serde(default)]
    math_equations: HashMap<String, DiagramEntry>,
    #[serde(default)]
    seqdiag_diagrams: HashMap<String, DiagramEntry>,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
struct DiagramEntry {
    files: HashMap<String, String>,
}

fn metadata_path(note_folder: &Path) -> PathBuf {
    note_folder.join("metadata.json")
}

fn read_metadata(note_folder: &Path) -> Metadata {
    let path = metadata_path(note_folder);
    match fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Metadata::default(),
    }
}

fn write_metadata(note_folder: &Path, meta: &Metadata) -> Result<(), SeqDiagError> {
    let path = metadata_path(note_folder);
    let json = serde_json::to_string_pretty(meta)
        .map_err(|e| SeqDiagError::IoFailed { msg: e.to_string() })?;
    fs::write(&path, json).map_err(|e| SeqDiagError::IoFailed { msg: e.to_string() })?;
    Ok(())
}

// ── Public FFI functions ────────────────────────────────────────────────────

/// Primary render function — handles everything:
/// 1. Parses the Mitosu theme JSON → extracts fg/bg colors
/// 2. Computes SHA256 checksum of seqdiag source
/// 3. Checks if output file already exists on disk (cache hit)
/// 4. If not, renders and writes to disk (SVG string or PNG bytes)
/// 5. Updates metadata.json
/// Returns the filename (not full path) of the output file.
///
/// `output_format` should be `"svg"` or `"png"`. Defaults to SVG for unknown values.
#[uniffi::export]
pub fn render_seqdiag_for_note(
    seqdiag_source: String,
    note_folder_path: String,
    theme_json: String,
    theme_name: String,
    width: f32,
    height: f32,
    output_format: String,
) -> Result<String, SeqDiagError> {
    let note_folder = Path::new(&note_folder_path);
    let checksum = compute_checksum(&seqdiag_source);
    let safe_theme = sanitise_theme_name(&theme_name);
    let use_png = output_format.eq_ignore_ascii_case("png");
    let ext = if use_png { "png" } else { "svg" };
    let filename = format!("seqdiag_{}_{}.{}", checksum, safe_theme, ext);
    let output_path = note_folder.join(&filename);

    // Cache hit — file already exists
    if output_path.exists() {
        return Ok(filename);
    }

    // Extract colors from theme
    let theme = extract_colors(&theme_json)?;

    let options = seqdiagsvg::RenderOptions {
        font_size_pt: 14.0,
        scale: 2.0,
        fg_color: theme.fg,
        bg_color: theme.bg,
        note_color: theme.note_color,
        padding: 16,
        system_font: None,
        style: seqdiagsvg::StyleConfig::default(),
        max_width: if width > 0.0 { Some(width) } else { None },
        max_height: if height > 0.0 { Some(height) } else { None },
    };

    // Write to a temp file then atomically rename to prevent partial reads
    let tmp_path = output_path.with_extension("tmp");

    if use_png {
        let png_bytes = seqdiagsvg::render_to_png(&seqdiag_source, Some(options))
            .map_err(|e| SeqDiagError::RenderFailed { msg: e.to_string() })?;
        fs::write(&tmp_path, &png_bytes)
            .map_err(|e| SeqDiagError::IoFailed { msg: e.to_string() })?;
    } else {
        let svg = seqdiagsvg::render_to_svg(&seqdiag_source, Some(options))
            .map_err(|e| SeqDiagError::RenderFailed { msg: e.to_string() })?;
        fs::write(&tmp_path, &svg)
            .map_err(|e| SeqDiagError::IoFailed { msg: e.to_string() })?;
    }

    fs::rename(&tmp_path, &output_path)
        .map_err(|e| SeqDiagError::IoFailed { msg: e.to_string() })?;

    // Update metadata.json
    let mut meta = read_metadata(note_folder);
    let entry = meta
        .seqdiag_diagrams
        .entry(checksum.clone())
        .or_default();
    entry.files.insert(safe_theme, filename.clone());
    write_metadata(note_folder, &meta)?;

    Ok(filename)
}

/// Compute checksum only (for Swift-side cache checks without full render).
#[uniffi::export]
pub fn seqdiag_checksum(seqdiag_source: String) -> String {
    compute_checksum(&seqdiag_source)
}

/// Validate seqdiag syntax without rendering.
#[uniffi::export]
pub fn validate_seqdiag(seqdiag_source: String) -> Result<(), SeqDiagError> {
    seqdiagsvg::parse_document(&seqdiag_source)
        .map_err(|e| SeqDiagError::ParseFailed { msg: e.to_string() })?;
    Ok(())
}

/// Remove all seqdiag files whose checksum is NOT in `valid_checksums`.
/// Updates metadata.json. Returns list of deleted filenames.
#[uniffi::export]
pub fn cleanup_stale_seqdiag_files(
    note_folder_path: String,
    valid_checksums: Vec<String>,
) -> Result<Vec<String>, SeqDiagError> {
    let note_folder = Path::new(&note_folder_path);
    let mut meta = read_metadata(note_folder);
    let mut deleted: Vec<String> = Vec::new();
    let valid_set: HashSet<&str> = valid_checksums.iter().map(|s| s.as_str()).collect();

    // Collect checksums to remove from metadata
    let stale_checksums: Vec<String> = meta
        .seqdiag_diagrams
        .keys()
        .filter(|cs| !valid_set.contains(cs.as_str()))
        .cloned()
        .collect();

    for cs in &stale_checksums {
        if let Some(entry) = meta.seqdiag_diagrams.remove(cs) {
            for (_theme, filename) in &entry.files {
                let path = note_folder.join(filename);
                if path.exists() {
                    let _ = fs::remove_file(&path);
                    deleted.push(filename.clone());
                }
            }
        }
    }

    // Also scan for untracked seqdiag files on disk
    if let Ok(entries) = fs::read_dir(note_folder) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("seqdiag_") && (name.ends_with(".png") || name.ends_with(".svg")) {
                // Extract checksum from filename: seqdiag_<checksum>_<theme>.(png|svg)
                if let Some(cs) = name
                    .strip_prefix("seqdiag_")
                    .and_then(|rest| rest.split('_').next())
                {
                    if !valid_set.contains(cs) && !deleted.contains(&name) {
                        let path = entry.path();
                        if path.exists() {
                            let _ = fs::remove_file(&path);
                            deleted.push(name);
                        }
                    }
                }
            }
        }
    }

    if !stale_checksums.is_empty() {
        write_metadata(note_folder, &meta)?;
    }

    Ok(deleted)
}

/// Read seqdiag metadata from a note folder's metadata.json.
/// Returns JSON string of the seqdiag_diagrams section (or empty object).
#[uniffi::export]
pub fn get_seqdiag_metadata(note_folder_path: String) -> Result<String, SeqDiagError> {
    let note_folder = Path::new(&note_folder_path);
    let meta = read_metadata(note_folder);
    serde_json::to_string(&meta.seqdiag_diagrams)
        .map_err(|e| SeqDiagError::IoFailed { msg: e.to_string() })
}
