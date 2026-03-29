
//! Font and text editor — font atlas generation, glyph metrics, text layout, rich text.

use glam::{Vec2, Vec4};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Unicode / glyph
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct GlyphMetrics {
    pub codepoint: u32,
    pub advance_x: f32,
    pub advance_y: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub width: f32,
    pub height: f32,
    pub uv_min: Vec2,
    pub uv_max: Vec2,
    pub atlas_page: u32,
    pub is_fallback: bool,
}

impl GlyphMetrics {
    pub fn new(cp: u32, advance: f32, bearing_x: f32, bearing_y: f32, w: f32, h: f32) -> Self {
        Self {
            codepoint: cp,
            advance_x: advance,
            advance_y: 0.0,
            bearing_x,
            bearing_y,
            width: w,
            height: h,
            uv_min: Vec2::ZERO,
            uv_max: Vec2::new(w / 512.0, h / 512.0),
            atlas_page: 0,
            is_fallback: false,
        }
    }

    pub fn draw_rect(&self, cursor: Vec2, scale: f32) -> [Vec2; 2] {
        let x = cursor.x + self.bearing_x * scale;
        let y = cursor.y - self.bearing_y * scale;
        [Vec2::new(x, y), Vec2::new(x + self.width * scale, y + self.height * scale)]
    }
}

// ---------------------------------------------------------------------------
// Kerning
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct KerningPair {
    pub first: u32,
    pub second: u32,
    pub amount_x: f32,
    pub amount_y: f32,
}

// ---------------------------------------------------------------------------
// Font face
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontStyle {
    Regular, Bold, Italic, BoldItalic,
}

impl FontStyle {
    pub fn label(self) -> &'static str {
        match self { FontStyle::Regular => "Regular", FontStyle::Bold => "Bold", FontStyle::Italic => "Italic", FontStyle::BoldItalic => "Bold Italic" }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontHinting { None, Slight, Normal, Full }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontRenderMode { Bitmap, SDF, MSDF, MTSDF }

impl FontRenderMode {
    pub fn label(self) -> &'static str {
        match self { FontRenderMode::Bitmap => "Bitmap", FontRenderMode::SDF => "SDF", FontRenderMode::MSDF => "MSDF", FontRenderMode::MTSDF => "MTSDF" }
    }
    pub fn supports_smooth_scaling(self) -> bool { !matches!(self, FontRenderMode::Bitmap) }
}

#[derive(Debug, Clone)]
pub struct FontFace {
    pub family: String,
    pub style: FontStyle,
    pub units_per_em: u32,
    pub ascent: f32,
    pub descent: f32,
    pub line_gap: f32,
    pub cap_height: f32,
    pub x_height: f32,
    pub underline_position: f32,
    pub underline_thickness: f32,
    pub strikethrough_position: f32,
    pub strikethrough_thickness: f32,
    pub glyphs: HashMap<u32, GlyphMetrics>,
    pub kerning: Vec<KerningPair>,
    pub render_mode: FontRenderMode,
    pub hinting: FontHinting,
    pub atlas_size: u32,
    pub sdf_range: f32,
    pub tab_width: u32,
}

impl FontFace {
    pub fn new(family: impl Into<String>, style: FontStyle) -> Self {
        Self {
            family: family.into(),
            style,
            units_per_em: 2048,
            ascent: 1890.0,
            descent: -434.0,
            line_gap: 0.0,
            cap_height: 1462.0,
            x_height: 1098.0,
            underline_position: -150.0,
            underline_thickness: 100.0,
            strikethrough_position: 530.0,
            strikethrough_thickness: 100.0,
            glyphs: HashMap::new(),
            kerning: Vec::new(),
            render_mode: FontRenderMode::MSDF,
            hinting: FontHinting::Normal,
            atlas_size: 512,
            sdf_range: 6.0,
            tab_width: 4,
        }
    }

    pub fn line_height(&self, size: f32) -> f32 {
        (self.ascent - self.descent + self.line_gap) * size / self.units_per_em as f32
    }

    pub fn scale_for_size(&self, size: f32) -> f32 {
        size / self.units_per_em as f32
    }

