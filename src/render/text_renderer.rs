//! Text layout and rendering — monospace glyph sequences with rich text support.
//!
//! `TextBlock` lays out a string as a grid of per-character glyphs in world or
//! screen space.  Rich text markup `[color:r,g,b]text[/color]` is supported for
//! color changes mid-string.  A typewriter effect is built in.

use glam::{Vec2, Vec3, Vec4};
use crate::glyph::{Glyph, GlyphPool, BlendMode, RenderLayer};

// ── Text alignment ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextBaseline {
    #[default]
    Top,
    Middle,
    Bottom,
}

// ── Span (rich text segment) ──────────────────────────────────────────────────

/// A run of text with uniform style.
#[derive(Clone, Debug)]
pub struct TextSpan {
    pub text:     String,
    pub color:    Vec4,
    pub emission: f32,
    pub scale:    Vec2,
    pub layer:    RenderLayer,
    pub blend:    BlendMode,
}

impl TextSpan {
    pub fn plain(text: &str) -> Self {
        Self {
            text:     text.into(),
            color:    Vec4::ONE,
            emission: 0.0,
            scale:    Vec2::ONE,
            layer:    RenderLayer::UI,
            blend:    BlendMode::Normal,
        }
    }

    pub fn colored(text: &str, color: Vec4) -> Self {
        Self { text: text.into(), color, ..Self::plain("") }
    }

    pub fn glowing(text: &str, color: Vec4, emission: f32) -> Self {
        Self { text: text.into(), color, emission, ..Self::plain("") }
    }
}

// ── Rich text parser ──────────────────────────────────────────────────────────

