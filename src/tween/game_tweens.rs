//! Game-specific tween presets — ready-to-call functions that wire the tween
//! engine to specific game events: menu navigation, combat, screen transitions,
//! stat changes, level ups, death, victory, and more.
//!
//! Each function takes a `&mut TweenManager` and the minimum context needed
//! (glyph IDs, bar IDs, positions, values), starts the relevant tweens, and
//! returns the tween IDs for optional cancellation.

use std::f32::consts::TAU;
use glam::{Vec3, Vec4};

use super::easing::Easing;
use super::tween_manager::{TweenManager, TweenTarget, TweenId, BarId};
use crate::glyph::GlyphId;

// ═══════════════════════════════════════════════════════════════════════════
// MENU
// ═══════════════════════════════════════════════════════════════════════════

/// Slide the menu cursor glyph from one Y position to another.
pub fn menu_cursor_slide(
    mgr: &mut TweenManager,
    cursor_glyph: GlyphId,
    from_y: f32,
    to_y: f32,
) -> TweenId {
    mgr.cancel_glyph(cursor_glyph);
    mgr.start(
        TweenTarget::GlyphPositionY(cursor_glyph),
        from_y, to_y, 0.1,
        Easing::EaseOutCubic,
    )
}

/// Pulse the cursor glyph's emission to indicate it's selected.
pub fn menu_cursor_pulse(
    mgr: &mut TweenManager,
    cursor_glyph: GlyphId,
) -> TweenId {
    mgr.start(
        TweenTarget::GlyphEmission(cursor_glyph),
        0.3, 0.8, 0.6,
        Easing::EaseInOutSine,
    )
}

/// Menu selection confirm: flash bright then fade.
pub fn menu_select_flash(
    mgr: &mut TweenManager,
    glyphs: &[GlyphId],
) -> Vec<TweenId> {
    let mut ids = Vec::new();
    for &glyph in glyphs {
        ids.push(mgr.start(
            TweenTarget::GlyphEmission(glyph),
            0.0, 2.0, 0.08,
            Easing::EaseOutExpo,
        ));
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphEmission(glyph),
            2.0, 0.0, 0.2, 0.08,
            Easing::EaseInQuad,
        ));
    }
    ids
}

/// Theme/screen crossfade: fade out old, swap, fade in new.
pub fn screen_crossfade(
    mgr: &mut TweenManager,
    fade_duration: f32,
    on_midpoint: impl FnOnce(&mut TweenManager) + Send + 'static,
) -> TweenId {
    mgr.start_with_callback(
        TweenTarget::ScreenFade,
        0.0, 1.0, fade_duration,
        Easing::EaseInQuad,
        move |mgr| {
            on_midpoint(mgr);
            mgr.start(
                TweenTarget::ScreenFade,
                1.0, 0.0, fade_duration,
                Easing::EaseOutQuad,
            );
        },
    )
}

