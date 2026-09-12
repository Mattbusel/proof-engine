//! The matter has to actually move.
//!
//! Proof Engine's premise is that particles are the rendering primitive, not an
//! effect layered on top. That only means anything if the springs binding a
//! figure together really drive glyph positions: an entity should hold its
//! shape while healthy, breathe, take a hit, and come apart when killed.
//!
//! These live in tests/ rather than in the module because the crate's unit-test
//! build does not currently compile (unrelated editor and backend test code).

use proof_engine::entity::formation::Formation;
use proof_engine::entity::AmorphousEntity;
use proof_engine::prelude::*;




fn figure() -> AmorphousEntity {
    let f = Formation::circle(12, 1.0);
    let mut e = AmorphousEntity::new("test", Vec3::ZERO);
    e.formation = f.positions;
    e.formation_chars = f.chars;
    e.pulse_rate = 0.5;
    e.pulse_depth = 0.1;
    e
}

#[test]
fn glyph_physics_produces_one_position_per_formation_slot() {
    let mut e = figure();
    let out = e.step_glyph_physics(1.0 / 60.0);
    assert_eq!(out.len(), e.formation.len());
    for p in out {
        assert!(p.is_finite(), "physics produced a non-finite position");
    }
}

#[test]
fn a_healthy_entity_holds_its_shape() {
    let mut e = figure();
    for _ in 0..240 {
        e.tick(1.0 / 60.0, 0.0);
        e.step_glyph_physics(1.0 / 60.0);
    }
    let out = e.step_glyph_physics(1.0 / 60.0);
    // Every glyph should sit near its slot, within the pulse amplitude.
    for (i, p) in out.iter().enumerate() {
        let slot = e.position + e.formation[i];
        assert!(
            (*p - slot).length() < 0.5,
            "glyph {i} drifted {:.2} from its slot while at full health",
            (*p - slot).length()
        );
    }
}

#[test]
fn damage_loosens_the_binding() {
    // The premise: a hurt entity is not animated as hurt, its cohesion drops.
    let mut healthy = figure();
    healthy.tick(1.0 / 60.0, 0.0);
    let bound = healthy.cohesion;

    let mut hurt = figure();
    hurt.take_damage(80.0);
    hurt.tick(1.0 / 60.0, 0.0);
    assert!(hurt.cohesion < bound, "damage did not weaken cohesion");
}

#[test]
fn a_dissolved_entity_actually_comes_apart() {
    let mut e = figure();
    for _ in 0..120 {
        e.tick(1.0 / 60.0, 0.0);
        e.step_glyph_physics(1.0 / 60.0);
    }
    let before: Vec<Vec3> = e.step_glyph_physics(1.0 / 60.0);
    e.dissolve();
    for _ in 0..90 {
        e.step_glyph_physics(1.0 / 60.0);
    }
    let after = e.step_glyph_physics(1.0 / 60.0);

    let spread = |v: &Vec<Vec3>| -> f32 {
        let c = v.iter().fold(Vec3::ZERO, |a, b| a + *b) / v.len() as f32;
        v.iter().map(|p| (*p - c).length()).sum::<f32>() / v.len() as f32
    };
    assert!(
        spread(&after) > spread(&before) * 1.2,
        "dissolving did not disperse the matter: {:.2} -> {:.2}",
        spread(&before),
        spread(&after)
    );
}

#[test]
fn an_impulse_moves_the_whole_formation() {
    let mut e = figure();
    for _ in 0..60 {
        e.step_glyph_physics(1.0 / 60.0);
    }
    let before = e.step_glyph_physics(1.0 / 60.0);
    e.apply_impulse(Vec3::new(6.0, 0.0, 0.0));
    let after = e.step_glyph_physics(1.0 / 60.0);
    let moved: f32 = before
        .iter()
        .zip(after.iter())
        .map(|(a, b)| b.x - a.x)
        .sum();
    assert!(moved > 0.0, "the impulse did not push the matter");
}

