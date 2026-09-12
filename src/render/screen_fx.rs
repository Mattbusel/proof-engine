//! Transient full-screen effects a game fires at moments: shockwaves, flashes,
//! and a light-shaft source.
//!
//! [`crate::config::RenderConfig`] is the standing look of the picture, the
//! grade and the bloom and the lens. This is what happens *to* the picture
//! when something happens in the game. A blow lands and a ring of refraction
//! runs out from the point of impact. A spell goes off and the frame flashes
//! its colour and falls back. A brazier or a boss's eye becomes the source
//! that light shafts stream from.
//!
//! All of it is stateful and decays on its own, so a game fires and forgets.
//! Coordinates are the same screen pixels the UI layer uses: `(0, 0)` at the
//! top left, `y` down.

use glam::{Vec2, Vec3};

/// Hard cap on simultaneous shockwaves; the composite shader has a fixed
/// uniform array of this size.
pub const MAX_SHOCKWAVES: usize = 12;

/// One expanding ring of refraction.
#[derive(Clone, Copy, Debug)]
pub struct Shockwave {
    /// Origin, in screen pixels.
    pub origin: Vec2,
    /// Seconds since it was fired.
    pub age: f32,
    /// Seconds it lives.
    pub duration: f32,
    /// How fast the ring travels, in pixels per second.
    pub speed: f32,
    /// Thickness of the ring, in pixels.
    pub width: f32,
    /// Peak displacement at the ring, in pixels.
    pub strength: f32,
}

impl Shockwave {
    /// Radius right now, in pixels.
    pub fn radius(&self) -> f32 {
        self.age * self.speed
    }

    /// Displacement right now: the peak, eased out over the lifetime.
    pub fn current_strength(&self) -> f32 {
        let t = (self.age / self.duration.max(1e-3)).clamp(0.0, 1.0);
        let fade = 1.0 - t;
        self.strength * fade * fade
    }

    pub fn is_dead(&self) -> bool {
        self.age >= self.duration
    }
}

/// A glossy floor: the picture mirrored about a horizontal line and laid
/// back over itself below it, fading with distance from the line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reflection {
    /// The line the picture reflects about, in screen pixels from the top.
    pub y: f32,
    /// How much of the mirrored picture shows right at the line. 0.5 is a
    /// wet floor; 0.15 is polished stone.
    pub strength: f32,
    /// How far below the line the reflection is still visible, in pixels.
    pub fade: f32,
    /// Horizontal ripple in the reflection, in pixels. 0.0 is a still mirror.
    pub ripple: f32,
    /// How blurred the reflection is, in pixels. Rough floors blur more.
    pub blur: f32,
}

/// Transient screen effects. Lives on the engine as `engine.fx`.
#[derive(Clone, Debug)]
pub struct ScreenFx {
    pub shockwaves: Vec<Shockwave>,
    /// A floor reflection, if the scene has a floor worth reflecting in.
    /// Set every frame it should show; cleared with [`clear_reflection`](Self::clear_reflection).
    pub reflection: Option<Reflection>,
    /// Heat shimmer over the whole frame, 0.0 to about 1.0. Stronger toward
    /// the bottom of the frame, where hot air rises from.
    pub haze: f32,
    /// Let the renderer pick the light-shaft source itself, from whatever is
    /// brightest in the bloom, whenever no explicit source is set. Costs one
    /// tiny read-back per frame and is a frame late, which no one can see.
    pub auto_shafts: bool,
    /// Colour added to the whole frame before the tonemap, already scaled by
    /// its strength. Decays toward black.
    pub flash: Vec3,
    /// Fraction of the flash that survives each second.
    pub flash_decay: f32,
    /// Where light shafts stream from, in screen pixels, if anywhere.
    pub shaft_origin: Option<Vec2>,
    /// How strong the shafts are right now. Eases toward `shaft_target`.
    pub shaft_strength: f32,
    /// Where the shaft strength is heading.
    pub shaft_target: f32,
    /// How quickly the shafts ease toward the target, per second.
    pub shaft_ease: f32,
    /// A colour cast for the shafts. White lets the light's own colour through.
    pub shaft_tint: Vec3,
    /// Seconds accumulated, for anything that wants a clock.
    pub time: f32,
}

impl Default for ScreenFx {
    fn default() -> Self {
        Self {
            shockwaves: Vec::with_capacity(MAX_SHOCKWAVES),
            reflection: None,
            haze: 0.0,
            auto_shafts: false,
            flash: Vec3::ZERO,
            flash_decay: 0.0005,
            shaft_origin: None,
            shaft_strength: 0.0,
            shaft_target: 0.0,
            shaft_ease: 3.0,
            shaft_tint: Vec3::ONE,
            time: 0.0,
        }
    }
}

impl ScreenFx {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fire a shockwave with sensible defaults for a hit: a ring that crosses
    /// a 1280-wide frame in about half a second.
    ///
    /// `strength` is the peak displacement in pixels; 6 to 10 reads as a
    /// blow, 20 or more as an explosion.
    pub fn shockwave(&mut self, x: f32, y: f32, strength: f32) {
        self.shockwave_with(Shockwave {
            origin: Vec2::new(x, y),
            age: 0.0,
            duration: 0.55,
            speed: 1400.0,
            width: 48.0,
            strength,
        });
    }