/// Instant fade to black, hold, then fade in.
pub fn screen_fade_through_black(
    mgr: &mut TweenManager,
    fade_out: f32,
    hold: f32,
    fade_in: f32,
) -> TweenId {
    mgr.start_with_callback(
        TweenTarget::ScreenFade,
        0.0, 1.0, fade_out,
        Easing::EaseInQuad,
        move |mgr| {
            mgr.start_delayed(
                TweenTarget::ScreenFade,
                1.0, 0.0, fade_in, hold,
                Easing::EaseOutQuad,
            );
        },
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// HP / MP / BARS
// ═══════════════════════════════════════════════════════════════════════════

/// Smoothly animate an HP bar from current to new percentage.
pub fn hp_bar_change(
    mgr: &mut TweenManager,
    bar: BarId,
    from_pct: f32,
    to_pct: f32,
) -> TweenId {
    mgr.start_tagged(
        TweenTarget::BarFillPercent(bar),
        from_pct, to_pct, 0.3,
        Easing::EaseOutQuad,
        "hp_bar",
    )
}

/// Ghost bar (recent damage indicator): delays, then slides down.
pub fn hp_ghost_bar(
    mgr: &mut TweenManager,
    bar: BarId,
    from_pct: f32,
    to_pct: f32,
) -> TweenId {
    mgr.start_delayed(
        TweenTarget::BarGhostPercent(bar),
        from_pct, to_pct, 0.5, 0.3,
        Easing::EaseInQuad,
    )
}

/// Full HP change: animate both fill and ghost bar.
pub fn hp_bar_damage(
    mgr: &mut TweenManager,
    bar: BarId,
    old_pct: f32,
    new_pct: f32,
) -> (TweenId, TweenId) {
    let fill = hp_bar_change(mgr, bar, old_pct, new_pct);
    let ghost = hp_ghost_bar(mgr, bar, old_pct, new_pct);
    (fill, ghost)
}

/// MP bar smooth change (no ghost).
pub fn mp_bar_change(
    mgr: &mut TweenManager,
    bar: BarId,
    from_pct: f32,
    to_pct: f32,
) -> TweenId {
    mgr.start_tagged(
        TweenTarget::BarFillPercent(bar),
        from_pct, to_pct, 0.25,
        Easing::EaseOutCubic,
        "mp_bar",
    )
}

/// XP bar fill (slower, more satisfying).
pub fn xp_bar_fill(
    mgr: &mut TweenManager,
    bar: BarId,
    from_pct: f32,
    to_pct: f32,
) -> TweenId {
    mgr.start(
        TweenTarget::BarFillPercent(bar),
        from_pct, to_pct, 0.8,
        Easing::EaseOutBack,
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// DAMAGE NUMBERS
// ═══════════════════════════════════════════════════════════════════════════

/// Spawn a damage number arc: rises, scales from big to normal, fades out.
pub fn damage_number_arc(
    mgr: &mut TweenManager,
    glyph: GlyphId,
    start_y: f32,
    rise_height: f32,
) -> Vec<TweenId> {
    vec![
        // Rise upward
        mgr.start(
            TweenTarget::GlyphPositionY(glyph),
            start_y, start_y + rise_height, 1.0,
            Easing::EaseOutQuad,
        ),
        // Fade out
        mgr.start(
            TweenTarget::GlyphAlpha(glyph),
            1.0, 0.0, 1.0,
            Easing::EaseInQuad,
        ),
        // Start big, spring to normal size
        mgr.start(
            TweenTarget::GlyphScale(glyph),
            2.0, 1.0, 0.2,
            Easing::Spring { stiffness: 8.0, damping: 0.4 },
        ),
    ]
}

/// Critical hit damage number: bigger arc, gold flash, longer duration.
pub fn crit_damage_number(
    mgr: &mut TweenManager,
    glyph: GlyphId,
    start_y: f32,
    rise_height: f32,
) -> Vec<TweenId> {
    vec![
        mgr.start(
            TweenTarget::GlyphPositionY(glyph),
            start_y, start_y + rise_height * 1.5, 1.2,
            Easing::EaseOutQuad,
        ),
        mgr.start(
            TweenTarget::GlyphAlpha(glyph),
            1.0, 0.0, 1.2,
            Easing::EaseInCubic,
        ),
        mgr.start(
            TweenTarget::GlyphScale(glyph),
            3.0, 1.2, 0.3,
            Easing::Spring { stiffness: 6.0, damping: 0.3 },
        ),
        // Gold emission flash
        mgr.start(
            TweenTarget::GlyphEmission(glyph),
            3.0, 0.5, 0.3,
            Easing::EaseOutExpo,
        ),
    ]
}

/// Healing number: green, floats up gently.
pub fn heal_number(
    mgr: &mut TweenManager,
    glyph: GlyphId,
    start_y: f32,
) -> Vec<TweenId> {
    vec![
        mgr.start(
            TweenTarget::GlyphPositionY(glyph),
            start_y, start_y + 2.0, 0.8,
            Easing::EaseOutSine,
        ),
        mgr.start(
            TweenTarget::GlyphAlpha(glyph),
            1.0, 0.0, 0.8,
            Easing::EaseInQuad,
        ),
        mgr.start(
            TweenTarget::GlyphScale(glyph),
            1.5, 1.0, 0.15,
            Easing::EaseOutBack,
        ),
    ]
}

// ═══════════════════════════════════════════════════════════════════════════
// STAT CHANGES
// ═══════════════════════════════════════════════════════════════════════════

/// Flash a stat glyph gold when a stat increases.
pub fn stat_increase_flash(
    mgr: &mut TweenManager,
    glyph: GlyphId,
) -> Vec<TweenId> {
    vec![
        // Flash bright
        mgr.start(
            TweenTarget::GlyphEmission(glyph),
            0.0, 2.0, 0.1,
            Easing::EaseOutQuad,
        ),
        // Fade back
        mgr.start_delayed(
            TweenTarget::GlyphEmission(glyph),
            2.0, 0.0, 0.3, 0.1,
            Easing::EaseInQuad,
        ),
        // Scale pop
        mgr.start(
            TweenTarget::GlyphScale(glyph),
            1.0, 1.3, 0.08,
            Easing::EaseOutQuad,
        ),
        mgr.start_delayed(
            TweenTarget::GlyphScale(glyph),
            1.3, 1.0, 0.15, 0.08,
            Easing::EaseOutBack,
        ),
    ]
}

/// Flash red when a stat decreases.
pub fn stat_decrease_flash(
    mgr: &mut TweenManager,
    glyph: GlyphId,
) -> Vec<TweenId> {
    vec![
        mgr.start(
            TweenTarget::GlyphColorR(glyph),
            1.0, 1.0, 0.1,
            Easing::Flash,
        ),
        mgr.start(
            TweenTarget::GlyphEmission(glyph),
            0.0, 1.5, 0.08,
            Easing::EaseOutQuad,
        ),
        mgr.start_delayed(
            TweenTarget::GlyphEmission(glyph),
            1.5, 0.0, 0.2, 0.08,
            Easing::EaseInQuad,
        ),
    ]
}

// ═══════════════════════════════════════════════════════════════════════════
// STATUS EFFECTS
// ═══════════════════════════════════════════════════════════════════════════

/// Apply a status effect visual: tint + emission pulse.
pub fn status_effect_apply(
    mgr: &mut TweenManager,
    entity_glyphs: &[GlyphId],
    emission_color_intensity: f32,
) -> Vec<TweenId> {
    let mut ids = Vec::new();
    for &glyph in entity_glyphs {
        ids.push(mgr.start(
            TweenTarget::GlyphEmission(glyph),
            0.0, emission_color_intensity, 0.15,
            Easing::EaseOutExpo,
        ));
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphEmission(glyph),
            emission_color_intensity, 0.0, 0.3, 0.15,
            Easing::EaseInQuad,
        ));
    }
    ids
}

// ═══════════════════════════════════════════════════════════════════════════
// LEVEL UP
// ═══════════════════════════════════════════════════════════════════════════

/// Level up pulse ring: expanding ring of glyphs radiating outward.
///
/// Returns the glyph IDs and tween IDs for the ring particles.
/// The caller must spawn the glyphs before calling this.
pub fn level_up_ring(
    mgr: &mut TweenManager,
    ring_glyphs: &[GlyphId],
    center_x: f32,
    center_y: f32,
    max_radius: f32,
) -> Vec<TweenId> {
    let count = ring_glyphs.len();
    let mut ids = Vec::new();

    for (i, &glyph) in ring_glyphs.iter().enumerate() {
        let angle = i as f32 * TAU / count as f32;
        let target_x = center_x + angle.cos() * max_radius;
        let target_y = center_y + angle.sin() * max_radius;

        // Expand outward
        ids.push(mgr.start(
            TweenTarget::GlyphPositionX(glyph),
            center_x, target_x, 0.5,
            Easing::EaseOutQuad,
        ));
        ids.push(mgr.start(
            TweenTarget::GlyphPositionY(glyph),
            center_y, target_y, 0.5,
            Easing::EaseOutQuad,
        ));
        // Fade out
        ids.push(mgr.start(
            TweenTarget::GlyphAlpha(glyph),
            1.0, 0.0, 0.5,
            Easing::EaseInQuad,
        ));
        // Bright emission
        ids.push(mgr.start(
            TweenTarget::GlyphEmission(glyph),
            2.0, 0.0, 0.5,
            Easing::EaseOutQuad,
        ));
    }

    ids
}

/// Level up screen flash.
pub fn level_up_screen_flash(mgr: &mut TweenManager) -> Vec<TweenId> {
    vec![
        mgr.start(TweenTarget::ScreenBloom, 0.5, 2.0, 0.15, Easing::EaseOutExpo),
        mgr.start_delayed(TweenTarget::ScreenBloom, 2.0, 0.5, 0.4, 0.15, Easing::EaseInQuad),
        mgr.start(TweenTarget::ScreenVignette, 0.0, 0.4, 0.1, Easing::EaseOutQuad),
        mgr.start_delayed(TweenTarget::ScreenVignette, 0.4, 0.0, 0.3, 0.1, Easing::EaseInQuad),
    ]
}

// ═══════════════════════════════════════════════════════════════════════════
// COMBAT
// ═══════════════════════════════════════════════════════════════════════════

/// Entity hit recoil: brief scale squish + position bump.
pub fn entity_hit_recoil(
    mgr: &mut TweenManager,
    entity_glyphs: &[GlyphId],
    hit_direction_x: f32,
) -> Vec<TweenId> {
    let mut ids = Vec::new();
    for &glyph in entity_glyphs {
        // Scale squish
        ids.push(mgr.start(
            TweenTarget::GlyphScaleX(glyph),
            1.0, 0.8, 0.05,
            Easing::EaseOutQuad,
        ));
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphScaleX(glyph),
            0.8, 1.0, 0.1, 0.05,
            Easing::EaseOutBack,
        ));
        ids.push(mgr.start(
            TweenTarget::GlyphScaleY(glyph),
            1.0, 1.2, 0.05,
            Easing::EaseOutQuad,
        ));
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphScaleY(glyph),
            1.2, 1.0, 0.1, 0.05,
            Easing::EaseOutBack,
        ));
        // Emission flash
        ids.push(mgr.start(
            TweenTarget::GlyphEmission(glyph),
            0.0, 1.5, 0.05,
            Easing::Flash,
        ));
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphEmission(glyph),
            1.5, 0.0, 0.15, 0.05,
            Easing::EaseInQuad,
        ));
    }
    // Screen trauma
    ids.push(mgr.start(TweenTarget::CameraTrauma, 0.0, 0.3, 0.1, Easing::Flash));
    ids
}

