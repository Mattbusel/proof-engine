//! Screen effects: shockwaves, flashes and light shafts decay on their own
//! and pack into the shader's uniform layout.

use proof_engine::render::screen_fx::{ScreenFx, MAX_SHOCKWAVES};
use glam::Vec3;

mod tests {
    use super::*;

    #[test]
    fn a_shockwave_grows_fades_and_dies() {
        let mut fx = ScreenFx::new();
        fx.shockwave(100.0, 100.0, 10.0);
        assert_eq!(fx.shockwaves.len(), 1);
        let s0 = fx.shockwaves[0].current_strength();
        fx.tick(0.2);
        let w = fx.shockwaves[0];
        assert!(w.radius() > 200.0, "the ring did not travel: {}", w.radius());
        assert!(w.current_strength() < s0, "the ring did not fade");
        fx.tick(1.0);
        assert!(fx.shockwaves.is_empty(), "the ring outlived its duration");
    }

    #[test]
    fn the_cap_drops_the_oldest() {
        let mut fx = ScreenFx::new();
        for i in 0..(MAX_SHOCKWAVES + 3) {
            fx.shockwave(i as f32, 0.0, 1.0);
        }
        assert_eq!(fx.shockwaves.len(), MAX_SHOCKWAVES);
        assert_eq!(fx.shockwaves[0].origin.x, 3.0);
    }

    #[test]
    fn a_flash_decays_to_nothing() {
        let mut fx = ScreenFx::new();
        fx.flash(Vec3::new(1.0, 0.5, 0.2), 0.8);
        assert!(fx.flash.x > 0.79);
        fx.tick(0.1);
        assert!(fx.flash.x < 0.5, "a flash should be mostly gone in a tenth of a second");
        fx.tick(1.0);
        assert_eq!(fx.flash, Vec3::ZERO);
        assert!(fx.is_idle());
    }

    #[test]
    fn flashes_do_not_stack_past_the_brightest() {
        let mut fx = ScreenFx::new();
        fx.flash(Vec3::ONE, 0.6);
        fx.flash(Vec3::ONE, 0.6);
        assert!((fx.flash.x - 0.6).abs() < 1e-6);
    }

    #[test]
    fn shafts_ease_in_and_release_their_origin_when_gone() {
        let mut fx = ScreenFx::new();
        fx.light_shaft_at(640.0, 200.0, 1.0);
        fx.tick(0.1);
        let a = fx.shaft_strength;
        assert!(a > 0.0 && a < 1.0, "should be easing in: {a}");
        let p = fx.pack_shaft(1280.0, 800.0);
        assert!((p[0] - 0.5).abs() < 1e-6);
        assert!((p[1] - 0.75).abs() < 1e-6, "y must flip to texture space: {}", p[1]);
        fx.clear_light_shaft();
        for _ in 0..100 {
            fx.tick(0.1);
        }
        assert!(fx.shaft_origin.is_none());
        assert_eq!(fx.pack_shaft(1280.0, 800.0)[2], 0.0);
    }

    #[test]
    fn packing_skips_spent_waves_and_flips_y() {
        let mut fx = ScreenFx::new();
        fx.shockwave(320.0, 200.0, 8.0);
        let (data, strength, n) = fx.pack_shockwaves(1280.0, 800.0);
        assert_eq!(n, 1);
        assert!((data[0][0] - 0.25).abs() < 1e-6);
        assert!((data[0][1] - 0.75).abs() < 1e-6);
        assert!(strength[0] > 7.9);
    }
}