#[test]
fn breathing_changes_the_size_of_the_formation() {
    let mut e = figure();
    e.pulse_rate = 1.0;
    e.pulse_depth = 0.3;
    let mut radii = Vec::new();
    for _ in 0..120 {
        e.tick(1.0 / 60.0, 0.0);
        let p = e.step_glyph_physics(1.0 / 60.0);
        let c = p.iter().fold(Vec3::ZERO, |a, b| a + *b) / p.len() as f32;
        radii.push(p.iter().map(|q| (*q - c).length()).sum::<f32>() / p.len() as f32);
    }
    let min = radii.iter().cloned().fold(f32::MAX, f32::min);
    let max = radii.iter().cloned().fold(f32::MIN, f32::max);
    assert!(max > min * 1.02, "the entity is not breathing: {min:.3}..{max:.3}");
}

#[test]
fn physics_never_produces_nan() {
    // A NaN here would take the whole frame with it.
    let mut e = figure();
    e.entity_entropy = 1.0;
    e.entity_temperature = 1.0;
    for step in 0..600 {
        if step == 200 {
            e.take_damage(60.0);
        }
        if step == 400 {
            e.dissolve();
        }
        e.tick(1.0 / 60.0, 0.0);
        for p in e.step_glyph_physics(1.0 / 60.0) {
            assert!(p.is_finite(), "non-finite position at step {step}");
        }
    }
}

// ── Mass ─────────────────────────────────────────────────────────────────────
//
// "Give each glyph mass" is the part that makes a figure read as matter rather
// than as a pattern: the heavy core holds while the light edges whip around it.

use proof_engine::entity::cohesion::CohesionManager;
use proof_engine::math::springs::SpringDamper;

#[test]
fn a_heavier_spring_takes_longer_to_arrive() {
    let mut light = SpringDamper::new(0.0, 40.0, 8.0);
    let mut heavy = SpringDamper::new(0.0, 40.0, 8.0).with_mass(6.0);
    light.set_target(1.0);
    heavy.set_target(1.0);
    for _ in 0..30 {
        light.tick(1.0 / 60.0);
        heavy.tick(1.0 / 60.0);
    }
    assert!(
        light.position > heavy.position,
        "mass did not slow the spring: light {:.3} vs heavy {:.3}",
        light.position,
        heavy.position
    );
}

#[test]
fn mass_is_never_zero_or_nan() {
    // A zero mass would divide the whole frame into infinity.
    for bad in [0.0, -3.0, f32::NAN, f32::INFINITY] {
        let mut s = SpringDamper::new(0.0, 20.0, 4.0);
        s.set_mass(bad);
        s.set_target(1.0);
        assert!(s.mass > 0.0, "mass {bad} was accepted");
        for _ in 0..60 {
            s.tick(1.0 / 60.0);
        }
        assert!(s.position.is_finite(), "mass {bad} produced a non-finite position");
    }
}

#[test]
fn the_same_blow_moves_light_matter_further() {
    let mut cs = CohesionManager::new(&[Vec3::ZERO, Vec3::ZERO], 1.0);
    cs.set_masses(&[1.0, 8.0]);
    for g in &mut cs.glyphs {
        g.apply_impulse(Vec3::new(5.0, 0.0, 0.0));
    }
    let out = cs.tick(1.0 / 60.0);
    assert!(
        out[0].x > out[1].x,
        "the heavy glyph moved as far as the light one: {:.4} vs {:.4}",
        out[0].x,
        out[1].x
    );
}

#[test]
fn total_mass_counts_every_glyph() {
    let mut cs = CohesionManager::new(&[Vec3::ZERO; 4], 1.0);
    cs.set_masses(&[1.0, 2.0, 3.0, 4.0]);
    assert!((cs.total_mass() - 10.0).abs() < 0.001);
}