/// Boss entrance: dramatic camera + screen effects.
pub fn boss_entrance(mgr: &mut TweenManager) -> Vec<TweenId> {
    vec![
        // Vignette crush
        mgr.start(TweenTarget::ScreenVignette, 0.0, 0.8, 0.3, Easing::EaseInQuad),
        mgr.start_delayed(TweenTarget::ScreenVignette, 0.8, 0.2, 0.5, 0.3, Easing::EaseOutQuad),
        // Chromatic aberration spike
        mgr.start(TweenTarget::ScreenChromaticAberration, 0.0, 0.8, 0.2, Easing::EaseOutExpo),
        mgr.start_delayed(TweenTarget::ScreenChromaticAberration, 0.8, 0.0, 0.4, 0.2, Easing::EaseInQuad),
        // Desaturation
        mgr.start(TweenTarget::ScreenSaturation, 1.0, 0.3, 0.3, Easing::EaseInQuad),
        mgr.start_delayed(TweenTarget::ScreenSaturation, 0.3, 1.0, 0.5, 0.3, Easing::EaseOutQuad),
        // Camera trauma
        mgr.start(TweenTarget::CameraTrauma, 0.0, 0.5, 0.3, Easing::EaseOutExpo),
    ]
}

/// Combat hit screen shake.
pub fn combat_hit_shake(
    mgr: &mut TweenManager,
    intensity: f32,
) -> TweenId {
    mgr.start(
        TweenTarget::CameraTrauma,
        0.0, intensity, 0.08,
        Easing::Flash,
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// DEATH
// ═══════════════════════════════════════════════════════════════════════════

/// "YOU DIED" text scales up from 0 with spring easing.
pub fn death_text_reveal(
    mgr: &mut TweenManager,
    text_glyphs: &[GlyphId],
) -> Vec<TweenId> {
    let mut ids = Vec::new();
    for (i, &glyph) in text_glyphs.iter().enumerate() {
        let delay = i as f32 * 0.03; // stagger each character
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphScale(glyph),
            0.0, 1.0, 0.4, delay,
            Easing::Spring { stiffness: 6.0, damping: 0.35 },
        ));
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphAlpha(glyph),
            0.0, 1.0, 0.2, delay,
            Easing::EaseOutQuad,
        ));
    }
    // Screen effects
    ids.push(mgr.start(TweenTarget::ScreenSaturation, 1.0, 0.0, 1.5, Easing::EaseInQuad));
    ids.push(mgr.start(TweenTarget::ScreenVignette, 0.0, 0.9, 1.0, Easing::EaseInCubic));
    ids.push(mgr.start(TweenTarget::ScreenChromaticAberration, 0.0, 0.3, 0.5, Easing::EaseOutQuad));
    ids
}

