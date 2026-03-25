//! Render settings panel — real-time control of all post-processing effects.
//!
//! Controls bloom, chromatic aberration, film grain, vignette, scanlines,
//! motion blur, color grading, exposure, and tone mapping.

use glam::Vec4;
use proof_engine::prelude::*;
use crate::widgets::{WidgetTheme, WidgetDraw, Rect};
use crate::widgets::slider::Slider;
use crate::widgets::common::Toggle;

pub struct RenderSettingsPanel {
    pub bloom_enabled: Toggle,
    pub bloom_intensity: Slider,
    pub bloom_radius: Slider,
    pub chromatic_aberration: Slider,
    pub film_grain: Slider,
    pub vignette_strength: Slider,
    pub vignette_radius: Slider,
    pub scanline_intensity: Slider,
    pub scanline_count: Slider,
    pub motion_blur: Toggle,
    pub motion_blur_strength: Slider,
    pub exposure: Slider,
    pub saturation: Slider,
    pub contrast: Slider,
    pub brightness: Slider,
    pub hue_shift: Slider,
    pub temperature: Slider,
    pub gamma: Slider,
    pub dithering: Toggle,
    pub fxaa_enabled: Toggle,
    pub scroll_offset: f32,
    pub collapsed: bool,
}

impl RenderSettingsPanel {
    pub fn new(x: f32, y: f32, width: f32) -> Self {
        let w = width;
        let mut sy = y;
        let mk_slider = |label: &str, val: f32, min: f32, max: f32, y: &mut f32| -> Slider {
            *y -= 0.6;
            Slider::new(label, val, min, max, x, *y, w)
        };
        let mk_toggle = |label: &str, val: bool, y: &mut f32| -> Toggle {
            *y -= 0.6;
            Toggle::new(label, val, x, *y)
        };

        Self {
            bloom_enabled: mk_toggle("Bloom", true, &mut sy),
            bloom_intensity: mk_slider("Intensity", 1.5, 0.0, 5.0, &mut sy),
            bloom_radius: mk_slider("Radius", 8.0, 1.0, 32.0, &mut sy),
            chromatic_aberration: mk_slider("Chromatic", 0.002, 0.0, 0.02, &mut sy),
            film_grain: mk_slider("Grain", 0.01, 0.0, 0.1, &mut sy),
            vignette_strength: mk_slider("Vignette", 0.3, 0.0, 1.0, &mut sy),
            vignette_radius: mk_slider("Vig.Radius", 0.8, 0.0, 2.0, &mut sy),
            scanline_intensity: mk_slider("Scanlines", 0.0, 0.0, 1.0, &mut sy),
            scanline_count: mk_slider("SL Count", 240.0, 60.0, 1080.0, &mut sy),
            motion_blur: mk_toggle("MotionBlur", false, &mut sy),
            motion_blur_strength: mk_slider("MB Str", 0.5, 0.0, 1.0, &mut sy),
            exposure: mk_slider("Exposure", 1.0, 0.1, 5.0, &mut sy),
            saturation: mk_slider("Saturation", 1.0, 0.0, 2.0, &mut sy),
            contrast: mk_slider("Contrast", 1.0, 0.5, 2.0, &mut sy),
            brightness: mk_slider("Brightness", 0.0, -0.5, 0.5, &mut sy),
            hue_shift: mk_slider("HueShift", 0.0, -1.0, 1.0, &mut sy),
            temperature: mk_slider("Temp", 0.0, -1.0, 1.0, &mut sy),
            gamma: mk_slider("Gamma", 2.2, 1.0, 3.0, &mut sy),
            dithering: mk_toggle("Dithering", false, &mut sy),
            fxaa_enabled: mk_toggle("FXAA", false, &mut sy),
            scroll_offset: 0.0,
            collapsed: false,
        }
    }

    /// Apply current settings to the engine config.
    pub fn apply_to_config(&self, config: &mut proof_engine::config::RenderConfig) {
        config.bloom_enabled = self.bloom_enabled.value;
        config.bloom_intensity = self.bloom_intensity.value;
        config.bloom_radius = self.bloom_radius.value;
        config.chromatic_aberration = self.chromatic_aberration.value;
        config.film_grain = self.film_grain.value;
        config.motion_blur_enabled = self.motion_blur.value;
    }

