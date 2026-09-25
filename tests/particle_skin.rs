//! Skinning a particle cloud to a skeleton.
//!
//! Lives here rather than beside the module because the crate's `--lib` test
//! build does not currently compile: several modules under `editor/` and
//! `glyph/` reference private or renamed items. An integration test exercises
//! the same public surface a caller uses and actually runs.

use proof_engine::anim::particle_skin::{Bind, ParticleSkin, MAX_INFLUENCES};
use proof_engine::anim::skeleton::{BoneId, Pose, Skeleton, SkeletonBuilder, Transform3D};
use glam::{Quat, Vec3};

/// A two-bone arm along +X: shoulder at the origin, elbow at 1, tip at 2.
fn arm() -> Skeleton {
    SkeletonBuilder::new()
        .add_bone("upper", None, Transform3D::identity())
        .add_bone(
            "fore",
            Some("upper"),
            Transform3D::new(Vec3::X, Quat::IDENTITY, Vec3::ONE),
        )
        .add_bone(
            "hand",
            Some("fore"),
            Transform3D::new(Vec3::X, Quat::IDENTITY, Vec3::ONE),
        )
        .build()
}

#[test]
fn a_rest_pose_moves_nothing() {
    // The most important property in the whole module: binding a cloud and
    // posing it at rest has to be the identity, or every figure in the game
    // shifts the moment it is skinned.
    let s = arm();
    let pts: Vec<Vec3> = (0..40)
        .map(|i| Vec3::new(i as f32 * 0.05, (i % 5) as f32 * 0.02, 0.0))
        .collect();
    let skin = ParticleSkin::bind_nearest(&pts, &s, 0.3);
    let mut out = Vec::new();
    skin.apply(&s, &s.rest_pose(), &pts, &mut out);
    assert_eq!(out.len(), pts.len());
    for (a, b) in pts.iter().zip(out.iter()) {
        assert!(
            (*a - *b).length() < 1e-4,
            "the rest pose moved a particle: {a} -> {b}"
        );
    }
}

#[test]
fn bending_a_joint_carries_the_matter_past_it() {
    // Rotating the forearm has to move everything beyond the elbow and
    // leave the upper arm where it was. This is the entire point.
    let s = arm();
    let upper = Vec3::new(0.5, 0.0, 0.0);
    let tip = Vec3::new(1.9, 0.0, 0.0);
    let pts = vec![upper, tip];
    let skin = ParticleSkin::bind_nearest(&pts, &s, 0.15);

    let mut pose = s.rest_pose();
    let mut fore = pose.get(BoneId(1)).unwrap();
    fore.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    pose.set(BoneId(1), fore);

    let mut out = Vec::new();
    skin.apply(&s, &pose, &pts, &mut out);
    assert!(
        (out[0] - upper).length() < 0.05,
        "the upper arm moved when the elbow bent: {}",
        out[0]
    );
    assert!(
        (out[1] - tip).length() > 0.5,
        "the elbow bent and the hand stayed put: {}",
        out[1]
    );
    // A quarter turn about z takes +X to +Y, so the tip should be up.
    assert!(out[1].y > 0.5, "the hand bent the wrong way: {}", out[1]);
}

#[test]
fn a_particle_in_the_joint_belongs_to_both_bones() {
    // Without shared weights a joint shears: the two halves of a limb slide
    // past each other and the figure comes apart at every elbow.
    let s = arm();
    let elbow = vec![Vec3::new(1.0, 0.0, 0.0)];
    let skin = ParticleSkin::bind_nearest(&elbow, &s, 0.5);
    let b = skin.bind_of(0).expect("bound");
    assert!(
        b.weights[1] > 0.15,
        "a particle sitting in the joint is rigid to one bone: {:?}",
        b.weights
    );
    assert!(
        (b.weights[0] + b.weights[1] - 1.0).abs() < 1e-4,
        "weights do not sum to one: {:?}",
        b.weights
    );
}

#[test]
fn a_falloff_of_nothing_binds_rigidly() {
    let s = arm();
    let pts = vec![Vec3::new(1.0, 0.0, 0.0)];
    let skin = ParticleSkin::bind_nearest(&pts, &s, 0.0);
    assert_eq!(skin.bind_of(0).unwrap().weights, [1.0, 0.0]);
}

#[test]
fn every_particle_ends_up_bound_to_something() {
    // A particle nowhere near any bone still has to move with the figure,
    // or bits of a sculpt are left hanging in space when it walks off.
    let s = arm();
    let stray = vec![Vec3::new(0.5, 9.0, -4.0)];
    let skin = ParticleSkin::bind_nearest(&stray, &s, 0.3);
    let b = skin.bind_of(0).expect("bound");
    assert!(b.weights[0] > 0.0);
    assert!((b.bones[0] as usize) < s.len());
}

#[test]
fn an_empty_skeleton_is_survivable() {
    let s = Skeleton::new();
    let pts = vec![Vec3::X, Vec3::Y];
    let skin = ParticleSkin::bind_nearest(&pts, &s, 0.2);
    let mut out = Vec::new();
    skin.apply(&s, &Pose::new(0), &pts, &mut out);
    assert_eq!(out, pts, "an empty skeleton should leave the cloud alone");
}

#[test]
fn a_cloud_that_grew_since_it_was_bound_does_not_panic() {
    // Figures here gain matter: a reshape can hand the same body more
    // particles than it was bound with.
    let s = arm();
    let pts = vec![Vec3::new(0.5, 0.0, 0.0)];
    let skin = ParticleSkin::bind_nearest(&pts, &s, 0.2);
    let grown = vec![Vec3::new(0.5, 0.0, 0.0), Vec3::new(1.5, 0.0, 0.0)];
    let mut out = Vec::new();
    skin.apply(&s, &s.rest_pose(), &grown, &mut out);
    assert_eq!(out.len(), 2);
    assert!(out[1].is_finite());
}