/// Entity death: glyphs scatter and fade.
pub fn entity_death_scatter(
    mgr: &mut TweenManager,
    entity_glyphs: &[GlyphId],
    entity_x: f32,
    entity_y: f32,
) -> Vec<TweenId> {
    let mut ids = Vec::new();
    let count = entity_glyphs.len();
    for (i, &glyph) in entity_glyphs.iter().enumerate() {
        let angle = i as f32 * TAU / count.max(1) as f32;
        let dist = 3.0 + (i as f32 * 0.5);
        ids.push(mgr.start(
            TweenTarget::GlyphPositionX(glyph),
            entity_x, entity_x + angle.cos() * dist, 0.6,
            Easing::EaseOutQuad,
        ));
        ids.push(mgr.start(
            TweenTarget::GlyphPositionY(glyph),
            entity_y, entity_y + angle.sin() * dist, 0.6,
            Easing::EaseOutQuad,
        ));
        ids.push(mgr.start(
            TweenTarget::GlyphAlpha(glyph),
            1.0, 0.0, 0.6,
            Easing::EaseInQuad,
        ));
        ids.push(mgr.start(
            TweenTarget::GlyphRotation(glyph),
            0.0, TAU * (if i % 2 == 0 { 1.0 } else { -1.0 }), 0.6,
            Easing::EaseOutQuad,
        ));
    }
    ids
}