    pub fn kerning_for_pair(&self, first: u32, second: u32) -> f32 {
        self.kerning.iter()
            .find(|k| k.first == first && k.second == second)
            .map(|k| k.amount_x)
            .unwrap_or(0.0)
    }

    pub fn glyph(&self, codepoint: u32) -> Option<&GlyphMetrics> {
        self.glyphs.get(&codepoint)
    }

    /// Populate with synthetic ASCII glyphs.
    pub fn populate_ascii(&mut self) {
        for cp in 32u32..128 {
            let w = if cp == 32 { 0.0 } else { 800.0 + (cp as f32 * 13.0) % 400.0 };
            let h = 1300.0_f32;
            let bearing_x = 50.0_f32;
            let bearing_y = 1000.0_f32;
            let advance = w + bearing_x + 100.0;
            let mut g = GlyphMetrics::new(cp, advance, bearing_x, bearing_y, w, h);
            g.uv_min = Vec2::new((cp % 16) as f32 / 16.0, (cp / 16) as f32 / 8.0);
            g.uv_max = g.uv_min + Vec2::new(w / (self.atlas_size as f32 * 128.0), h / (self.atlas_size as f32 * 64.0));
            self.glyphs.insert(cp, g);
        }
        // Some kerning pairs
        self.kerning.push(KerningPair { first: b'A' as u32, second: b'V' as u32, amount_x: -120.0, amount_y: 0.0 });
        self.kerning.push(KerningPair { first: b'T' as u32, second: b'a' as u32, amount_x: -100.0, amount_y: 0.0 });
        self.kerning.push(KerningPair { first: b'f' as u32, second: b'i' as u32, amount_x: -50.0, amount_y: 0.0 });
    }

    pub fn measure_text(&self, text: &str, size: f32) -> Vec2 {
        let scale = self.scale_for_size(size);
        let mut width = 0.0_f32;
        let mut max_width = 0.0_f32;
        let mut lines = 1u32;
        let mut prev_cp: Option<u32> = None;
        for ch in text.chars() {
            if ch == '\n' {
                lines += 1;
                max_width = max_width.max(width);
                width = 0.0;
                prev_cp = None;
                continue;
            }
            let cp = ch as u32;
            let kern = prev_cp.map(|p| self.kerning_for_pair(p, cp)).unwrap_or(0.0);
            let advance = self.glyphs.get(&cp).map(|g| g.advance_x).unwrap_or(600.0);
            width += (advance + kern) * scale;
            prev_cp = Some(cp);
        }
        max_width = max_width.max(width);
        Vec2::new(max_width, self.line_height(size) * lines as f32)
    }
}

// ---------------------------------------------------------------------------
// Rich text spans
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextDecoration { None, Underline, Strikethrough, Overline, DottedUnderline }