    /// Fire a fully specified shockwave. If the cap is reached the oldest
    /// one is dropped, so the newest blow always shows.
    pub fn shockwave_with(&mut self, wave: Shockwave) {
        if self.shockwaves.len() >= MAX_SHOCKWAVES {
            self.shockwaves.remove(0);
        }
        self.shockwaves.push(wave);
    }

    /// Flash the frame. `strength` of 1.0 is a full-colour hit before the
    /// tonemap rolls it off; 0.2 is a noticeable pulse.
    pub fn flash(&mut self, color: Vec3, strength: f32) {
        let add = color * strength;
        // The stronger of the two rather than the sum, so a burst of hits
        // does not stack to a white screen.
        self.flash = Vec3::new(
            self.flash.x.max(add.x),
            self.flash.y.max(add.y),
            self.flash.z.max(add.z),
        );
    }

    /// Stream light shafts from a screen point at the given strength. Call
    /// every frame the source is on screen; the strength eases in and out.
    pub fn light_shaft_at(&mut self, x: f32, y: f32, strength: f32) {
        self.shaft_origin = Some(Vec2::new(x, y));
        self.shaft_target = strength.max(0.0);
    }

    /// Let the shafts fade out. The origin is kept until they are gone so the
    /// fade happens in place.
    pub fn clear_light_shaft(&mut self) {
        self.shaft_target = 0.0;
    }

    /// Reflect the picture about the line `y` pixels from the top, for
    /// `fade` pixels below it, at the given strength. A still, lightly
    /// blurred mirror; set the fields on [`Reflection`] for a wet one.
    pub fn reflect_at(&mut self, y: f32, strength: f32, fade: f32) {
        self.reflection = Some(Reflection {
            y,
            strength: strength.max(0.0),
            fade: fade.max(1.0),
            ripple: 0.0,
            blur: 1.5,
        });
    }

    pub fn clear_reflection(&mut self) {
        self.reflection = None;
    }

    /// The reflection for the shader: `(line_v, strength, fade_v, ripple_px)`
    /// and the blur, in texture space with `v = 0` at the bottom.
    pub fn pack_reflection(&self, screen_w: f32, screen_h: f32) -> ([f32; 4], f32) {
        let _ = screen_w;
        match self.reflection {
            Some(r) if r.strength > 0.0 => {
                let h = screen_h.max(1.0);
                ([1.0 - r.y / h, r.strength, r.fade / h, r.ripple], r.blur)
            }
            _ => ([0.0, 0.0, 1.0, 0.0], 0.0),
        }
    }

    /// Advance every effect by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
        for w in &mut self.shockwaves {
            w.age += dt;
        }
        self.shockwaves.retain(|w| !w.is_dead());

        // Exponential decay: the fraction that survives one second is
        // `flash_decay`, so a flash is essentially gone in a third of one.
        let keep = self.flash_decay.max(1e-6).powf(dt);
        self.flash *= keep;
        if self.flash.max_element() < 0.002 {
            self.flash = Vec3::ZERO;
        }

        let ease = (self.shaft_ease * dt).clamp(0.0, 1.0);
        self.shaft_strength += (self.shaft_target - self.shaft_strength) * ease;
        if self.shaft_target <= 0.0 && self.shaft_strength < 0.005 {
            self.shaft_strength = 0.0;
            self.shaft_origin = None;
        }
    }

    /// Whether anything at all is active, so the renderer can skip uploads.
    pub fn is_idle(&self) -> bool {
        self.shockwaves.is_empty()
            && self.flash == Vec3::ZERO
            && self.shaft_strength <= 0.0
            && self.reflection.is_none()
            && self.haze <= 0.0
    }

    /// Pack the shockwaves for the composite shader.
    ///
    /// Returns `(data, strengths, count)`. Each `data` entry is
    /// `(u, v, radius_px, width_px)` with `u, v` in texture space, which has
    /// `v = 0` at the bottom, so `y` is flipped here from the UI's `y`-down.
    pub fn pack_shockwaves(&self, screen_w: f32, screen_h: f32)
        -> ([[f32; 4]; MAX_SHOCKWAVES], [f32; MAX_SHOCKWAVES], usize)
    {
        let mut data = [[0.0f32; 4]; MAX_SHOCKWAVES];
        let mut strength = [0.0f32; MAX_SHOCKWAVES];
        let w = screen_w.max(1.0);
        let h = screen_h.max(1.0);
        let mut n = 0;
        for wave in self.shockwaves.iter().take(MAX_SHOCKWAVES) {
            let s = wave.current_strength();
            if s <= 0.01 {
                continue;
            }
            data[n] = [
                wave.origin.x / w,
                1.0 - wave.origin.y / h,
                wave.radius(),
                wave.width.max(1.0),
            ];
            strength[n] = s;
            n += 1;
        }
        (data, strength, n)
    }

    /// The light-shaft source for the shader: `(u, v, strength)`, or zero
    /// strength when there is none.
    pub fn pack_shaft(&self, screen_w: f32, screen_h: f32) -> [f32; 3] {
        match self.shaft_origin {
            Some(o) if self.shaft_strength > 0.0 => [
                o.x / screen_w.max(1.0),
                1.0 - o.y / screen_h.max(1.0),
                self.shaft_strength,
            ],
            _ => [0.5, 0.5, 0.0],
        }
    }
}