// ═══════════════════════════════════════════════════════════════════════════
// VICTORY
// ═══════════════════════════════════════════════════════════════════════════

/// Score counter rolls up from 0 to final value.
pub fn score_roll_up(
    mgr: &mut TweenManager,
    final_score: f32,
    duration: f32,
) -> TweenId {
    mgr.start(
        TweenTarget::Named("score_display".to_string()),
        0.0, final_score, duration,
        Easing::EaseOutCubic,
    )
}

/// Victory text entrance: each character drops in with bounce.
pub fn victory_text_drop(
    mgr: &mut TweenManager,
    text_glyphs: &[GlyphId],
    target_y: f32,
) -> Vec<TweenId> {
    let mut ids = Vec::new();
    for (i, &glyph) in text_glyphs.iter().enumerate() {
        let delay = i as f32 * 0.05;
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphPositionY(glyph),
            target_y + 10.0, target_y, 0.4, delay,
            Easing::EaseOutBounce,
        ));
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphAlpha(glyph),
            0.0, 1.0, 0.1, delay,
            Easing::EaseOutQuad,
        ));
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphEmission(glyph),
            2.0, 0.3, 0.3, delay,
            Easing::EaseOutQuad,
        ));
    }
    // Bloom flash
    ids.push(mgr.start(TweenTarget::ScreenBloom, 0.5, 3.0, 0.2, Easing::EaseOutExpo));
    ids.push(mgr.start_delayed(TweenTarget::ScreenBloom, 3.0, 0.5, 0.5, 0.2, Easing::EaseInQuad));
    ids
}