#[test]
fn a_heavy_core_survives_the_blast_that_scatters_the_edges() {
    let ring = Formation::circle(16, 1.0);
    let mut e = AmorphousEntity::new("cored", Vec3::ZERO);
    e.formation = ring.positions;
    e.formation_chars = ring.chars;
    e.pulse_depth = 0.0;
    e.step_glyph_physics(1.0 / 60.0);

    // Slot 0 is the anchor: an order of magnitude heavier than the rest.
    let masses: Vec<f32> = (0..e.formation.len())
        .map(|i| if i == 0 { 20.0 } else { 1.0 })
        .collect();
    e.cohesion_system.as_mut().unwrap().set_masses(&masses);

    let origin: Vec<Vec3> = e.step_glyph_physics(0.0);
    e.dissolve();
    for _ in 0..60 {
        e.step_glyph_physics(1.0 / 60.0);
    }
    let after = e.step_glyph_physics(1.0 / 60.0);

    let moved = |i: usize| (after[i] - origin[i]).length();
    let light: f32 = (1..after.len()).map(moved).sum::<f32>() / (after.len() - 1) as f32;
    assert!(
        moved(0) < light,
        "the heavy core flew as far as the light matter: {:.3} vs {:.3}",
        moved(0),
        light
    );
}

#[test]
fn dissolving_matter_reports_how_far_it_has_come_from_its_slot() {
    // `drift` is what every renderer reads to decide whether a particle is
    // still held. If dissolution stops updating it, a cloud flying apart still
    // claims to be bound, and it keeps its light and never tumbles.
    let ring = Formation::circle(12, 1.0);
    let mut e = AmorphousEntity::new("scatter", Vec3::ZERO);
    e.formation = ring.positions;
    e.formation_chars = ring.chars;
    e.pulse_depth = 0.0;
    for _ in 0..60 {
        e.step_glyph_physics(1.0 / 60.0);
    }
    let bound: f32 = e
        .cohesion_system
        .as_ref()
        .unwrap()
        .glyphs
        .iter()
        .map(|g| g.drift)
        .sum();

    e.dissolve();
    for _ in 0..60 {
        e.step_glyph_physics(1.0 / 60.0);
    }
    let loose: f32 = e
        .cohesion_system
        .as_ref()
        .unwrap()
        .glyphs
        .iter()
        .map(|g| g.drift)
        .sum();
    assert!(
        loose > bound + 1.0,
        "drift did not follow the matter apart: {bound:.3} -> {loose:.3}"
    );
}

// ── Coming apart and back together ───────────────────────────────────────────

#[test]
fn dissolved_matter_can_be_pulled_back_together() {
    let ring = Formation::circle(16, 1.0);
    let mut e = AmorphousEntity::new("phoenix", Vec3::ZERO);
    e.formation = ring.positions.clone();
    e.formation_chars = ring.chars;
    e.pulse_depth = 0.0;
    for _ in 0..60 {
        e.step_glyph_physics(1.0 / 60.0);
    }

    let spread = |v: &Vec<Vec3>| -> f32 {
        let c = v.iter().fold(Vec3::ZERO, |a, b| a + *b) / v.len() as f32;
        v.iter().map(|p| (*p - c).length()).sum::<f32>() / v.len() as f32
    };
    let whole = spread(&e.step_glyph_physics(0.0));

    e.dissolve();
    for _ in 0..60 {
        e.step_glyph_physics(1.0 / 60.0);
    }
    let scattered = spread(&e.step_glyph_physics(0.0));
    assert!(scattered > whole * 1.3, "it never came apart");

    e.reform(1.0);
    for _ in 0..240 {
        e.tick(1.0 / 60.0, 0.0);
        e.step_glyph_physics(1.0 / 60.0);
    }
    let back = e.step_glyph_physics(0.0);
    for (i, p) in back.iter().enumerate() {
        let slot = ring.positions[i % ring.positions.len()];
        assert!(
            (*p - slot).length() < 0.25,
            "glyph {i} never came home: {:.3} away",
            (*p - slot).length()
        );
    }
}