/// Parses a simple markup string into a list of `TextSpan`s.
///
/// Supported tags:
/// - `[color:r,g,b]text[/color]` — RGB color (0..1 floats)
/// - `[rgba:r,g,b,a]text[/rgba]` — RGBA color
/// - `[emit:v]text[/emit]` — emission strength
/// - `[scale:x,y]text[/scale]` — per-span scale
/// - `[bold]text[/bold]` — treated as scale:1.2,1.2
/// - `[wave]text[/wave]` — marks span for wavy animation (emission > 0)
pub fn parse_rich_text(markup: &str) -> Vec<TextSpan> {
    let mut spans  = Vec::new();
    let mut stack: Vec<TextSpan> = Vec::new();
    let mut cursor = 0_usize;
    let bytes = markup.as_bytes();

    // Current style defaults
    let mut color    = Vec4::ONE;
    let mut emission = 0.0_f32;
    let mut scale    = Vec2::ONE;

    let mut text_buf = String::new();

    macro_rules! flush {
        () => {
            if !text_buf.is_empty() {
                spans.push(TextSpan {
                    text:     std::mem::take(&mut text_buf),
                    color, emission, scale,
                    layer: RenderLayer::UI,
                    blend: BlendMode::Normal,
                });
            }
        }
    }

    while cursor < bytes.len() {
        if bytes[cursor] == b'[' {
            // Find closing ]
            if let Some(end) = markup[cursor..].find(']') {
                let tag = &markup[cursor+1 .. cursor+end];
                cursor += end + 1;

                if tag.starts_with('/') {
                    // Closing tag — restore style from stack
                    flush!();
                    if let Some(saved) = stack.pop() {
                        color    = saved.color;
                        emission = saved.emission;
                        scale    = saved.scale;
                    }
                } else if let Some(rest) = tag.strip_prefix("color:") {
                    flush!();
                    stack.push(TextSpan { color, emission, scale, ..TextSpan::plain("") });
                    let parts: Vec<f32> = rest.split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                    if parts.len() >= 3 {
                        color = Vec4::new(parts[0], parts[1], parts[2], 1.0);
                    }
                } else if let Some(rest) = tag.strip_prefix("rgba:") {
                    flush!();
                    stack.push(TextSpan { color, emission, scale, ..TextSpan::plain("") });
                    let parts: Vec<f32> = rest.split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                    if parts.len() >= 4 {
                        color = Vec4::new(parts[0], parts[1], parts[2], parts[3]);
                    }
                } else if let Some(rest) = tag.strip_prefix("emit:") {
                    flush!();
                    stack.push(TextSpan { color, emission, scale, ..TextSpan::plain("") });
                    emission = rest.trim().parse().unwrap_or(0.0);
                } else if let Some(rest) = tag.strip_prefix("scale:") {
                    flush!();
                    stack.push(TextSpan { color, emission, scale, ..TextSpan::plain("") });
                    let parts: Vec<f32> = rest.split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                    if parts.len() >= 2 {
                        scale = Vec2::new(parts[0], parts[1]);
                    }
                } else if tag == "bold" {
                    flush!();
                    stack.push(TextSpan { color, emission, scale, ..TextSpan::plain("") });
                    scale = Vec2::new(1.15, 1.15);
                } else if tag == "wave" {
                    flush!();
                    stack.push(TextSpan { color, emission, scale, ..TextSpan::plain("") });
                    emission = 0.5;
                    color    = Vec4::new(0.7, 0.9, 1.0, 1.0);
                }
                continue;
            }
        }

        // Regular character
        text_buf.push(markup[cursor..].chars().next().unwrap_or(' '));
        cursor += markup[cursor..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    flush!();

    if spans.is_empty() {
        spans.push(TextSpan::plain(markup));
    }
    spans
}

// ── TextBlock ─────────────────────────────────────────────────────────────────

/// A laid-out block of text.  Position is the top-left corner in world/screen space.
#[derive(Clone, Debug)]
pub struct TextBlock {
    pub spans:       Vec<TextSpan>,
    pub position:    Vec3,
    pub char_width:  f32,
    pub char_height: f32,
    pub max_width:   Option<f32>,    // word wrap: max columns (chars)
    pub align:       TextAlign,
    pub baseline:    TextBaseline,
    pub layer:       RenderLayer,
    pub blend:       BlendMode,
    pub visible:     bool,
    pub z_offset:    f32,
}

impl TextBlock {
    pub fn new(text: &str, position: Vec3) -> Self {
        Self {
            spans:       vec![TextSpan::plain(text)],
            position,
            char_width:  0.6,
            char_height: 1.0,
            max_width:   None,
            align:       TextAlign::Left,
            baseline:    TextBaseline::Top,
            layer:       RenderLayer::UI,
            blend:       BlendMode::Normal,
            visible:     true,
            z_offset:    0.0,
        }
    }

    pub fn rich(markup: &str, position: Vec3) -> Self {
        Self {
            spans: parse_rich_text(markup),
            ..Self::new("", position)
        }
    }

    pub fn with_color(mut self, c: Vec4) -> Self {
        for s in &mut self.spans { s.color = c; }
        self
    }

    pub fn with_scale(mut self, w: f32, h: f32) -> Self {
        self.char_width  = w;
        self.char_height = h;
        self
    }

    pub fn with_align(mut self, a: TextAlign) -> Self { self.align = a; self }
    pub fn with_max_width(mut self, w: f32) -> Self { self.max_width = Some(w); self }
    pub fn with_layer(mut self, l: RenderLayer) -> Self { self.layer = l; self }

    /// Full text content (all spans concatenated).
    pub fn full_text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }

    /// Lay out characters into positions.  Returns (char, Vec3, Vec4, f32_emission) tuples.
    pub fn layout(&self) -> Vec<CharLayout> {
        let full = self.full_text();
        let lines = self.wrap_lines(&full);
        let total_height = lines.len() as f32 * self.char_height;

        let baseline_offset = match self.baseline {
            TextBaseline::Top    => 0.0,
            TextBaseline::Middle => -total_height * 0.5,
            TextBaseline::Bottom => -total_height,
        };

        let mut result = Vec::new();
        let mut span_iter = SpanCharIter::new(&self.spans);

        for (row, line) in lines.iter().enumerate() {
            let line_width = line.chars().count() as f32 * self.char_width;
            let x_offset = match self.align {
                TextAlign::Left   => 0.0,
                TextAlign::Center => -line_width * 0.5,
                TextAlign::Right  => -line_width,
            };

            for (col, _ch) in line.chars().enumerate() {
                let x = self.position.x + x_offset + col as f32 * self.char_width;
                let y = self.position.y + baseline_offset - row as f32 * self.char_height;
                let z = self.position.z + self.z_offset;

                if let Some((ch, color, emission, scale)) = span_iter.next() {
                    result.push(CharLayout {
                        ch,
                        position: Vec3::new(x, y, z),
                        color,
                        emission,
                        scale,
                        layer:    self.layer,
                        blend:    self.blend,
                    });
                }
            }
        }
        result
    }

    fn wrap_lines<'a>(&self, text: &'a str) -> Vec<String> {
        if let Some(max_w) = self.max_width {
            let max_chars = (max_w / self.char_width.max(0.01)) as usize;
            wrap_text(text, max_chars)
        } else {
            text.lines().map(|l| l.to_string()).collect()
        }
    }

    /// Spawn all characters into a GlyphPool.
    pub fn spawn_into(&self, pool: &mut GlyphPool) -> Vec<crate::glyph::GlyphId> {
        let mut ids = Vec::new();
        if !self.visible { return ids; }
        for cl in self.layout() {
            let g = Glyph {
                character:  cl.ch,
                position:   cl.position,
                color:      cl.color,
                emission:   cl.emission,
                scale:      cl.scale,
                layer:      cl.layer,
                blend_mode: cl.blend,
                visible:    true,
                ..Glyph::default()
            };
            ids.push(pool.spawn(g));
        }
        ids
    }
}