#[test]
fn skinning_never_produces_a_broken_number() {
    let s = arm();
    let pts: Vec<Vec3> = (0..64)
        .map(|i| Vec3::new(i as f32 * 0.03, (i as f32 * 0.7).sin() * 0.2, 0.0))
        .collect();
    let skin = ParticleSkin::bind_nearest(&pts, &s, 0.25);
    let mut pose = s.rest_pose();
    for (i, id) in [BoneId(0), BoneId(1), BoneId(2)].iter().enumerate() {
        let mut t = pose.get(*id).unwrap();
        t.rotation = Quat::from_rotation_z(i as f32 * 1.7 - 2.0);
        pose.set(*id, t);
    }
    let mut out = Vec::new();
    skin.apply(&s, &pose, &pts, &mut out);
    for p in &out {
        assert!(p.is_finite(), "skinning produced {p}");
    }
}

#[test]
fn normals_turn_with_the_bone_they_are_bound_to() {
    // Lighting is per particle against a real normal. A normal that stays put
    // while its limb rotates makes the limb read as going flat exactly when it
    // moves, which is the one moment it needs to read as solid.
    let s = arm();
    let pts = vec![Vec3::new(1.6, 0.0, 0.0)];
    let normals = vec![Vec3::Y];
    let skin = ParticleSkin::bind_nearest(&pts, &s, 0.0);

    let mut pose = s.rest_pose();
    let mut fore = pose.get(BoneId(1)).unwrap();
    fore.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    pose.set(BoneId(1), fore);

    let mut out = Vec::new();
    skin.apply_normals(&s, &pose, &normals, &mut out);
    assert_eq!(out.len(), 1);
    assert!((out[0].length() - 1.0).abs() < 1e-4, "a normal lost its length");
    // A quarter turn about z takes +Y to -X.
    assert!(out[0].x < -0.9, "the normal did not follow the bone: {}", out[0]);
}

#[test]
fn a_rest_pose_leaves_normals_alone() {
    let s = arm();
    let pts = vec![Vec3::new(0.5, 0.0, 0.0), Vec3::new(1.5, 0.0, 0.0)];
    let normals = vec![Vec3::Y, Vec3::Z];
    let skin = ParticleSkin::bind_nearest(&pts, &s, 0.2);
    let mut out = Vec::new();
    skin.apply_normals(&s, &s.rest_pose(), &normals, &mut out);
    for (a, b) in normals.iter().zip(out.iter()) {
        assert!((*a - *b).length() < 1e-4, "the rest pose turned a normal");
    }
}

#[test]
fn a_group_keeps_a_point_off_the_bones_it_must_not_use() {
    // The case this exists for: a two-handed sword held up beside the head is
    // nearest to the skull, so binding by distance alone leaves the blade
    // behind when the arm swings and drags it around when the head turns.
    let s = arm();
    // A point right on the "hand" bone but much nearer the upper arm segment.
    let blade = vec![Vec3::new(0.4, 0.05, 0.0)];

    let loose = ParticleSkin::bind_nearest(&blade, &s, 0.0);
    assert_eq!(
        loose.bind_of(0).unwrap().dominant(),
        BoneId(0),
        "without a group it binds to whatever is nearest"
    );

    // Told it may only use the hand, it uses the hand.
    let held = ParticleSkin::bind_grouped(&blade, &s, 0.0, &[0], &[vec![BoneId(2)]]);
    assert_eq!(held.bind_of(0).unwrap().dominant(), BoneId(2));

    // And it actually moves with it.
    let mut pose = s.rest_pose();
    let mut fore = pose.get(BoneId(1)).unwrap();
    fore.rotation = Quat::from_rotation_z(1.2);
    pose.set(BoneId(1), fore);
    let mut out = Vec::new();
    held.apply(&s, &pose, &blade, &mut out);
    assert!(
        (out[0] - blade[0]).length() > 0.3,
        "the held point did not follow the arm: {}",
        out[0]
    );
}

#[test]
fn an_unclassified_point_still_binds_to_something() {
    // A caller that forgets to classify a particle should get one that moves
    // with the body, not one abandoned in mid-air.
    let s = arm();
    let pts = vec![Vec3::new(0.5, 0.0, 0.0), Vec3::new(1.5, 0.0, 0.0)];
    // Only the first point is classified; the second names a group that is not
    // there at all.
    let skin = ParticleSkin::bind_grouped(&pts, &s, 0.2, &[0, 9], &[vec![BoneId(0)]]);
    assert_eq!(skin.bind_of(0).unwrap().dominant(), BoneId(0));
    let b = skin.bind_of(1).unwrap();
    assert!(b.weights[0] > 0.0, "an unclassified point was left unbound");
}

#[test]
fn an_empty_group_means_the_whole_skeleton() {
    let s = arm();
    // Mid-forearm belongs to the forearm; past the tip belongs to the hand.
    let pts = vec![Vec3::new(1.9, 0.0, 0.0), Vec3::new(2.3, 0.0, 0.0)];
    let skin = ParticleSkin::bind_grouped(&pts, &s, 0.0, &[0, 0], &[Vec::new()]);
    assert_eq!(skin.bind_of(0).unwrap().dominant(), BoneId(1));
    assert_eq!(skin.bind_of(1).unwrap().dominant(), BoneId(2));
}