#[test]
fn matter_can_be_rebuilt_into_a_different_shape() {
    // The intro takes two figures apart and reassembles them as the logo.
    let start = Formation::circle(12, 1.0);
    let mut e = AmorphousEntity::new("morph", Vec3::ZERO);
    e.formation = start.positions;
    e.formation_chars = start.chars;
    e.pulse_depth = 0.0;
    for _ in 0..120 {
        e.step_glyph_physics(1.0 / 60.0);
    }

    // A line, which nothing about the circle resembles.
    let line: Vec<Vec3> = (0..7).map(|i| Vec3::new(i as f32 * 0.4 - 1.2, 0.0, 0.0)).collect();
    e.dissolve();
    for _ in 0..30 {
        e.step_glyph_physics(1.0 / 60.0);
    }
    e.reshape(&line);
    e.reform(1.0);
    for _ in 0..300 {
        e.tick(1.0 / 60.0, 0.0);
        e.step_glyph_physics(1.0 / 60.0);
    }

    let out = e.step_glyph_physics(0.0);
    assert_eq!(out.len(), e.formation.len(), "matter was stranded");
    for (i, p) in out.iter().enumerate() {
        let want = line[i % line.len()];
        assert!(
            (*p - want).length() < 0.25,
            "glyph {i} did not reach the new shape"
        );
    }
    // And it really is a line now: nothing should be far off the y axis.
    assert!(out.iter().all(|p| p.y.abs() < 0.3), "it kept the old shape");
}

#[test]
fn reshaping_never_strands_matter_when_the_new_shape_is_smaller() {
    let mut e = AmorphousEntity::new("shrink", Vec3::ZERO);
    e.formation = (0..20).map(|i| Vec3::new(i as f32 * 0.1, 0.0, 0.0)).collect();
    e.formation_chars = vec!['#'; 20];
    e.step_glyph_physics(1.0 / 60.0);
    e.reshape(&[Vec3::ZERO, Vec3::X]);
    assert_eq!(e.formation.len(), 20, "slots were dropped with the matter on them");
    let out = e.step_glyph_physics(1.0 / 60.0);
    assert_eq!(out.len(), 20);
}

#[test]
fn a_stronger_bind_makes_matter_arrive_sooner() {
    // The default cohesion curve is underdamped, which is right for a body that
    // has just been hit and wrong for a shape that is supposed to snap.
    let build = |k: f32, d: f32| -> f32 {
        let ring = Formation::circle(10, 1.0);
        let mut e = AmorphousEntity::new("bind", Vec3::ZERO);
        e.formation = ring.positions.clone();
        e.formation_chars = ring.chars;
        e.pulse_depth = 0.0;
        e.rebuild_cohesion();
        e.set_bind(k, d);
        // Start every glyph well away from its slot.
        e.apply_impulse(Vec3::new(6.0, 0.0, 0.0));
        // Sampled a quarter of a second in, while both are still travelling:
        // given long enough every bind arrives, and the point is the timing.
        for _ in 0..15 {
            e.step_glyph_physics(1.0 / 60.0);
        }
        let out = e.step_glyph_physics(0.0);
        out.iter()
            .enumerate()
            .map(|(i, p)| (*p - ring.positions[i]).length())
            .sum::<f32>()
            / out.len() as f32
    };
    let loose = build(1.0, 1.0);
    let tight = build(6.0, 3.0);
    assert!(
        tight < loose * 0.8,
        "a stronger bind did not settle sooner: {loose:.3} vs {tight:.3}"
    );
}

#[test]
fn stiff_matter_holds_its_shape_and_soft_matter_does_not() {
    // The point of per-particle springs: a steel blade and a wool cloak hanging
    // off the same body must not deform by the same amount when it moves. This
    // is what stops a sword bending like an arm.
    use proof_engine::entity::AmorphousEntity;
    use proof_engine::prelude::Vec3;

    let build = |stiff: f32, damp: f32| {
        let mut e = AmorphousEntity::new("bar", Vec3::ZERO);
        e.formation = (0..40)
            .map(|i| Vec3::new(i as f32 * 0.02, 0.0, 0.0))
            .collect();
        e.rebuild_cohesion();
        e.set_bind_each(&vec![(stiff, damp); 40]);
        e
    };

    // Steel: very stiff, moderately damped. It returns to shape fast and
    // barely leaves it. Cloth: slack and lightly damped, so it strays a long
    // way and takes its time coming back. Heavy damping on cloth was the wrong
    // model — it made the cloak behave *stiffer* than the sword.
    let mut steel = build(3.4, 1.5);
    let mut cloth = build(0.30, 0.9);
    // The same blow to both.
    steel.apply_impulse(Vec3::new(0.0, -6.0, 0.0));
    cloth.apply_impulse(Vec3::new(0.0, -6.0, 0.0));

    let mut steel_worst = 0.0f32;
    let mut cloth_worst = 0.0f32;
    for _ in 0..30 {
        let a = steel.step_glyph_physics(1.0 / 60.0);
        let b = cloth.step_glyph_physics(1.0 / 60.0);
        for (p, slot) in a.iter().zip(steel.formation.iter()) {
            steel_worst = steel_worst.max((*p - *slot).length());
        }
        for (p, slot) in b.iter().zip(cloth.formation.iter()) {
            cloth_worst = cloth_worst.max((*p - *slot).length());
        }
    }
    assert!(
        cloth_worst > steel_worst * 1.5,
        "cloth deformed {cloth_worst:.4} against steel's {steel_worst:.4}; \
         the two materials are behaving the same"
    );
    assert!(steel_worst.is_finite() && cloth_worst.is_finite());
}