/// A single character's layout result.
#[derive(Clone, Debug)]
pub struct CharLayout {
    pub ch:       char,
    pub position: Vec3,
    pub color:    Vec4,
    pub emission: f32,
    pub scale:    Vec2,
    pub layer:    RenderLayer,
    pub blend:    BlendMode,
}

/// Iterates over characters in a span list, yielding per-character style info.
struct SpanCharIter<'a> {
    spans:      &'a [TextSpan],
    span_idx:   usize,
    char_idx:   usize,
}

impl<'a> SpanCharIter<'a> {
    fn new(spans: &'a [TextSpan]) -> Self {
        Self { spans, span_idx: 0, char_idx: 0 }
    }

    fn next(&mut self) -> Option<(char, Vec4, f32, Vec2)> {
        loop {
            let span = self.spans.get(self.span_idx)?;
            let ch   = span.text.chars().nth(self.char_idx);
            if let Some(ch) = ch {
                self.char_idx += 1;
                if ch == '\n' { continue; } // skip newlines — handled by layout
                return Some((ch, span.color, span.emission, span.scale));
            } else {
                self.span_idx += 1;
                self.char_idx  = 0;
            }
        }
    }
}

/// Word-wrap `text` to at most `max_chars` per line.
pub fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() { lines.push(String::new()); continue; }
        let words: Vec<&str> = paragraph.split_whitespace().collect();
        let mut line = String::new();
        for word in words {
            if line.is_empty() {
                // Long single word — hard break it
                if word.len() > max_chars {
                    let mut w = word;
                    while w.len() > max_chars {
                        lines.push(w[..max_chars].to_string());
                        w = &w[max_chars..];
                    }
                    line = w.to_string();
                } else {
                    line = word.to_string();
                }
            } else if line.len() + 1 + word.len() <= max_chars {
                line.push(' ');
                line.push_str(word);
            } else {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            }
        }
        if !line.is_empty() { lines.push(line); }
    }
    if lines.is_empty() { lines.push(String::new()); }
    lines
}

// ── Typewriter text block ─────────────────────────────────────────────────────

/// A TextBlock with a typewriter reveal effect.
pub struct TypewriterBlock {
    pub block:          TextBlock,
    pub chars_per_sec:  f32,
    revealed_chars:     usize,
    accumulator:        f32,
    pub complete:       bool,
    pause_timer:        f32,
    full_char_count:    usize,
    /// Optional sound trigger on each revealed character.
    pub on_char:        Option<Box<dyn Fn(char) + Send + Sync>>,
}

impl TypewriterBlock {
    pub fn new(block: TextBlock, chars_per_sec: f32) -> Self {
        let full = block.full_text().chars().count();
        Self {
            block,
            chars_per_sec,
            revealed_chars:  0,
            accumulator:     0.0,
            complete:        full == 0,
            pause_timer:     0.0,
            full_char_count: full,
            on_char:         None,
        }
    }

    pub fn with_char_callback(mut self, f: impl Fn(char) + Send + Sync + 'static) -> Self {
        self.on_char = Some(Box::new(f));
        self
    }

    pub fn tick(&mut self, dt: f32) {
        if self.complete { return; }
        if self.pause_timer > 0.0 {
            self.pause_timer -= dt;
            return;
        }
        self.accumulator += dt * self.chars_per_sec;
        let new = self.accumulator as usize;
        self.accumulator -= new as f32;

        let full_text = self.block.full_text();
        for _ in 0..new {
            if self.revealed_chars >= self.full_char_count { break; }
            let ch = full_text.chars().nth(self.revealed_chars).unwrap_or(' ');
            self.revealed_chars += 1;
            if let Some(f) = &self.on_char { f(ch); }
            match ch {
                '.' | '!' | '?' => self.pause_timer = 0.2,
                ',' | ';' | ':' => self.pause_timer = 0.08,
                _ => {}
            }
        }
        if self.revealed_chars >= self.full_char_count {
            self.complete = true;
        }
    }

    pub fn skip(&mut self) {
        self.revealed_chars = self.full_char_count;
        self.complete       = true;
    }