// ═══════════════════════════════════════════════════════════════════════════
// CHARACTER CREATION / ASSEMBLY
// ═══════════════════════════════════════════════════════════════════════════

/// Assemble entity: glyphs fly in from scattered positions to formation targets.
pub fn entity_assemble(
    mgr: &mut TweenManager,
    glyph_starts: &[(GlyphId, f32, f32)], // (glyph, start_x, start_y)
    glyph_targets: &[(f32, f32)],          // (target_x, target_y)
    stagger: f32,
) -> Vec<TweenId> {
    let mut ids = Vec::new();
    for (i, ((glyph, sx, sy), (tx, ty))) in glyph_starts.iter().zip(glyph_targets.iter()).enumerate() {
        let delay = i as f32 * stagger;
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphPositionX(*glyph),
            *sx, *tx, 0.3, delay,
            Easing::EaseOutBack,
        ));
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphPositionY(*glyph),
            *sy, *ty, 0.3, delay,
            Easing::EaseOutBack,
        ));
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphAlpha(*glyph),
            0.0, 1.0, 0.15, delay,
            Easing::EaseOutQuad,
        ));
    }
    ids
}

// ═══════════════════════════════════════════════════════════════════════════
// CRAFTING
// ═══════════════════════════════════════════════════════════════════════════

/// Shatter: glyphs explode outward from a center point.
pub fn crafting_shatter(
    mgr: &mut TweenManager,
    glyphs: &[GlyphId],
    center_x: f32,
    center_y: f32,
) -> Vec<TweenId> {
    entity_death_scatter(mgr, glyphs, center_x, center_y)
}

/// Forge: glyphs converge to center and flash.
pub fn crafting_forge(
    mgr: &mut TweenManager,
    glyph_positions: &[(GlyphId, f32, f32)],
    center_x: f32,
    center_y: f32,
) -> Vec<TweenId> {
    let mut ids = Vec::new();
    for (i, (glyph, sx, sy)) in glyph_positions.iter().enumerate() {
        let delay = i as f32 * 0.02;
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphPositionX(*glyph),
            *sx, center_x, 0.3, delay,
            Easing::EaseInQuad,
        ));
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphPositionY(*glyph),
            *sy, center_y, 0.3, delay,
            Easing::EaseInQuad,
        ));
    }
    // Screen flash at convergence
    ids.push(mgr.start_delayed(TweenTarget::ScreenBloom, 0.5, 3.0, 0.1, 0.3, Easing::Flash));
    ids.push(mgr.start_delayed(TweenTarget::ScreenBloom, 3.0, 0.5, 0.3, 0.4, Easing::EaseInQuad));
    ids.push(mgr.start_delayed(TweenTarget::CameraTrauma, 0.0, 0.2, 0.1, 0.3, Easing::Flash));
    ids
}

// ═══════════════════════════════════════════════════════════════════════════
// FLOOR NAVIGATION
// ═══════════════════════════════════════════════════════════════════════════

/// Slide the room cursor to a new position.
pub fn room_cursor_slide(
    mgr: &mut TweenManager,
    cursor_glyph: GlyphId,
    from_x: f32,
    from_y: f32,
    to_x: f32,
    to_y: f32,
) -> Vec<TweenId> {
    mgr.cancel_glyph(cursor_glyph);
    vec![
        mgr.start(TweenTarget::GlyphPositionX(cursor_glyph), from_x, to_x, 0.15, Easing::EaseOutCubic),
        mgr.start(TweenTarget::GlyphPositionY(cursor_glyph), from_y, to_y, 0.15, Easing::EaseOutCubic),
    ]
}