    /// Read current engine config into the panel.
    pub fn read_from_config(&mut self, config: &proof_engine::config::RenderConfig) {
        self.bloom_enabled.value = config.bloom_enabled;
        self.bloom_intensity.set_value(config.bloom_intensity);
        self.bloom_radius.set_value(config.bloom_radius);
        self.chromatic_aberration.set_value(config.chromatic_aberration);
        self.film_grain.set_value(config.film_grain);
        self.motion_blur.value = config.motion_blur_enabled;
    }

    /// Render all controls.
    pub fn render(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        if self.collapsed {
            WidgetDraw::text(engine, x, y, "> Render Settings", theme.fg, 0.1, RenderLayer::UI);
            return;
        }

        WidgetDraw::text(engine, x, y, "RENDER SETTINGS", theme.accent, 0.25, RenderLayer::UI);
        WidgetDraw::separator(engine, x, y - 0.5, width, theme.separator);

        self.bloom_enabled.render(engine, theme);
        self.bloom_intensity.render(engine, theme);
        self.bloom_radius.render(engine, theme);
        self.chromatic_aberration.render(engine, theme);
        self.film_grain.render(engine, theme);
        self.vignette_strength.render(engine, theme);
        self.vignette_radius.render(engine, theme);
        self.scanline_intensity.render(engine, theme);
        self.motion_blur.render(engine, theme);
        self.motion_blur_strength.render(engine, theme);

        WidgetDraw::separator(engine, x, y - 7.5, width, theme.separator);
        WidgetDraw::text(engine, x, y - 7.8, "COLOR GRADING", theme.accent, 0.2, RenderLayer::UI);

        self.exposure.render(engine, theme);
        self.saturation.render(engine, theme);
        self.contrast.render(engine, theme);
        self.brightness.render(engine, theme);
        self.hue_shift.render(engine, theme);
        self.temperature.render(engine, theme);
        self.gamma.render(engine, theme);

        WidgetDraw::separator(engine, x, y - 13.0, width, theme.separator);
        self.dithering.render(engine, theme);
        self.fxaa_enabled.render(engine, theme);
    }

    /// Reset all to defaults.
    pub fn reset_defaults(&mut self) {
        self.bloom_enabled.value = true;
        self.bloom_intensity.set_value(1.5);
        self.bloom_radius.set_value(8.0);
        self.chromatic_aberration.set_value(0.002);
        self.film_grain.set_value(0.01);
        self.vignette_strength.set_value(0.3);
        self.motion_blur.value = false;
        self.exposure.set_value(1.0);
        self.saturation.set_value(1.0);
        self.contrast.set_value(1.0);
        self.brightness.set_value(0.0);
        self.hue_shift.set_value(0.0);
        self.gamma.set_value(2.2);
    }
}

/// Quick presets for render settings.
pub struct RenderPresets;

impl RenderPresets {
    pub fn cinematic(panel: &mut RenderSettingsPanel) {
        panel.bloom_intensity.set_value(2.0);
        panel.chromatic_aberration.set_value(0.005);
        panel.film_grain.set_value(0.03);
        panel.vignette_strength.set_value(0.5);
        panel.saturation.set_value(0.9);
        panel.contrast.set_value(1.2);
    }

    pub fn retro(panel: &mut RenderSettingsPanel) {
        panel.bloom_intensity.set_value(0.5);
        panel.scanline_intensity.set_value(0.3);
        panel.film_grain.set_value(0.05);
        panel.saturation.set_value(0.7);
        panel.chromatic_aberration.set_value(0.008);
    }

    pub fn neon(panel: &mut RenderSettingsPanel) {
        panel.bloom_intensity.set_value(3.0);
        panel.bloom_radius.set_value(16.0);
        panel.saturation.set_value(1.5);
        panel.chromatic_aberration.set_value(0.003);
        panel.vignette_strength.set_value(0.4);
    }

    pub fn clean(panel: &mut RenderSettingsPanel) {
        panel.reset_defaults();
        panel.bloom_intensity.set_value(1.0);
        panel.film_grain.set_value(0.0);
        panel.chromatic_aberration.set_value(0.0);
    }
}