    pub fn progress(&self) -> f32 {
        if self.full_char_count == 0 { 1.0 }
        else { self.revealed_chars as f32 / self.full_char_count as f32 }
    }

    /// Build a truncated TextBlock showing only revealed characters.
    pub fn visible_block(&self) -> TextBlock {
        if self.complete { return self.block.clone(); }
        let full = self.block.full_text();
        let visible: String = full.chars().take(self.revealed_chars).collect();
        TextBlock {
            spans: vec![TextSpan { text: visible, ..self.block.spans[0].clone() }],
            ..self.block.clone()
        }
    }

    /// Spawn visible characters into a GlyphPool.
    pub fn spawn_into(&self, pool: &mut GlyphPool) -> Vec<crate::glyph::GlyphId> {
        self.visible_block().spawn_into(pool)
    }
}

// ── ScrollingText ─────────────────────────────────────────────────────────────

/// A scrolling text display — think: terminal output, combat log, news ticker.
pub struct ScrollingText {
    pub lines:      Vec<String>,
    pub max_lines:  usize,
    /// How many lines are visible at once.
    pub visible:    usize,
    pub scroll_pos: usize,
    pub position:   Vec3,
    pub char_width: f32,
    pub char_height: f32,
    pub color:      Vec4,
    pub layer:      RenderLayer,
    /// Auto-scroll speed (lines per second). 0 = manual only.
    pub auto_scroll: f32,
    scroll_accum:   f32,
}

impl ScrollingText {
    pub fn new(position: Vec3, visible: usize) -> Self {
        Self {
            lines:       Vec::new(),
            max_lines:   1000,
            visible,
            scroll_pos:  0,
            position,
            char_width:  0.6,
            char_height: 1.0,
            color:       Vec4::ONE,
            layer:       RenderLayer::UI,
            auto_scroll: 0.0,
            scroll_accum: 0.0,
        }
    }

    pub fn push(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
        if self.lines.len() > self.max_lines {
            self.lines.remove(0);
        }
        // Auto-jump to bottom
        if self.lines.len() > self.visible {
            self.scroll_pos = self.lines.len() - self.visible;
        }
    }

    pub fn scroll_up(&mut self)   { self.scroll_pos = self.scroll_pos.saturating_sub(1); }
    pub fn scroll_down(&mut self) {
        let max = self.lines.len().saturating_sub(self.visible);
        self.scroll_pos = (self.scroll_pos + 1).min(max);
    }

    pub fn tick(&mut self, dt: f32) {
        if self.auto_scroll > 0.0 {
            self.scroll_accum += dt * self.auto_scroll;
            while self.scroll_accum >= 1.0 {
                self.scroll_accum -= 1.0;
                self.scroll_down();
            }
        }
    }

    pub fn spawn_into(&self, pool: &mut GlyphPool) -> Vec<crate::glyph::GlyphId> {
        let mut ids = Vec::new();
        let end = (self.scroll_pos + self.visible).min(self.lines.len());
        for (row, line) in self.lines[self.scroll_pos..end].iter().enumerate() {
            let y = self.position.y - row as f32 * self.char_height;
            for (col, ch) in line.chars().enumerate() {
                let x = self.position.x + col as f32 * self.char_width;
                let g = Glyph {
                    character:  ch,
                    position:   Vec3::new(x, y, self.position.z),
                    color:      self.color,
                    layer:      self.layer,
                    visible:    true,
                    ..Glyph::default()
                };
                ids.push(pool.spawn(g));
            }
        }
        ids
    }
}

// ── Marquee ───────────────────────────────────────────────────────────────────

/// Horizontally scrolling marquee text.
pub struct Marquee {
    pub text:     String,
    pub position: Vec3,
    pub width:    f32,         // display width in world units
    pub char_w:   f32,
    pub speed:    f32,         // world units per second
    offset:       f32,         // current scroll offset
    pub color:    Vec4,
    pub layer:    RenderLayer,
}