#[test]
fn matter_cannot_go_through_the_floor() {
    // A figure that hovers at whatever height something else decided is not
    // standing on anything. With a floor under it, matter driven into the
    // ground stops at the ground.
    use proof_engine::entity::AmorphousEntity;
    use proof_engine::prelude::Vec3;

    let mut e = AmorphousEntity::new("body", Vec3::ZERO);
    e.formation = (0..30)
        .map(|i| Vec3::new(i as f32 * 0.02 - 0.3, 0.0, 0.0))
        .collect();
    e.rebuild_cohesion();
    // y grows downward, so the floor is just below the formation.
    e.set_floor(Some(0.05));
    // Thrown hard at the ground.
    e.apply_impulse(Vec3::new(0.0, 9.0, 0.0));

    let mut lowest = f32::MIN;
    for _ in 0..90 {
        for p in e.step_glyph_physics(1.0 / 60.0) {
            lowest = lowest.max(p.y);
            assert!(p.y <= 0.05 + 1e-3, "matter went through the floor to {}", p.y);
        }
    }
    assert!(lowest > 0.0, "nothing ever reached the floor: {lowest}");
}

#[test]
fn a_death_settles_on_the_floor_rather_than_falling_through_it() {
    // Dispersing matter takes a different path through the physics, and it was
    // the one that mattered most here: a body coming apart used to rain
    // through the ground it was standing on.
    use proof_engine::entity::AmorphousEntity;
    use proof_engine::prelude::Vec3;

    let mut e = AmorphousEntity::new("body", Vec3::ZERO);
    e.formation = (0..40)
        .map(|i| Vec3::new((i % 8) as f32 * 0.03, (i / 8) as f32 * 0.03 - 0.1, 0.0))
        .collect();
    e.rebuild_cohesion();
    e.set_floor(Some(0.2));
    e.dissolve();

    for _ in 0..200 {
        for p in e.step_glyph_physics(1.0 / 60.0) {
            assert!(p.y <= 0.2 + 1e-3, "a dispersing body fell through the floor");
            assert!(p.is_finite());
        }
    }
}

#[test]
fn a_floor_can_be_taken_away_again() {
    use proof_engine::entity::AmorphousEntity;
    use proof_engine::prelude::Vec3;

    let mut e = AmorphousEntity::new("body", Vec3::ZERO);
    e.formation = vec![Vec3::ZERO];
    e.rebuild_cohesion();
    e.set_floor(Some(0.0));
    e.apply_impulse(Vec3::new(0.0, 6.0, 0.0));
    for _ in 0..20 {
        e.step_glyph_physics(1.0 / 60.0);
    }
    e.set_floor(None);
    e.dissolve();
    e.apply_impulse(Vec3::new(0.0, 6.0, 0.0));
    let mut went_below = false;
    for _ in 0..60 {
        for p in e.step_glyph_physics(1.0 / 60.0) {
            if p.y > 0.05 {
                went_below = true;
            }
        }
    }
    assert!(went_below, "the floor was removed and still held the matter up");
}
