path = r"C:\proof-engine\editor\src\physics_editor.rs"

addon = """

// ─── Physics Constants ────────────────────────────────────────────────────────

pub const PHYSICS_GRAVITY_EARTH: [f32; 2] = [0.0, -9.81];
pub const PHYSICS_GRAVITY_MOON: [f32; 2] = [0.0, -1.62];
pub const PHYSICS_GRAVITY_MARS: [f32; 2] = [0.0, -3.72];
pub const PHYSICS_GRAVITY_ZERO: [f32; 2] = [0.0, 0.0];
pub const PHYSICS_DEFAULT_TIMESTEP: f32 = 0.0167;
pub const PHYSICS_DEFAULT_VELOCITY_ITERATIONS: u32 = 8;
pub const PHYSICS_DEFAULT_POSITION_ITERATIONS: u32 = 3;
pub const PHYSICS_SLEEP_THRESHOLD: f32 = 0.01;
pub const PHYSICS_BROADPHASE_MARGIN: f32 = 0.1;
pub const PHYSICS_GJK_TOLERANCE: f32 = 0.0001;
pub const PHYSICS_EPA_TOLERANCE: f32 = 0.001;
pub const PHYSICS_MAX_CONTACT_POINTS: u32 = 4;
pub const PHYSICS_BAUMGARTE_FACTOR: f32 = 0.2;
pub const PHYSICS_SPH_H: f32 = 0.5;
pub const SOFT_BODY_VERLET_DAMPING: f32 = 0.99;
pub const CLOTH_RELAXATION_ITERATIONS_COUNT: u32 = 10;
pub const ROPE_PBD_ITERATIONS_COUNT: u32 = 20;
pub const RAGDOLL_BONE_COUNT_HUMANOID: usize = 17;
pub const FLUID_MAX_PARTICLES: usize = 1000;
pub const MATERIAL_LIBRARY_PRESET_COUNT: usize = 22;
pub const COLLISION_MATRIX_SIZE: usize = 16;
pub const PHYSICS_LAYER_COUNT: usize = 16;
pub const IK_MAX_ITERATIONS: u32 = 100;

#[inline] pub fn compute_kinetic_energy(mass: f32, vx: f32, vy: f32, inertia: f32, omega: f32) -> f32 {
    0.5 * mass * (vx*vx + vy*vy) + 0.5 * inertia * omega * omega
}
#[inline] pub fn compute_potential_energy(mass: f32, height: f32, g: f32) -> f32 { mass * g * height }
#[inline] pub fn remap_f32(v: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    let t = if (b-a).abs() < 1e-10 { 0.0 } else { (v-a)/(b-a) };
    c + t*(d-c)
}
#[inline] pub fn point_in_aabb_f32(px: f32, py: f32, min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> bool {
    px >= min_x && px <= max_x && py >= min_y && py <= max_y
}
#[inline] pub fn point_in_circle_f32(px: f32, py: f32, cx: f32, cy: f32, r: f32) -> bool {
    let dx = px-cx; let dy = py-cy; dx*dx+dy*dy <= r*r
}
#[inline] pub fn reflect_vec2(vx: f32, vy: f32, nx: f32, ny: f32) -> [f32; 2] {
    let dot = vx*nx+vy*ny; [vx-2.0*dot*nx, vy-2.0*dot*ny]
}
#[inline] pub fn perp_vec2(vx: f32, vy: f32) -> [f32; 2] { [-vy, vx] }
#[inline] pub fn spring_force_f32(rest: f32, current: f32, stiffness: f32) -> f32 { stiffness * (current - rest) }
#[inline] pub fn damping_force_f32(vel: f32, damping: f32) -> f32 { -damping * vel }
#[inline] pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }
#[inline] pub fn smoothstep_f32(t: f32) -> f32 { let tc = t.clamp(0.0,1.0); tc*tc*(3.0-2.0*tc) }
#[inline] pub fn impulse_magnitude(vn: f32, mass_a: f32, mass_b: f32, restitution: f32) -> f32 {
    if vn > 0.0 { return 0.0; }
    let inv_mass = 1.0/mass_a.max(1e-6) + 1.0/mass_b.max(1e-6);
    -(1.0+restitution)*vn / inv_mass
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(addon)

import subprocess
r = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(r.stdout.strip())