impl Marquee {
    pub fn new(text: impl Into<String>, position: Vec3, width: f32, speed: f32) -> Self {
        Self {
            text: text.into(),
            position,
            width,
            char_w: 0.6,
            speed,
            offset: 0.0,
            color: Vec4::ONE,
            layer: RenderLayer::UI,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.offset += dt * self.speed;
        let total_width = self.text.chars().count() as f32 * self.char_w;
        if self.offset > total_width { self.offset = 0.0; }
    }

    pub fn spawn_into(&self, pool: &mut GlyphPool) -> Vec<crate::glyph::GlyphId> {
        let mut ids       = Vec::new();
        let max_chars     = (self.width / self.char_w.max(0.01)) as usize + 1;
        let total_chars   = self.text.chars().count();
        if total_chars == 0 { return ids; }

        let start_char = (self.offset / self.char_w) as usize;

        for i in 0..max_chars {
            let text_idx = (start_char + i) % total_chars;
            let ch = self.text.chars().nth(text_idx).unwrap_or(' ');
            let frac = (self.offset / self.char_w).fract();
            let x = self.position.x + (i as f32 - frac) * self.char_w;
            if x < self.position.x || x > self.position.x + self.width { continue; }
            let g = Glyph {
                character: ch,
                position:  Vec3::new(x, self.position.y, self.position.z),
                color:     self.color,
                layer:     self.layer,
                visible:   true,
                ..Glyph::default()
            };
            ids.push(pool.spawn(g));
        }
        ids
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_text_basic() {
        let lines = wrap_text("Hello world foo bar baz", 10);
        for l in &lines { assert!(l.len() <= 10, "Line too long: {}", l); }
        assert!(lines.len() >= 2);
    }

    #[test]
    fn wrap_text_empty() {
        let lines = wrap_text("", 20);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn wrap_text_single_word() {
        let lines = wrap_text("Hello", 20);
        assert_eq!(lines[0], "Hello");
    }

    #[test]
    fn wrap_long_word() {
        let lines = wrap_text("AAAAAAAAAA", 4);
        assert_eq!(lines[0], "AAAA");
        assert_eq!(lines[1], "AAAA");
        assert_eq!(lines[2], "AA");
    }

    #[test]
    fn parse_rich_text_plain() {
        let spans = parse_rich_text("Hello");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "Hello");
    }

    #[test]
    fn parse_rich_text_color() {
        let spans = parse_rich_text("[color:1.0,0.0,0.0]Red[/color] Normal");
        assert!(spans.len() >= 2);
        let red = &spans[0];
        assert!((red.color.x - 1.0).abs() < 0.01);
        assert!((red.color.y - 0.0).abs() < 0.01);
    }

    #[test]
    fn parse_rich_text_nested() {
        let spans = parse_rich_text("[color:0,1,0]Green [bold]BoldGreen[/bold][/color]");
        assert!(spans.iter().any(|s| (s.color.y - 1.0).abs() < 0.01));
    }

    #[test]
    fn text_block_layout() {
        let block = TextBlock::new("Hello", Vec3::ZERO);
        let layout = block.layout();
        assert_eq!(layout.len(), 5); // 5 chars
    }

    #[test]
    fn text_block_wrap() {
        let block = TextBlock::new("Hello World", Vec3::ZERO)
            .with_max_width(0.6 * 5.0); // 5 chars wide
        let layout = block.layout();
        // "Hello" on row 0, "World" on row 1 → max y different
        let max_y = layout.iter().map(|c| c.position.y).fold(f32::MIN, f32::max);
        let min_y = layout.iter().map(|c| c.position.y).fold(f32::MAX, f32::min);
        assert!(max_y > min_y, "Expect multiple rows");
    }

    #[test]
    fn typewriter_reveals_chars() {
        let block  = TextBlock::new("Hello", Vec3::ZERO);
        let mut tw = TypewriterBlock::new(block, 20.0);
        tw.tick(0.1);  // 2 chars
        assert!(tw.revealed_chars > 0);
        assert!(!tw.complete);
    }

    #[test]
    fn typewriter_completes() {
        let block  = TextBlock::new("Hi", Vec3::ZERO);
        let mut tw = TypewriterBlock::new(block, 100.0);
        tw.tick(1.0);
        assert!(tw.complete);
    }

    #[test]
    fn typewriter_skip() {
        let block  = TextBlock::new("Long text", Vec3::ZERO);
        let mut tw = TypewriterBlock::new(block, 2.0);
        tw.skip();
        assert!(tw.complete);
        assert_eq!(tw.progress(), 1.0);
    }

    #[test]
    fn scrolling_text_push_and_scroll() {
        let mut log = ScrollingText::new(Vec3::ZERO, 3);
        log.push("Line 1");
        log.push("Line 2");
        log.push("Line 3");
        log.push("Line 4");
        assert_eq!(log.lines.len(), 4);
        assert!(log.scroll_pos >= 1); // should have scrolled to show Line 4
    }

    #[test]
    fn marquee_wraps() {
        let mut m = Marquee::new("ABCDE", Vec3::ZERO, 3.0, 1.0);
        for _ in 0..300 { m.tick(0.016); }
        // offset should not grow without bound
        assert!(m.offset < m.text.len() as f32 * m.char_w + 1.0);
    }
}