/// Room transition: brief camera pan.
pub fn room_transition_pan(
    mgr: &mut TweenManager,
    from_x: f32,
    to_x: f32,
    from_y: f32,
    to_y: f32,
) -> Vec<TweenId> {
    mgr.cancel_tag("room_pan");
    vec![
        mgr.start_tagged(TweenTarget::CameraPositionX, from_x, to_x, 0.3, Easing::EaseInOutCubic, "room_pan"),
        mgr.start_tagged(TweenTarget::CameraPositionY, from_y, to_y, 0.3, Easing::EaseInOutCubic, "room_pan"),
    ]
}

// ═══════════════════════════════════════════════════════════════════════════
// UTILITY
// ═══════════════════════════════════════════════════════════════════════════

/// Pulse a glyph's emission in a loop (for highlights, cursors, etc.).
/// Returns the tween ID — cancel to stop pulsing.
pub fn pulse_emission(
    mgr: &mut TweenManager,
    glyph: GlyphId,
    min_emission: f32,
    max_emission: f32,
    period: f32,
) -> TweenId {
    // Use a yoyo tween for continuous pulsing.
    let tween = super::Tween::new(min_emission, max_emission, period * 0.5, Easing::EaseInOutSine)
        .with_repeat(-1, true);
    let state = super::TweenState::new(tween);
    mgr.push_raw(
        TweenTarget::GlyphEmission(glyph),
        state,
        0.0,
        Some("pulse".to_string()),
        None,
    )
}

/// Text typewriter reveal: stagger alpha on each glyph.
pub fn typewriter_reveal(
    mgr: &mut TweenManager,
    glyphs: &[GlyphId],
    chars_per_second: f32,
) -> Vec<TweenId> {
    let mut ids = Vec::new();
    let interval = 1.0 / chars_per_second.max(0.01);
    for (i, &glyph) in glyphs.iter().enumerate() {
        let delay = i as f32 * interval;
        ids.push(mgr.start_delayed(
            TweenTarget::GlyphAlpha(glyph),
            0.0, 1.0, 0.05, delay,
            Easing::Step,
        ));
    }
    ids
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_number_arc_creates_three_tweens() {
        let mut mgr = TweenManager::new();
        let glyph = GlyphId(42);
        let ids = damage_number_arc(&mut mgr, glyph, 0.0, 3.0);
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn hp_bar_damage_creates_two_tweens() {
        let mut mgr = TweenManager::new();
        let bar = BarId(0);
        let (fill, ghost) = hp_bar_damage(&mut mgr, bar, 1.0, 0.7);
        assert!(mgr.is_active(fill));
        assert!(mgr.is_active(ghost));
    }

    #[test]
    fn level_up_ring_creates_four_per_glyph() {
        let mut mgr = TweenManager::new();
        let glyphs = vec![GlyphId(0), GlyphId(1), GlyphId(2)];
        let ids = level_up_ring(&mut mgr, &glyphs, 0.0, 0.0, 5.0);
        assert_eq!(ids.len(), 12); // 4 per glyph × 3 glyphs
    }

    #[test]
    fn screen_crossfade_chains() {
        let mut mgr = TweenManager::new();
        screen_crossfade(&mut mgr, 0.2, |_| {});
        mgr.tick(0.25); // complete first fade
        assert!(mgr.active_count() >= 1, "Should have started fade-in");
    }

    #[test]
    fn score_roll_up_named() {
        let mut mgr = TweenManager::new();
        score_roll_up(&mut mgr, 1000.0, 1.0);
        mgr.tick(0.5);
        let val = mgr.get_named("score_display");
        assert!(val > 0.0 && val < 1000.0);
    }
}