#[derive(Debug, Clone)]
pub struct TextSpan {
    pub text: String,
    pub font_size: f32,
    pub color: Vec4,
    pub font_family: Option<String>,
    pub style: FontStyle,
    pub decoration: TextDecoration,
    pub letter_spacing: f32,
    pub word_spacing: f32,
    pub line_height_multiplier: f32,
    pub baseline_shift: f32,
    pub bold: bool,
    pub italic: bool,
    pub shadow: Option<TextShadow>,
    pub outline: Option<TextOutline>,
    pub link: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct TextShadow {
    pub offset: Vec2,
    pub blur: f32,
    pub color: Vec4,
}

#[derive(Debug, Clone, Copy)]
pub struct TextOutline {
    pub width: f32,
    pub color: Vec4,
}

impl Default for TextSpan {
    fn default() -> Self {
        Self {
            text: String::new(),
            font_size: 16.0,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            font_family: None,
            style: FontStyle::Regular,
            decoration: TextDecoration::None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            line_height_multiplier: 1.2,
            baseline_shift: 0.0,
            bold: false,
            italic: false,
            shadow: None,
            outline: None,
            link: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Text layout
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextAlign { Left, Center, Right, Justify }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerticalAlign { Top, Middle, Bottom, Baseline }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextOverflow { Clip, Ellipsis, Scroll, Visible }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WordWrap { NoWrap, Normal, BreakWord, BreakAll }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextDirection { Ltr, Rtl, Auto }

#[derive(Debug, Clone)]
pub struct TextLayoutSettings {
    pub align: TextAlign,
    pub vertical_align: VerticalAlign,
    pub overflow: TextOverflow,
    pub word_wrap: WordWrap,
    pub direction: TextDirection,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub tab_size: f32,
    pub paragraph_spacing: f32,
    pub indent: f32,
    pub enable_ligatures: bool,
    pub enable_smart_quotes: bool,
    pub auto_size: bool,
    pub auto_size_min: f32,
    pub auto_size_max: f32,
}

impl Default for TextLayoutSettings {
    fn default() -> Self {
        Self {
            align: TextAlign::Left,
            vertical_align: VerticalAlign::Top,
            overflow: TextOverflow::Clip,
            word_wrap: WordWrap::Normal,
            direction: TextDirection::Auto,
            max_width: None,
            max_height: None,
            tab_size: 32.0,
            paragraph_spacing: 0.0,
            indent: 0.0,
            enable_ligatures: true,
            enable_smart_quotes: false,
            auto_size: false,
            auto_size_min: 8.0,
            auto_size_max: 72.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LayoutGlyph {
    pub codepoint: u32,
    pub position: Vec2,
    pub size: Vec2,
    pub uv_min: Vec2,
    pub uv_max: Vec2,
    pub color: Vec4,
    pub atlas_page: u32,
    pub line_index: u32,
    pub span_index: u32,
}

#[derive(Debug, Clone)]
pub struct TextLayoutResult {
    pub glyphs: Vec<LayoutGlyph>,
    pub line_starts: Vec<usize>,
    pub total_size: Vec2,
    pub line_count: u32,
    pub overflows: bool,
}

impl TextLayoutResult {
    pub fn empty() -> Self {
        Self { glyphs: Vec::new(), line_starts: Vec::new(), total_size: Vec2::ZERO, line_count: 0, overflows: false }
    }

    pub fn glyph_count(&self) -> usize { self.glyphs.len() }
}

pub fn layout_text(spans: &[TextSpan], font: &FontFace, settings: &TextLayoutSettings) -> TextLayoutResult {
    let mut result = TextLayoutResult::empty();
    let max_width = settings.max_width.unwrap_or(f32::INFINITY);
    let mut cursor = Vec2::ZERO;
    let mut line_idx = 0u32;
    result.line_starts.push(0);
    for (si, span) in spans.iter().enumerate() {
        let scale = font.scale_for_size(span.font_size);
        let line_h = font.line_height(span.font_size) * span.line_height_multiplier;
        let mut word_start = result.glyphs.len();
        let mut word_width = 0.0_f32;
        for ch in span.text.chars() {
            if ch == '\n' {
                cursor.x = 0.0;
                cursor.y += line_h;
                line_idx += 1;
                result.line_starts.push(result.glyphs.len());
                continue;
            }
            let cp = ch as u32;
            let glyph = font.glyph(cp);
            let advance = glyph.map(|g| g.advance_x).unwrap_or(600.0) * scale + span.letter_spacing;
            // Word-wrap
            if settings.word_wrap == WordWrap::Normal && cursor.x + advance > max_width {
                // Move to next line
                cursor.x = 0.0;
                cursor.y += line_h;
                line_idx += 1;
                result.line_starts.push(result.glyphs.len());
            }
            let (uv_min, uv_max, w, h, bx, by) = if let Some(g) = glyph {
                (g.uv_min, g.uv_max, g.width * scale, g.height * scale, g.bearing_x * scale, g.bearing_y * scale)
            } else { (Vec2::ZERO, Vec2::ONE / 512.0, advance, span.font_size, 0.0, span.font_size) };
            result.glyphs.push(LayoutGlyph {
                codepoint: cp,
                position: Vec2::new(cursor.x + bx, cursor.y - by + span.baseline_shift),
                size: Vec2::new(w, h),
                uv_min, uv_max,
                color: span.color,
                atlas_page: 0,
                line_index: line_idx,
                span_index: si as u32,
            });
            cursor.x += advance;
        }
        // Track max width
        result.total_size.x = result.total_size.x.max(cursor.x);
    }
    result.total_size.y = cursor.y + spans.last().map(|s| font.line_height(s.font_size)).unwrap_or(0.0);
    result.line_count = line_idx + 1;
    if let Some(mw) = settings.max_width { if result.total_size.x > mw { result.overflows = true; } }
    if let Some(mh) = settings.max_height { if result.total_size.y > mh { result.overflows = true; } }
    result
}

// ---------------------------------------------------------------------------
// Font atlas packer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AtlasPackingMode { Shelf, Skyline, Guillotine, MaxRects }

#[derive(Debug, Clone)]
pub struct AtlasPackRect {
    pub id: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct AtlasPacker {
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub mode: AtlasPackingMode,
    pub padding: u32,
    pub rects: Vec<AtlasPackRect>,
    pub current_shelf_y: u32,
    pub current_shelf_x: u32,
    pub current_shelf_height: u32,
    pub pages: u32,
}

impl AtlasPacker {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            atlas_width: width, atlas_height: height,
            mode: AtlasPackingMode::Shelf, padding: 1,
            rects: Vec::new(), current_shelf_y: 0,
            current_shelf_x: 0, current_shelf_height: 0, pages: 1,
        }
    }

    pub fn pack(&mut self, id: u32, width: u32, height: u32) -> Option<AtlasPackRect> {
        let pw = width + self.padding;
        let ph = height + self.padding;
        // Simple shelf algorithm
        if self.current_shelf_x + pw > self.atlas_width {
            self.current_shelf_y += self.current_shelf_height;
            self.current_shelf_x = 0;
            self.current_shelf_height = 0;
            if self.current_shelf_y + ph > self.atlas_height {
                self.pages += 1;
                self.current_shelf_y = 0;
            }
        }
        let r = AtlasPackRect { id, x: self.current_shelf_x, y: self.current_shelf_y, width, height };
        self.current_shelf_x += pw;
        self.current_shelf_height = self.current_shelf_height.max(ph);
        self.rects.push(r.clone());
        Some(r)
    }

    pub fn reset(&mut self) {
        self.rects.clear();
        self.current_shelf_x = 0;
        self.current_shelf_y = 0;
        self.current_shelf_height = 0;
        self.pages = 1;
    }

    pub fn utilization(&self) -> f32 {
        let used: u32 = self.rects.iter().map(|r| r.width * r.height).sum();
        let total = self.atlas_width * self.atlas_height * self.pages;
        used as f32 / total as f32
    }
}

// ---------------------------------------------------------------------------
// Font asset / registry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FontAsset {
    pub id: u64,
    pub family: String,
    pub faces: Vec<FontFace>,
    pub atlas_packer: AtlasPacker,
    pub fallback_font: Option<u64>,
}

impl FontAsset {
    pub fn new(id: u64, family: impl Into<String>) -> Self {
        let family_str: String = family.into();
        let mut regular = FontFace::new(family_str.clone(), FontStyle::Regular);
        regular.populate_ascii();
        let mut bold = FontFace::new(family_str.clone(), FontStyle::Bold);
        bold.populate_ascii();
        let mut italic = FontFace::new(family_str.clone(), FontStyle::Italic);
        italic.populate_ascii();
        Self {
            id,
            family: family_str,
            faces: vec![regular, bold, italic],
            atlas_packer: AtlasPacker::new(512, 512),
            fallback_font: None,
        }
    }

    pub fn get_face(&self, style: FontStyle) -> Option<&FontFace> {
        self.faces.iter().find(|f| f.style == style)
    }

    pub fn measure_text(&self, text: &str, size: f32, style: FontStyle) -> Vec2 {
        self.get_face(style).map(|f| f.measure_text(text, size)).unwrap_or(Vec2::ZERO)
    }
}

// ---------------------------------------------------------------------------
// Font editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontEditorTab { Preview, GlyphMap, AtlasViewer, Metrics, KerningTable, Settings }

#[derive(Debug, Clone)]
pub struct FontEditorState {
    pub fonts: Vec<FontAsset>,
    pub selected_font: Option<u64>,
    pub selected_glyph: Option<u32>,
    pub active_tab: FontEditorTab,
    pub preview_text: String,
    pub preview_size: f32,
    pub preview_style: FontStyle,
    pub preview_color: Vec4,
    pub preview_background: Vec4,
    pub layout_settings: TextLayoutSettings,
    pub show_baseline: bool,
    pub show_ascender: bool,
    pub show_descender: bool,
    pub show_cap_height: bool,
    pub show_x_height: bool,
    pub show_advance: bool,
    pub show_bearings: bool,
    pub atlas_zoom: f32,
    pub search_query: String,
    pub unicode_page: u32,
}

impl FontEditorState {
    pub fn new() -> Self {
        let fonts = vec![
            FontAsset::new(1, "Inter"),
            FontAsset::new(2, "JetBrains Mono"),
            FontAsset::new(3, "Roboto"),
        ];
        Self {
            fonts,
            selected_font: Some(1),
            selected_glyph: Some(b'A' as u32),
            active_tab: FontEditorTab::Preview,
            preview_text: "The quick brown fox jumps over the lazy dog.\nABCDEFGHIJKLMNOPQRSTUVWXYZ\n0123456789".into(),
            preview_size: 24.0,
            preview_style: FontStyle::Regular,
            preview_color: Vec4::ONE,
            preview_background: Vec4::new(0.1, 0.1, 0.1, 1.0),
            layout_settings: TextLayoutSettings::default(),
            show_baseline: true,
            show_ascender: false,
            show_descender: false,
            show_cap_height: false,
            show_x_height: false,
            show_advance: false,
            show_bearings: false,
            atlas_zoom: 1.0,
            search_query: String::new(),
            unicode_page: 0,
        }
    }

    pub fn selected_font(&self) -> Option<&FontAsset> {
        self.selected_font.and_then(|id| self.fonts.iter().find(|f| f.id == id))
    }

    pub fn compute_preview_layout(&self) -> Option<TextLayoutResult> {
        let font_asset = self.selected_font()?;
        let face = font_asset.get_face(self.preview_style)?;
        let span = TextSpan {
            text: self.preview_text.clone(),
            font_size: self.preview_size,
            color: self.preview_color,
            style: self.preview_style,
            ..Default::default()
        };
        Some(layout_text(&[span], face, &self.layout_settings))
    }

    pub fn selected_glyph_metrics(&self) -> Option<&GlyphMetrics> {
        let font = self.selected_font()?;
        let face = font.get_face(self.preview_style)?;
        let cp = self.selected_glyph?;
        face.glyph(cp)
    }

    pub fn atlas_utilization(&self) -> f32 {
        self.selected_font()
            .map(|f| f.atlas_packer.utilization())
            .unwrap_or(0.0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glyph_measure() {
        let mut face = FontFace::new("Test", FontStyle::Regular);
        face.populate_ascii();
        let sz = face.measure_text("Hello", 16.0);
        assert!(sz.x > 0.0);
    }

    #[test]
    fn test_text_layout() {
        let mut face = FontFace::new("Test", FontStyle::Regular);
        face.populate_ascii();
        let span = TextSpan { text: "Hello World".into(), font_size: 16.0, ..Default::default() };
        let result = layout_text(&[span], &face, &TextLayoutSettings::default());
        assert!(!result.glyphs.is_empty());
    }

    #[test]
    fn test_atlas_packer() {
        let mut packer = AtlasPacker::new(128, 128);
        packer.pack(1, 32, 32);
        packer.pack(2, 32, 32);
        assert_eq!(packer.rects.len(), 2);
        assert!(packer.utilization() > 0.0);
    }

    #[test]
    fn test_font_editor() {
        let ed = FontEditorState::new();
        assert!(!ed.fonts.is_empty());
        let layout = ed.compute_preview_layout();
        assert!(layout.is_some());
    }
}
